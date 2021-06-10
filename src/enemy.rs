use ggez::graphics::{Color, DrawMode, DrawParam, Drawable, Mesh, MeshBuilder, Scale};
use ggez::{nalgebra as na, Context, GameResult};

use grid::Grid;
use player::Player;
use time::{Beat, BeatF64, Time};
use util;
use util::{color_lerp, lerp, quartic, smooth_step, GridPoint, GREEN, RED, TRANSPARENT, WHITE};

pub const LASER_PREDELAY: f64 = 4.0;
pub const LASER_DURATION: f64 = 1.0;
pub const LASER_PREDELAY_BEATS: Beat = Beat { beat: 4, offset: 0 };

const LASER_COOLDOWN: f64 = 1.0;

const BULLET_GUIDE_RADIUS: f32 = 10.0;
const BULLET_GUIDE_WIDTH: f32 = 1.0;
pub const BULLET_DURATION_BEATS: Beat = Beat { beat: 4, offset: 0 };

pub trait Enemy {
    fn on_spawn(&mut self, start_time: BeatF64);
    fn update(&mut self, curr_time: BeatF64);
    fn draw(&self, ctx: &mut Context, grid: &Grid) -> GameResult<()>;
    fn intersects(&self, player: &Player) -> bool;
    fn is_alive(&self) -> bool;
}

/// A bullet is a simple enemy that moves from point A to point B in some amount
/// of time. It also has a cool glowy decoration thing for cool glowiness.
// TODO: Add a predelay for fairness
#[derive(Debug)]
pub struct Bullet {
    pub pos: GridPoint,   // Current position
    start_pos: GridPoint, // Position bullet started from
    end_pos: GridPoint,   // Position bullet must end up at
    start_time: BeatF64,  // Start of bullet existance.
    duration: BeatF64,    // Time over which this bullet lives, in beats.
    alive: bool,
    glow_size: f32,
    glow_trans: f32,
}

impl Bullet {
    pub fn new(start_pos: GridPoint, end_pos: GridPoint, duration: BeatF64) -> Bullet {
        Bullet {
            pos: start_pos,
            start_pos,
            end_pos,
            start_time: 0.0,
            duration,
            alive: true,
            glow_size: 0.0,
            glow_trans: 0.0,
        }
    }
}

impl Enemy for Bullet {
    fn on_spawn(&mut self, start_time: BeatF64) {
        self.start_time = start_time;
    }

    // TODO: Make this use some sort of percent over duration.
    /// Move bullet towards end position. Also do the cool glow thing.
    fn update(&mut self, curr_time: BeatF64) {
        let total_percent = Time::percent_over_duration(self.start_time, curr_time, self.duration);
        self.pos = lerp(self.start_pos, self.end_pos, total_percent as f32);

        let delta_time = curr_time - self.start_time;
        self.alive = delta_time < self.duration;

        let beat_percent = delta_time % 1.0;
        self.glow_size = 15.0 * smooth_step(beat_percent) as f32;
        self.glow_trans = 1.0 - quartic(beat_percent) as f32;
    }

    fn draw(&self, ctx: &mut Context, grid: &Grid) -> GameResult<()> {
        let pos = grid.to_screen_coord(self.pos);
        let end_pos = grid.to_screen_coord(self.end_pos);
        // TODO: Maybe use a mesh? This is probably really slow
        // Draw the guide
        let mut guide = MeshBuilder::new();
        guide.circle(
            DrawMode::stroke(0.5),
            end_pos,
            BULLET_GUIDE_RADIUS,
            0.1,
            GREEN,
        );
        let distance = na::distance(&pos, &end_pos);
        if distance > BULLET_GUIDE_RADIUS {
            let scale_factor = (distance - BULLET_GUIDE_RADIUS) / distance;
            let delta = (end_pos - pos) * scale_factor;
            guide.line(&[pos, pos + delta], BULLET_GUIDE_WIDTH, GREEN)?;
        }

        let glow_color = Color::new(1.0, 0.0, 0.0, self.glow_trans);
        // Draw the bullet itself.
        // TODO: consider using draw param "dst" feature here?
        let mut bullet = MeshBuilder::new();
        bullet
            .circle(DrawMode::fill(), pos, 5.0, 2.0, RED)
            // transparent glow
            .circle(DrawMode::fill(), pos, self.glow_size, 2.0, glow_color);

        guide.build(ctx)?.draw(ctx, DrawParam::default())?;
        bullet.build(ctx)?.draw(ctx, DrawParam::default())?;
        Ok(())
    }

