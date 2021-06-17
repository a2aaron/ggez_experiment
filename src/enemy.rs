use ggez::graphics::{Color, DrawMode, DrawParam, Drawable, Mesh, MeshBuilder};
use ggez::{nalgebra as na, Context, GameResult};

use crate::color::RED;
use crate::ease::Lerp;
use crate::player::WorldPos;
use crate::time::Beats;

pub const LASER_PREDELAY: f64 = 4.0;
pub const LASER_DURATION: f64 = 1.0;
pub const LASER_PREDELAY_BEATS: Beats = Beats(4.0);

const LASER_COOLDOWN: f64 = 1.0;

const BULLET_GUIDE_RADIUS: f32 = 10.0;
const BULLET_GUIDE_WIDTH: f32 = 1.0;

pub trait Enemy {
    fn on_spawn(&mut self, start_time: Beats);
    fn update(&mut self, curr_time: Beats);
    fn draw(&self, ctx: &mut Context) -> GameResult<()>;
    fn sdf(&self, pos: WorldPos) -> f64;
    fn lifetime_state(&self, curr_time: Beats) -> EnemyLifetime;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EnemyLifetime {
    Unspawned, // The enemy has not spawned yet
    Alive,     // The enemy is currently active
    Dead,      // The enemy is now dead.
}

/// A bullet is a simple enemy that moves from point A to point B in some amount
/// of time. It also has a cool glowy decoration thing for cool glowiness.
// TODO: Add a predelay for fairness
#[derive(Debug)]
pub struct Bullet {
    // Current position
    pub pos: WorldPos,
    // Position bullet started from
    start_pos: WorldPos,
    // Position bullet must end up at
    end_pos: WorldPos,
    // Start of bullet existance. If this is "None", then the bullet hasn't
    // spawned yet.
    start_time: Option<Beats>,
    // Time over which this bullet lives, in beats.
    duration: Beats,
    // Size of glowy bit
    glow_size: f64,
    // Transparency of glowy bit.
    glow_trans: f32,
    // The radius of this bullet, in World space
    size: f64,
}

impl Bullet {
    pub fn new(start_pos: WorldPos, end_pos: WorldPos, duration: Beats) -> Bullet {
        Bullet {
            pos: start_pos,
            start_pos,
            end_pos,
            start_time: None,
            duration,
            glow_size: 0.0,
            glow_trans: 0.0,
            size: 3.0,
        }
    }
}

impl Enemy for Bullet {
    fn on_spawn(&mut self, start_time: Beats) {
        self.start_time = Some(start_time);
    }

    // TODO: Make this use some sort of percent over duration.
    /// Move bullet towards end position. Also do the cool glow thing.
    fn update(&mut self, curr_time: Beats) {
        if self.lifetime_state(curr_time) != EnemyLifetime::Alive {
            return;
        }

        let start_time = self.start_time.unwrap();

        let delta_time = curr_time - start_time;
        let total_percent = delta_time.0 / self.duration.0;
        self.pos = WorldPos::lerp(self.start_pos, self.end_pos, total_percent);

        let beat_percent = delta_time.0 % 1.0;
        self.glow_size = self.size + 5.0 * crate::util::rev_quartic(beat_percent);
        self.glow_trans = 0.5 * (1.0 - beat_percent as f32).powi(4);
    }

    fn sdf(&self, pos: WorldPos) -> f64 {
        WorldPos::distance(pos, self.pos) - self.size
    }

