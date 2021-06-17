use ggez::graphics::{BlendMode, Color, DrawMode, DrawParam, Drawable, Mesh, MeshBuilder};
use ggez::{nalgebra as na, Context, GameResult};

use crate::color::{GREEN, LASER_RED, RED, TRANSPARENT, WHITE};
use crate::ease::{color_lerp, Easing, Lerp};
use crate::time::Beats;
use crate::world::{WorldLen, WorldPos};

pub const LASER_PREDELAY: Beats = Beats(4.0);
pub const LASER_DURATION: Beats = Beats(1.0);

const LASER_COOLDOWN: Beats = Beats(0.25);

const BULLET_GUIDE_RADIUS: f32 = 10.0;
const BULLET_GUIDE_WIDTH: f32 = 1.0;

pub trait Enemy {
    fn on_spawn(&mut self, start_time: Beats);
    fn update(&mut self, curr_time: Beats);
    fn draw(&self, ctx: &mut Context) -> GameResult<()>;
    /// If None, the enemy has no hitbox, otherwise, positive values give the
    /// distance to the object and negative values are inside the object.
    fn sdf(&self, pos: WorldPos, curr_time: Beats) -> Option<WorldLen>;
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
    glow_size: WorldLen,
    // Transparency of glowy bit.
    glow_trans: f32,
    // The radius of this bullet, in World space
    size: WorldLen,
}