    fn intersects(&self, player: &Player) -> bool {
        na::distance(&player.position().as_point(), &self.pos.as_point()) < player.size
        // TODO
    }

    fn is_alive(&self) -> bool {
        self.alive
    }
}

pub struct Laser {
    start_time: BeatF64,
    durations: LaserDuration,
    outline_color: Color,
    outline_keyframes: Envelope, // The outline thickness to animate
    hitbox_keyframes: Envelope, // The hitbox thickness to animate to and from while in active state.
    width: f32,                 // The length of the laser, in gridspace
    outline_thickness: f32,     // Non hitdetecting outline, in gridspace
    hitbox_thickness: f32,      // In gridspace
    position: GridPoint,
    angle: f32,
    state: LaserState,
    alive: bool,
}
impl Laser {
    pub fn new_through_point(point: GridPoint, angle: f32, duration: BeatF64) -> Laser {
        Laser {
            start_time: 0.0,
            durations: LaserDuration::new(duration),
            outline_keyframes: Envelope {
                predelay_keyframes: vec![(0.0, 0.1), (1.0, 0.3)],
                active_keyframes: vec![(0.0, 0.6), (1.0, 0.2)],
                cooldown_keyframes: vec![(0.0, 0.2), (1.0, 0.0)],
            },
            hitbox_keyframes: Envelope {
                predelay_keyframes: vec![(0.0, 0.0), (0.0, 0.0)],
                active_keyframes: vec![(0.0, 0.3), (0.1, 0.2), (1.0, 0.0)],
                cooldown_keyframes: vec![(0.0, 0.0), (1.0, 0.0)],
            },
            position: point,
            angle,
            width: 30.0,
            outline_thickness: 0.0,
            hitbox_thickness: 0.0,
            outline_color: TRANSPARENT,
            state: LaserState::Predelay,
            alive: true,
        }
    }
}

impl Enemy for Laser {
    fn on_spawn(&mut self, start_time: BeatF64) {
        self.start_time = start_time;
    }

    fn update(&mut self, curr_time: BeatF64) {
        let delta_time = curr_time - self.start_time;

        self.alive = delta_time < self.durations.total_duration(LaserState::Cooldown);

        use self::LaserState::*;
        self.state = self.durations.get_state(delta_time);
        let percent_over_state = self.durations.percent_over_state(delta_time) as f32;
        self.outline_thickness = self
            .outline_keyframes
            .lin_interp(self.state, percent_over_state);
        self.hitbox_thickness = self
            .hitbox_keyframes
            .lin_interp(self.state, percent_over_state);
        let (start_color, end_color) = match self.state {
            Predelay => (
                TRANSPARENT,
                Color {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.8,
                },
            ),
            Active => (RED, RED),
            Cooldown => (RED, TRANSPARENT),
        };
        self.outline_color = color_lerp(start_color, end_color, percent_over_state);
    }

    fn draw(&self, ctx: &mut Context, grid: &Grid) -> GameResult<()> {
        let position = grid.to_screen_coord(self.position);
        let width = grid.to_screen_length(self.width);
        let hitbox_thickness = grid.to_screen_length(self.hitbox_thickness);
        let outline_thickness = grid.to_screen_length(self.outline_thickness);
        draw_laser_rect(
            ctx,
            position,
            width,
            outline_thickness,
            self.angle,
            self.outline_color,
        )
        .unwrap();
        draw_laser_rect(ctx, position, width, hitbox_thickness, self.angle, WHITE).unwrap();
        // TODO: why is this here?
        let green_circle =
            Mesh::new_circle(ctx, DrawMode::fill(), position, 4.0, 2.0, GREEN).unwrap();
        green_circle.draw(ctx, DrawParam::default())?;
        Ok(())
    }