    fn draw(&self, ctx: &mut Context) -> GameResult<()> {
        let pos = self.pos.as_screen_coords();
        let end_pos = self.end_pos.as_screen_coords();
        // TODO: Maybe use a mesh? This is probably really slow
        // Draw the guide
        let mut guide = MeshBuilder::new();
        guide.circle(
            DrawMode::stroke(0.5),
            end_pos,
            BULLET_GUIDE_RADIUS,
            0.1,
            crate::color::GREEN,
        );
        let distance = na::distance(&pos, &end_pos);
        if distance > BULLET_GUIDE_RADIUS {
            let scale_factor = (distance - BULLET_GUIDE_RADIUS) / distance;
            let delta = (end_pos - pos) * scale_factor;
            guide.line(&[pos, pos + delta], BULLET_GUIDE_WIDTH, crate::color::GREEN)?;
        }

        let glow_color = Color::new(1.0, 0.0, 0.0, self.glow_trans);
        // Draw the bullet itself.
        // TODO: consider using draw param "dst" feature here?
        let mut bullet = MeshBuilder::new();
        bullet
            .circle(
                DrawMode::fill(),
                pos,
                WorldPos::as_screen_length(self.size),
                2.0,
                RED,
            )
            // transparent glow
            .circle(
                DrawMode::fill(),
                pos,
                WorldPos::as_screen_length(self.glow_size),
                2.0,
                glow_color,
            );

        guide.build(ctx)?.draw(ctx, DrawParam::default())?;
        bullet.build(ctx)?.draw(ctx, DrawParam::default())?;
        Ok(())
    }