impl Bullet {
    pub fn new(start_pos: WorldPos, end_pos: WorldPos, duration: Beats) -> Bullet {
        Bullet {
            pos: start_pos,
            start_pos,
            end_pos,
            start_time: None,
            duration,
            glow_size: WorldLen(0.0),
            glow_trans: 0.0,
            size: WorldLen(3.0),
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
        self.glow_size = self.size + WorldLen(5.0 * crate::util::rev_quartic(beat_percent));
        self.glow_trans = 0.5 * (1.0 - beat_percent as f32).powi(4);
    }

    fn sdf(&self, pos: WorldPos, _curr_time: Beats) -> Option<WorldLen> {
        Some(WorldPos::distance(pos, self.pos) - self.size)
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
                self.size.as_screen_length(),
                2.0,
                RED,
            )
            // transparent glow
            .circle(
                DrawMode::fill(),
                pos,
                self.glow_size.as_screen_length(),
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

pub struct Laser {
    start_time: Option<Beats>,
    durations: LaserDuration,
    outline_color: Color,
    // The outline thickness to animate, in WorldLen units
    outline_keyframes: [Easing<f64>; 3],
    // The hitbox thickness to animate to and from while in active state.
    // Also in WorldLen units
    hitbox_keyframes: [Easing<f64>; 3],
    width: WorldLen,             // The length of the laser
    outline_thickness: WorldLen, // Non hitdetecting outline
    hitbox_thickness: WorldLen,  // In World space
    position: WorldPos,
    angle: f64,
}
impl Laser {
    pub fn new_through_point(point: WorldPos, angle: f64, duration: Beats) -> Laser {
        Laser {
            start_time: None,
            durations: LaserDuration::new(duration),
            outline_keyframes: [
                Easing::Linear {
                    start: 1.0,
                    end: 3.0,
                },
                Easing::SplitLinear {
                    start: 6.0,
                    end: 1.0,
                    mid: 2.0,
                    split_at: 0.6,
                    // easing: Box::new(Easing::Exponential {
                    //     start: 0.0,
                    //     end: 1.0,
                    // }),
                },
                Easing::Linear {
                    start: 1.0,
                    end: 0.0,
                },
            ],
            hitbox_keyframes: [
                Easing::Linear {
                    start: 0.0,
                    end: 0.0,
                },
                Easing::EaseOut {
                    start: 2.0,
                    end: 1.0,
                    easing: Box::new(Easing::Exponential {
                        start: 0.0,
                        end: 1.0,
                    }),
                },
                Easing::SplitLinear {
                    start: 1.0,
                    mid: 0.0,
                    end: 0.0,
                    split_at: 0.5,
                },
            ],
            position: point,
            angle,
            width: WorldLen(300.0),
            outline_thickness: WorldLen(0.0),
            hitbox_thickness: WorldLen(0.0),
            outline_color: TRANSPARENT,
        }
    }
}

impl Enemy for Laser {
    fn on_spawn(&mut self, start_time: Beats) {
        self.start_time = Some(start_time);
    }

    fn update(&mut self, curr_time: Beats) {
        if self.start_time.is_none() {
            return;
        }
        let start_time = self.start_time.unwrap();
        let delta_time = curr_time - start_time;

        let state = self.durations.get_state(delta_time);
        let index = match state {
            LaserState::Predelay => 0,
            LaserState::Active => 1,
            LaserState::Cooldown => 2,
        };

        let percent_over_state = self.durations.percent_over_state(delta_time);
        self.outline_thickness = WorldLen(self.outline_keyframes[index].ease(percent_over_state));
        self.hitbox_thickness = WorldLen(self.hitbox_keyframes[index].ease(percent_over_state));

        let (start_color, end_color) = match state {
            LaserState::Predelay => (
                TRANSPARENT,
                Color {
                    r: 0.5,
                    g: 0.1,
                    b: 0.1,
                    a: 0.0,
                },
            ),
            LaserState::Active => (LASER_RED, TRANSPARENT),
            LaserState::Cooldown => (TRANSPARENT, TRANSPARENT),
        };
        self.outline_color = color_lerp(start_color, end_color, percent_over_state);
    }

    fn draw(&self, ctx: &mut Context) -> GameResult<()> {
        let position = self.position.as_screen_coords();
        let width = self.width.as_screen_length();
        let hitbox_thickness = self.hitbox_thickness.as_screen_length();
        let outline_thickness = self.outline_thickness.as_screen_length();
        let angle = self.angle as f32;
        draw_laser_rect(
            ctx,
            position,
            width,
            outline_thickness,
            angle,
            self.outline_color,
        )?;
        draw_laser_rect(ctx, position, width, hitbox_thickness, angle, WHITE)?;
        // (probably debug, show the center point of the lazer)
        let green_circle = Mesh::new_circle(ctx, DrawMode::fill(), position, 4.0, 2.0, GREEN)?;
        green_circle.draw(ctx, DrawParam::default())?;

        Ok(())
    }

    fn sdf(&self, pos: WorldPos, curr_time: Beats) -> Option<WorldLen> {
        let start_time = self.start_time?;
        if self.durations.get_state(curr_time - start_time) != LaserState::Active {
            return None;
        }

        let width = self.hitbox_thickness;
        let dist_to_laser = shortest_distance_to_line(
            na::Point2::new(pos.x, pos.y),
            na::Point2::new(self.position.x, self.position.y),
            self.angle,
        );
        Some(WorldLen(dist_to_laser) - width)
    }

    fn lifetime_state(&self, curr_time: Beats) -> EnemyLifetime {
        if self.start_time.is_none() {
            EnemyLifetime::Unspawned
        } else if self.durations.total_duration(LaserState::Cooldown)
            < curr_time - self.start_time.unwrap()
        {
            EnemyLifetime::Dead
        } else {
            EnemyLifetime::Alive
        }
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
    // TODO: Setting blend mode on meshes seems to not work. File an issue &
    // investigate why?
    // mesh.set_blend_mode(Some(BlendMode::Add));
    ggez::graphics::set_blend_mode(ctx, BlendMode::Lighten)?;
    mesh.draw(
        ctx,
        DrawParam::default()
            .dest(position)
            .scale([width, thickness])
            .rotation(-angle),
    )?;
    // TODO/NOTE: There is no way to get the current blend mode, so we will just
    // assume Alpha is the default blend mode.
    ggez::graphics::set_blend_mode(ctx, BlendMode::Alpha)?;

    Ok(())
}

#[derive(Debug)]
struct LaserDuration {
    predelay: Beats, // The amount of time to show a predelay warning
    active: Beats,   // The amount of time to actually do hit detection
    cooldown: Beats, // The amount of time to show a "cool off" animation (and disable hit detection)
}

impl LaserDuration {
    fn new(active_duration: Beats) -> LaserDuration {
        LaserDuration {
            predelay: LASER_PREDELAY,
            active: active_duration,
            cooldown: LASER_COOLDOWN,
        }
    }

    fn get_state(&self, delta_time: Beats) -> LaserState {
        if delta_time < self.total_duration(LaserState::Predelay) {
            LaserState::Predelay
        } else if delta_time < self.total_duration(LaserState::Active) {
            LaserState::Active
        } else {
            LaserState::Cooldown
        }
    }

    /// Return the percent within a particular state.
    fn percent_over_state(&self, delta_time: Beats) -> f64 {
        use self::LaserState::*;
        let state = self.get_state(delta_time);
        let delta_time = delta_time.0;
        let predelay = self.predelay.0;
        let active = self.active.0;
        let cooldown = self.cooldown.0;
        match state {
            Predelay => delta_time / predelay,
            Active => (delta_time - predelay) / active,
            Cooldown => (delta_time - predelay - active) / cooldown,
        }
    }

    /// Returns the total duration that this state takes up, plus the previous
    /// states.
    fn total_duration(&self, state: LaserState) -> Beats {
        use self::LaserState::*;
        match state {
            Predelay => self.predelay,
            Active => self.predelay + self.active,
            Cooldown => self.predelay + self.active + self.cooldown,
        }
    }

    /// Return the percent of the two states combined
    fn percent_over_total(&self, state: LaserState, delta_time: Beats) -> f64 {
        delta_time.0 / self.total_duration(state).0
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum LaserState {
    Predelay,
    Active,
    Cooldown,
}

/// Return the shortest distance from `pos` to the line defined by `line_pos`
/// and `angle`. `angle` is in radians and measure the angle between a horizontal
/// line and the line in question.
pub fn shortest_distance_to_line(
    pos: na::Point2<f64>,
    line_pos: na::Point2<f64>,
    angle: f64,
) -> f64 {
    // We have the vector LP,
    #[allow(non_snake_case)]
    let LP_vec: na::Vector2<f64> = pos - line_pos;
    // The unit vector along the laser
    let laser_unit_vec = na::Vector2::new(angle.cos(), angle.sin());

    // We now find the angle between the two vectors
    let dot_prod = LP_vec.dot(&laser_unit_vec);

    // Project LP_vec along laser_unit_vec
    let proj = dot_prod * laser_unit_vec;

    // Now get the perpendicular, this is the distance to the laser.
    let perp = LP_vec - proj;
    perp.norm()
}

#[cfg(test)]
mod test {
    use ggez::nalgebra as na;

    use crate::enemy::shortest_distance_to_line;

    macro_rules! assert_eq_delta {
        ($x:expr, $y:expr) => {
            if ($x - $y).abs() > 0.0001 {
                panic!("{:?} does not approx. equal {:?}", $x, $y);
            }
        };
    }

    #[test]
    pub fn test_shortest_distance_to_line_horiz() {
        let pi = std::f64::consts::PI;
        let sqrt_3 = 3.0_f64.sqrt();

        let origin = na::Point2::origin();
        let pos = na::Point2::new(1.0, sqrt_3);
        assert_eq_delta!(shortest_distance_to_line(pos, origin, 0.0), pos.y.abs());

        let origin = na::Point2::origin();
        let pos = na::Point2::new(1.0, -sqrt_3);
        assert_eq_delta!(shortest_distance_to_line(pos, origin, pi), pos.y.abs());

        let origin = na::Point2::origin();
        let pos = na::Point2::new(1.0, -sqrt_3);
        assert_eq_delta!(
            shortest_distance_to_line(pos, origin, 2.0 * pi),
            pos.y.abs()
        );
    }

    #[test]
    pub fn test_shortest_distance_to_line_vert() {
        let pi = std::f64::consts::PI;
        let sqrt_3 = 3.0_f64.sqrt();

        let origin = na::Point2::origin();
        let pos = na::Point2::new(1.0, sqrt_3);
        assert_eq_delta!(
            shortest_distance_to_line(pos, origin, pi / 2.0),
            pos.x.abs()
        );

        let pos = na::Point2::new(1.0, sqrt_3);
        assert_eq_delta!(
            shortest_distance_to_line(pos, origin, 3.0 * pi / 2.0),
            pos.x.abs()
        );

        let pos = na::Point2::new(-1.0, -sqrt_3);
        assert_eq_delta!(
            shortest_distance_to_line(pos, origin, -pi / 2.0),
            pos.x.abs()
        );
    }
}