    fn intersects(&self, player: &Player) -> bool {
        if self.state != LaserState::Active {
            return false;
        }
        // We want the perpendicular of the line from the plane to the player
        let a = self.angle.sin();
        let b = -self.angle.cos();
        let c = -(a * self.position.x + b * self.position.y);

        let player_pos = player.position();
        let distance = (a * player_pos.x + b * player_pos.y + c).abs() / (a * a + b * b).sqrt();
        distance < self.hitbox_thickness / 2.0 + player.size
    }

    fn is_alive(&self) -> bool {
        self.alive
    }
}

fn draw_laser_rect(
    ctx: &mut Context,
    position: na::Point2<f32>,
    width: f32,
    thickness: f32,
    angle: f32,
    color: Color,
) -> GameResult<()> {
    // The mesh is done like this so that we draw about the center of the position
    // this lets us easily rotate the laser about its position.
    let points = [
        na::Point2::new(1.0, 1.0),
        na::Point2::new(1.0, -1.0),
        na::Point2::new(-1.0, -1.0),
        na::Point2::new(-1.0, 1.0),
    ];
    let mesh = Mesh::new_polygon(ctx, DrawMode::fill(), &points, color).unwrap();
    mesh.draw(
        ctx,
        DrawParam::default()
            .dest(position)
            .scale([width, thickness])
            .rotation(angle),
    )?;
    Ok(())
}

#[derive(Debug)]
struct Envelope {
    predelay_keyframes: Vec<(f32, f32)>,
    active_keyframes: Vec<(f32, f32)>,
    cooldown_keyframes: Vec<(f32, f32)>,
}

impl Envelope {
    fn lin_interp(&self, state: LaserState, t: f32) -> f32 {
        use self::LaserState::*;
        let keyframes = match state {
            Predelay => &self.predelay_keyframes,
            Active => &self.active_keyframes,
            Cooldown => &self.cooldown_keyframes,
        };
        match keyframes.binary_search_by(|v| v.0.partial_cmp(&t).expect("Could not compare value"))
        {
            Ok(index) => keyframes[index].1,
            Err(index) => {
                if index == keyframes.len() {
                    return keyframes[index - 1].1;
                }
                let next_point = GridPoint {
                    x: keyframes[index].0,
                    y: keyframes[index].1,
                };
                let prev_point = GridPoint {
                    x: keyframes[index - 1].0,
                    y: keyframes[index - 1].1,
                };
                util::lerp(prev_point, next_point, t).y
            }
        }
    }
}

#[derive(Debug)]
struct LaserDuration {
    predelay: BeatF64, // The amount of time to show a predelay warning
    active: BeatF64,   // The amount of time to actually do hit detection
    cooldown: BeatF64, // The amount of time to show a "cool off" animation (and disable hit detection)
}

impl LaserDuration {
    fn new(active_duration: BeatF64) -> LaserDuration {
        LaserDuration {
            predelay: LASER_PREDELAY,
            active: active_duration,
            cooldown: LASER_COOLDOWN,
        }
    }

    fn get_state(&self, delta_time: BeatF64) -> LaserState {
        if delta_time < self.total_duration(LaserState::Predelay) {
            LaserState::Predelay
        } else if delta_time < self.total_duration(LaserState::Active) {
            LaserState::Active
        } else {
            LaserState::Cooldown
        }
    }

    fn percent_over_state(&self, delta_time: BeatF64) -> f64 {
        use self::LaserState::*;
        let state = self.get_state(delta_time);
        match state {
            Predelay => delta_time / self.predelay,
            Active => (delta_time - self.predelay) / self.active,
            Cooldown => (delta_time - self.predelay - self.active) / self.cooldown,
        }
    }

    /// Returns the total duration until this state occurs
    fn total_duration(&self, state: LaserState) -> f64 {
        use self::LaserState::*;
        match state {
            Predelay => self.predelay,
            Active => self.predelay + self.active,
            Cooldown => self.predelay + self.active + self.cooldown,
        }
    }

    fn percent_over_total(&self, state: LaserState, delta_time: BeatF64) -> f64 {
        delta_time / self.total_duration(state)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum LaserState {
    Predelay,
    Active,
    Cooldown,
}