    fn lifetime_state(&self, curr_time: Beats) -> EnemyLifetime {
        if self.start_time.is_none() {
            EnemyLifetime::Unspawned
        } else if self.duration < curr_time - self.start_time.unwrap() {
            EnemyLifetime::Dead
        } else {
            EnemyLifetime::Alive
        }
    }
}

// pub struct Laser {
//     start_time: Beats,
//     durations: LaserDuration,
//     outline_color: Color,
//     outline_keyframes: Envelope, // The outline thickness to animate
//     hitbox_keyframes: Envelope, // The hitbox thickness to animate to and from while in active state.
//     width: f32,                 // The length of the laser, in gridspace
//     outline_thickness: f32,     // Non hitdetecting outline, in gridspace
//     hitbox_thickness: f32,      // In gridspace
//     position: WorldPos,
//     angle: f32,
//     state: LaserState,
//     alive: bool,
// }
// impl Laser {
//     pub fn new_through_point(point: WorldPos, angle: f32, duration: Beats) -> Laser {
//         Laser {
//             start_time: 0.0,
//             durations: LaserDuration::new(duration),
//             outline_keyframes: Envelope {
//                 predelay_keyframes: vec![(0.0, 0.1), (1.0, 0.3)],
//                 active_keyframes: vec![(0.0, 0.6), (1.0, 0.2)],
//                 cooldown_keyframes: vec![(0.0, 0.2), (1.0, 0.0)],
//             },
//             hitbox_keyframes: Envelope {
//                 predelay_keyframes: vec![(0.0, 0.0), (0.0, 0.0)],
//                 active_keyframes: vec![(0.0, 0.3), (0.1, 0.2), (1.0, 0.0)],
//                 cooldown_keyframes: vec![(0.0, 0.0), (1.0, 0.0)],
//             },
//             position: point,
//             angle,
//             width: 30.0,
//             outline_thickness: 0.0,
//             hitbox_thickness: 0.0,
//             outline_color: TRANSPARENT,
//             state: LaserState::Predelay,
//             alive: true,
//         }
//     }
// }

// impl Enemy for Laser {
//     fn on_spawn(&mut self, start_time: Beats) {
//         self.start_time = start_time;
//     }

//     fn update(&mut self, curr_time: Beats) {
//         let delta_time = curr_time - self.start_time;

//         self.alive = delta_time < self.durations.total_duration(LaserState::Cooldown);

//         use self::LaserState::*;
//         self.state = self.durations.get_state(delta_time);
//         let percent_over_state = self.durations.percent_over_state(delta_time) as f32;
//         self.outline_thickness = self
//             .outline_keyframes
//             .lin_interp(self.state, percent_over_state);
//         self.hitbox_thickness = self
//             .hitbox_keyframes
//             .lin_interp(self.state, percent_over_state);
//         let (start_color, end_color) = match self.state {
//             Predelay => (
//                 TRANSPARENT,
//                 Color {
//                     r: 1.0,
//                     g: 0.0,
//                     b: 0.0,
//                     a: 0.8,
//                 },
//             ),
//             Active => (RED, RED),
//             Cooldown => (RED, TRANSPARENT),
//         };
//         self.outline_color = color_lerp(start_color, end_color, percent_over_state);
//     }

//     fn draw(&self, ctx: &mut Context, grid: &Grid) -> GameResult<()> {
//         let position = grid.to_screen_coord(self.position);
//         let width = grid.to_screen_length(self.width);
//         let hitbox_thickness = grid.to_screen_length(self.hitbox_thickness);
//         let outline_thickness = grid.to_screen_length(self.outline_thickness);
//         draw_laser_rect(
//             ctx,
//             position,
//             width,
//             outline_thickness,
//             self.angle,
//             self.outline_color,
//         )?;
//         draw_laser_rect(ctx, position, width, hitbox_thickness, self.angle, WHITE)?;
//         // (probably debug, show the center point of the lazer)
//         let green_circle = Mesh::new_circle(ctx, DrawMode::fill(), position, 4.0, 2.0, GREEN)?;
//         green_circle.draw(ctx, DrawParam::default())?;
//         Ok(())
//     }

//     fn intersects(&self, player: &Player) -> bool {
//         if self.state != LaserState::Active {
//             return false;
//         }
//         // We want the perpendicular of the line from the plane to the player
//         let a = self.angle.sin();
//         let b = -self.angle.cos();
//         let c = -(a * self.position.x + b * self.position.y);

//         let player_pos = player.position();
//         let distance = (a * player_pos.x + b * player_pos.y + c).abs() / (a * a + b * b).sqrt();
//         distance < self.hitbox_thickness / 2.0 + player.size
//     }

//     fn is_alive(&self) -> bool {
//         self.alive
//     }
// }

// fn draw_laser_rect(
//     ctx: &mut Context,
//     position: na::Point2<f32>,
//     width: f32,
//     thickness: f32,
//     angle: f32,
//     color: Color,
// ) -> GameResult<()> {
//     // The mesh is done like this so that we draw about the center of the position
//     // this lets us easily rotate the laser about its position.
//     let points = [
//         na::Point2::new(1.0, 1.0),
//         na::Point2::new(1.0, -1.0),
//         na::Point2::new(-1.0, -1.0),
//         na::Point2::new(-1.0, 1.0),
//     ];
//     let mesh = Mesh::new_polygon(ctx, DrawMode::fill(), &points, color).unwrap();
//     mesh.draw(
//         ctx,
//         DrawParam::default()
//             .dest(position)
//             .scale([width, thickness])
//             .rotation(angle),
//     )?;
//     Ok(())
// }

// #[derive(Debug)]
// struct LaserDuration {
//     predelay: Beats, // The amount of time to show a predelay warning
//     active: Beats,   // The amount of time to actually do hit detection
//     cooldown: Beats, // The amount of time to show a "cool off" animation (and disable hit detection)
// }

// impl LaserDuration {
//     fn new(active_duration: Beats) -> LaserDuration {
//         LaserDuration {
//             predelay: LASER_PREDELAY,
//             active: active_duration,
//             cooldown: LASER_COOLDOWN,
//         }
//     }

//     fn get_state(&self, delta_time: Beats) -> LaserState {
//         if delta_time < self.total_duration(LaserState::Predelay) {
//             LaserState::Predelay
//         } else if delta_time < self.total_duration(LaserState::Active) {
//             LaserState::Active
//         } else {
//             LaserState::Cooldown
//         }
//     }

//     fn percent_over_state(&self, delta_time: Beats) -> f64 {
//         use self::LaserState::*;
//         let state = self.get_state(delta_time);
//         match state {
//             Predelay => delta_time / self.predelay,
//             Active => (delta_time - self.predelay) / self.active,
//             Cooldown => (delta_time - self.predelay - self.active) / self.cooldown,
//         }
//     }

//     /// Returns the total duration until this state occurs
//     fn total_duration(&self, state: LaserState) -> f64 {
//         use self::LaserState::*;
//         match state {
//             Predelay => self.predelay,
//             Active => self.predelay + self.active,
//             Cooldown => self.predelay + self.active + self.cooldown,
//         }
//     }

//     fn percent_over_total(&self, state: LaserState, delta_time: Beats) -> f64 {
//         delta_time / self.total_duration(state)
//     }
// }

// #[derive(Debug, PartialEq, Eq, Clone, Copy)]
// enum LaserState {
//     Predelay,
//     Active,
//     Cooldown,
// }
