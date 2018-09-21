use ggez::graphics::{Color, DrawMode, Drawable, Rect, MeshBuilder, Point2};
use ggez::{Context, GameResult, graphics};

use grid::Grid;
use util::{GridPoint, GREEN, GUIDE_GREY, TRANSPARENT, RED, color_lerp, lerp, lerpf32, quartic, smooth_step, distance};
use time::{Time, BeatF64};
use player::Player;

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
    pub pos: GridPoint, // Current position
    start_pos: GridPoint, // Position bullet started from
    end_pos: GridPoint, // Position bullet must end up at
    start_time: BeatF64, // Start of bullet existance.
    duration: BeatF64, // Time over which this bullet lives, in beats.
    alive: bool,
    glow_size: f32,
    glow_trans: f32,
}

impl Bullet {
    pub fn new(start_pos: GridPoint, end_pos: GridPoint, duration: BeatF64) -> Bullet {
        Bullet {
            pos: start_pos,
            start_pos: start_pos,
            end_pos: end_pos,
            start_time: 0.0,
            duration: duration,
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
        // Draw the bullet itself.
        graphics::set_color(ctx, RED)?;
        graphics::circle(ctx, DrawMode::Fill, pos, 5.0, 2.0)?;
        graphics::set_color(ctx, Color::new(1.0, 0.0, 0.0, self.glow_trans))?;
        graphics::circle(ctx, DrawMode::Fill, pos, self.glow_size, 2.0)?;
        // Draw the guide
        graphics::set_color(ctx, GREEN)?;
        graphics::circle(ctx, DrawMode::Line(0.5), end_pos, 10.0, 2.0)?;
        graphics::set_color(ctx, GUIDE_GREY)?;
        graphics::line(ctx, &[pos, end_pos], 1.0)?;
        Ok(())
    }

    fn intersects(&self, player: &Player) -> bool {
        distance(player.position().0, self.pos.0) < player.size // TODO
    }

    fn is_alive(&self) -> bool {
        self.alive
    }
}

pub struct Laser {
    start_time: BeatF64,
    durations: LaserDuration,
    color: Color,
    thickness: f32,
    end_thickness: f32, // The minimum thickness to do hit detection in active state
    bounds: Rect, // Stores the height, width, and offset of the laser
    angle: f32,
    state: LaserState,
    alive: bool,
}
impl Laser {
    pub fn new_through_point(point: GridPoint, angle: f32, thickness: f32, duration: BeatF64) -> Laser {
        let bounds = Rect {
            x: point.0[0],
            y: point.0[1],
            w: 30.0,
            h: 0.0,
        };
        Laser {
            start_time: 0.0,
            durations: LaserDuration::new(duration),
            thickness: thickness,
            end_thickness: 0.1,
            bounds: bounds,
            angle: angle,
            color: TRANSPARENT,
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
        let (start_thickness, end_thickness) = match self.durations.get_state(delta_time) {
            Predelay => (0.0, 0.05),
            Active => (self.thickness, self.end_thickness),
            Cooldown => (self.end_thickness, 0.0),
        };
        self.bounds.h = lerpf32(start_thickness, end_thickness, percent_over_state);

        let (start_color, end_color) = match self.durations.get_state(delta_time) {
            Predelay => (TRANSPARENT, Color {r: 1.0, g: 0.0, b: 0.0, a: 0.5}),
            Active => (RED, RED),
            Cooldown => (RED, TRANSPARENT),
        };
        self.color = color_lerp(start_color, end_color, percent_over_state);
    }

    fn draw(&self, ctx: &mut Context, grid: &Grid) -> GameResult<()> {
        let position = grid.to_screen_coord(GridPoint(Point2::new(self.bounds.x, self.bounds.y)));
        let width = grid.to_screen_length(self.bounds.w);
        let height = grid.to_screen_length(self.bounds.h);
        graphics::set_color(ctx, self.color)?;
        // The mesh is done like this so that we draw about the center of the position
        // this lets us easily rotate the laser about its position.
        let points = [Point2::new(-width / 2.0, -height / 2.0),
                      Point2::new(width / 2.0, -height / 2.0),
                      Point2::new(width / 2.0, height / 2.0),
                      Point2::new(-width / 2.0, height / 2.0)];
        let mesh = MeshBuilder::new().polygon(DrawMode::Fill, &points).build(ctx)?;
        mesh.draw(ctx, position, self.angle)?;
        graphics::set_color(ctx, GREEN)?;
        graphics::circle(ctx, DrawMode::Fill, position, 4.0, 2.0)?;
        Ok(())
    }

    fn intersects(&self, player: &Player) -> bool {
        if self.state != LaserState::Active {
            return false;
        }
        // We want the perpendicular of the line from the plane to the player
        let a = self.angle.sin();
        let b = -self.angle.cos();
        let c = -(a*self.bounds.x + b*self.bounds.y);

        let player_pos = player.position();
        let distance = (a*player_pos.0[0] + b*player_pos.0[1] + c).abs() / (a*a + b*b).sqrt();
        distance < self.bounds.h/2.0 + player.size
    }

    fn is_alive(&self) -> bool{
        self.alive
    }
}

#[derive(Debug)]
struct LaserDuration {
    predelay: BeatF64, // The amount of time to show a predelay warning
    active: BeatF64, // The amount of time to actually do hit detection
    cooldown: BeatF64, // The amount of time to show a "cool off" animation (and disable hit detection)
}

impl LaserDuration {
    fn new(active_duration: BeatF64) -> LaserDuration {
        LaserDuration {
            predelay: 4.0,
            active: active_duration,
            cooldown: 0.5,
        }
    }

    fn get_state(&self, delta_time: BeatF64) -> LaserState {

        if delta_time < self.total_duration(LaserState::Predelay) {
            LaserState::Predelay
        } else if delta_time < self.total_duration(LaserState::Active){
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
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum LaserState {
    Predelay,
    Active,
    Cooldown,
}