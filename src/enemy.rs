use ggez::graphics::{Color, DrawMode, DrawParam, Mesh, MeshBuilder};
use ggez::{Context, GameResult};

use cg::prelude::*;
use cgmath as cg;

use crate::color::{self, LASER_RED, RED, TRANSPARENT, WHITE};
use crate::ease::{Easing, EasingKind, Lerp};
use crate::time::Beats;
use crate::util;
use crate::world::{WorldLen, WorldPos, WORLD_SCALE_FACTOR};

pub const LASER_WARMUP: Beats = Beats(4.0);

pub const BOMB_WARMUP: Beats = Beats(4.0);

const LASER_COOLDOWN: Beats = Beats(0.25);

const TOLERANCE: f32 = 0.1;
const OUTLINE_THICKNESS: f32 = 0.25;

/// The public facing enemy trait that specifies how an enemy behaves over its
/// lifetime of existence.
pub trait Enemy {
    fn update(&mut self, curr_time: Beats);
    fn draw(
        &self,
        ctx: &mut Context,
        curr_time: Beats,
        rotated_about: Option<(WorldPos, f64)>,
    ) -> GameResult<Option<(Mesh, DrawParam)>>;
    // fn position_info(&self, curr_time: Beats) -> (WorldPos, f64);
    /// If None, the enemy has no hitbox, otherwise, positive values give the
    /// distance to the object and negative values are inside the object.
    fn sdf(
        &self,
        pos: WorldPos,
        curr_time: Beats,
        rotated_about: Option<(WorldPos, f64)>,
    ) -> Option<WorldLen>;
    fn lifetime_state(&self, curr_time: Beats) -> EnemyLifetime;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum EnemyLifetime {
    Unspawned, // The enemy has not spawned yet
    Warmup,    // The enemy's hitbox is not active and a warmup animation is shown
    Active,    // The enemy's hitbox is active
    Cooldown,  // The enemy's hitbox is not active and a cooldown animation is shown
    Dead,      // The enemy is now dead.
}

/// The internal enemy implementation trait. This is done so that a blanket impl
/// can be done that specifies most of the desired default behaviors of enemies.
pub trait EnemyImpl {
    /// Return the struct describing the enemy's durations in each phase.
    fn durations(&self) -> EnemyDurations;
    /// Return when this enemy starts to exist. This may be long before or after
    /// the current time.
    fn start_time(&self) -> Beats;
    fn delta_time(&self, curr_time: Beats) -> Beats {
        curr_time - self.start_time()
    }
    fn percent_over_curr_state(&self, curr_time: Beats) -> f64 {
        self.durations()
            .percent_over_curr_state(self.delta_time(curr_time))
    }
    /// Return the sdf of the enemy. Called only if this enemy's lifetime is
    /// in Warmup/Active/Cooldown
    fn sdf(&self, pos: WorldPos, curr_time: Beats) -> WorldLen;
    /// Update the enemy. Called only if this enemy's lifetime is
    /// in Warmup/Active/Cooldown
    fn update(&mut self, curr_time: Beats);
    /// Draw the enemy. Called only if this enemy's lifetime is
    /// in Warmup/Active/Cooldown
    fn get_mesh(&self, ctx: &mut Context, curr_time: Beats) -> GameResult<Mesh>;

    fn position_info(&self, curr_time: Beats) -> (WorldPos, f64);
}

impl<T: EnemyImpl> Enemy for T {
    fn update(&mut self, curr_time: Beats) {
        match self.lifetime_state(curr_time) {
            EnemyLifetime::Unspawned => (),
            EnemyLifetime::Dead => (),
            _ => self.update(curr_time),
        }
    }

    fn draw(
        &self,
        ctx: &mut Context,
        curr_time: Beats,
        rotated_about: Option<(WorldPos, f64)>,
    ) -> GameResult<Option<(Mesh, DrawParam)>> {
        match self.lifetime_state(curr_time) {
            EnemyLifetime::Unspawned => Ok(None),
            EnemyLifetime::Dead => Ok(None),
            _ => {
                let mesh = self.get_mesh(ctx, curr_time)?;
                let (pos, angle) = self.position_info(curr_time);

                // apply the rotation if need be
                let (pos, angle) = if let Some((rot_point, rot_angle)) = rotated_about {
                    (rotate_point(pos, rot_point, rot_angle), angle + rot_angle)
                } else {
                    (pos, angle)
                };

                // Note that the negative angle is required here as `rotation`
                // rotates objects clockwise, but we need counterclockwise
                // rotation. Also note the -4.0 on `scale`. This is needed to
                // flip the y-axis since screen space has the y-axis increasing
                // downwards but worldspace is increasing upwards.
                let param = DrawParam::default()
                    .dest(pos.as_screen_coords())
                    .rotation(-angle as f32)
                    .scale([WORLD_SCALE_FACTOR, -WORLD_SCALE_FACTOR]);
                Ok(Some((mesh, param)))
            }
        }
    }

    fn sdf(
        &self,
        pos: WorldPos,
        curr_time: Beats,
        rotated_about: Option<(WorldPos, f64)>,
    ) -> Option<WorldLen> {
        let pos = if let Some((rot_point, rot_angle)) = rotated_about {
            // since rotated_about rotates the enemy, we can simulate this by
            // rotating the player point opposite to the rot_angle.
            rotate_point(pos, rot_point, -rot_angle)
        } else {
            pos
        };

        if self.lifetime_state(curr_time) != EnemyLifetime::Active {
            None
        } else {
            Some(self.sdf(pos, curr_time))
        }
    }

    fn lifetime_state(&self, curr_time: Beats) -> EnemyLifetime {
        let delta_time = self.delta_time(curr_time);
        let warmup = self.durations().warmup;
        let active = self.durations().active;
        let cooldown = self.durations().cooldown;
        if curr_time < Beats(0.0) {
            EnemyLifetime::Unspawned
        } else if delta_time < warmup {
            EnemyLifetime::Warmup
        } else if delta_time < warmup + active {
            EnemyLifetime::Active
        } else if delta_time < warmup + active + cooldown {
            EnemyLifetime::Cooldown
        } else {
            EnemyLifetime::Dead
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct EnemyDurations {
    pub warmup: Beats,   // The amount of time to show a warmup warning
    pub active: Beats,   // The amount of time to actually do hit detection
    pub cooldown: Beats, // The amount of time to show a "cool off" animation (and disable hit detection)
}

impl EnemyDurations {
    pub fn default_laser(active_duration: Beats) -> EnemyDurations {
        EnemyDurations {
            warmup: LASER_WARMUP,
            active: active_duration,
            cooldown: LASER_COOLDOWN,
        }
    }

    fn percent_over_warmup(&self, delta_time: Beats) -> f64 {
        delta_time.0 / self.warmup.0
    }

    fn percent_over_active(&self, delta_time: Beats) -> f64 {
        (delta_time.0 - self.warmup.0) / self.active.0
    }

    fn percent_over_cooldown(&self, delta_time: Beats) -> f64 {
        (delta_time.0 - (self.warmup.0 + self.active.0)) / self.cooldown.0
    }

    fn percent_over_curr_state(&self, delta_time: Beats) -> f64 {
        if delta_time < Beats(0.0) {
            panic!("Delta time cannot be negative: {:?}", delta_time);
        } else if delta_time < self.warmup {
            self.percent_over_warmup(delta_time)
        } else if delta_time < self.warmup + self.active {
            self.percent_over_active(delta_time)
        } else if delta_time < self.warmup + self.active + self.cooldown {
            self.percent_over_cooldown(delta_time)
        } else {
            panic!("Delta time cannot exceed entire duration")
        }
    }
}

/// A bullet is a simple enemy that moves from point A to point B in some amount
/// of time. It also has a cool glowy decoration thing for cool glowiness.
// TODO: Add a predelay for fairness
#[derive(Debug)]
pub struct Bullet {
    // Position bullet started from
    start_pos: WorldPos,
    // Position bullet must end up at
    end_pos: WorldPos,
    // The start of bullet existance.
    start_time: Beats,
    // Time over which this bullet lives, in beats.
    duration: Beats,
    // The radius of this bullet, in World space
    size: WorldLen,
}

impl Bullet {
    pub fn new(
        start_pos: WorldPos,
        end_pos: WorldPos,
        start_time: Beats,
        duration: Beats,
        size: WorldLen,
    ) -> Bullet {
        Bullet {
            start_pos,
            end_pos,
            start_time,
            duration,
            size,
        }
    }

    fn pos(&self, curr_time: Beats) -> WorldPos {
        let delta_time = self.delta_time(curr_time);
        let total_percent = delta_time.0 / self.duration.0;
        WorldPos::lerp(self.start_pos, self.end_pos, total_percent)
    }
}

impl EnemyImpl for Bullet {
    fn update(&mut self, _curr_time: Beats) {}

    fn sdf(&self, pos: WorldPos, curr_time: Beats) -> WorldLen {
        WorldPos::distance(pos, self.pos(curr_time)) - self.size
    }

    fn get_mesh(&self, ctx: &mut Context, curr_time: Beats) -> GameResult<Mesh> {
        let origin = WorldPos::origin().as_mint();
        let pos = self.pos(curr_time);
        let end_pos = WorldPos::from((self.end_pos.x - pos.x, self.end_pos.y - pos.y)).as_mint();

        let guide_radius = self.size.0 as f32;

        // Draw the guide circle
        let mut mesh = MeshBuilder::new();
        mesh.circle(
            DrawMode::stroke(OUTLINE_THICKNESS),
            end_pos,
            guide_radius,
            TOLERANCE,
            crate::color::GREEN,
        )?;
        // Draw the green guide line
        let cg_origin = util::into_cg(origin);
        let cg_end_pos = util::into_cg(end_pos);
        let distance = cg_origin.distance(cg_end_pos);
        if distance > guide_radius {
            let scale_factor = (distance - guide_radius) / distance;
            let cg_delta = (cg_end_pos - cg_origin) * scale_factor;
            mesh.line(
                &[origin, util::into_mint(cg_origin + cg_delta)],
                OUTLINE_THICKNESS,
                crate::color::GREEN,
            )?;
        }

        // Draw the bullet itself.
        mesh.circle(DrawMode::fill(), origin, self.size.0 as f32, TOLERANCE, RED)?;

        // transparent glow
        let (glow_size, glow_trans) = self.glow(curr_time);
        let glow_color = Color::new(1.0, 0.0, 0.0, glow_trans);
        mesh.circle(
            DrawMode::fill(),
            origin,
            glow_size.0 as f32,
            TOLERANCE,
            glow_color,
        )?;

        mesh.build(ctx)
    }

    fn durations(&self) -> EnemyDurations {
        EnemyDurations {
            warmup: Beats(0.0),
            active: self.duration,
            cooldown: Beats(0.0),
        }
    }

    fn start_time(&self) -> Beats {
        self.start_time
    }

    fn position_info(&self, curr_time: Beats) -> (WorldPos, f64) {
        (self.pos(curr_time), 0.0)
    }
}

impl Bullet {
    fn glow(&self, curr_time: Beats) -> (WorldLen, f32) {
        let percent = curr_time.0 % 1.0;
        let glow_size = self.size + WorldLen(5.0 * crate::util::rev_quartic(percent));
        let glow_trans = 0.5 * (1.0 - percent as f32).powi(4);
        (glow_size, glow_trans)
    }
}

/// A rectangular energy beam. This enemy has a couple of states:
/// Predelay - The warning for the player before the laser activates.
/// Active - The laser is actively firing and can hurt the player.
/// Cooldown - The laser is over and the last bits of the laser are fading out.
pub struct Laser {
    // The start time of this laser. Note that this is when the laser starts to
    // appear on screen (ie: when the Predelay phase occurs)
    start_time: Beats,
    durations: EnemyDurations,
    outline_colors: [Easing<Color>; 4],
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
    /// Create a new laser going through the given points.
    /// start_time marks when the predelay phase of the laser occurs. Note that
    /// this means that the laser does not fire exactly at `start_time`
    pub fn new_through_points(
        a: WorldPos,
        b: WorldPos,
        start_time: Beats,
        durations: EnemyDurations,
        outline_colors: &[Easing<Color>; 4],
        outline_keyframes: &[Easing<f64>; 3],
    ) -> Laser {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        let angle = (dy / dx).atan();
        let angle = if !angle.is_finite() { 0.0 } else { angle };
        Laser::new_through_point(
            a,
            angle,
            start_time,
            durations,
            outline_colors,
            outline_keyframes,
        )
    }

    pub fn new_through_point(
        point: WorldPos,
        angle: f64,
        start_time: Beats,
        durations: EnemyDurations,
        outline_colors: &[Easing<Color>; 4],
        outline_keyframes: &[Easing<f64>; 3],
    ) -> Laser {
        Laser {
            start_time,
            durations,
            outline_colors: outline_colors.clone(),
            outline_keyframes: outline_keyframes.clone(),
            hitbox_keyframes: [
                Easing::linear(0.0, 0.0),
                Easing {
                    start: 2.0,
                    end: 0.5,
                    kind: EasingKind::EaseOut {
                        easing: Box::new(EasingKind::Exponential),
                    },
                },
                Easing::split_linear(0.5, 0.0, 0.5, 0.0),
            ],
            position: point,
            angle,
            width: WorldLen(300.0),
            outline_thickness: WorldLen(0.0),
            hitbox_thickness: WorldLen(0.0),
        }
    }

    pub fn outline_color(&self, curr_time: Beats) -> Color {
        let delta_time = self.delta_time(curr_time);
        let (index, percent) = match self.lifetime_state(curr_time) {
            EnemyLifetime::Warmup => {
                let percent = self.durations.percent_over_warmup(delta_time);
                if percent < 0.25 {
                    (0, percent * 4.0)
                } else {
                    (1, (percent - 0.25) / 0.75)
                }
            }
            EnemyLifetime::Active => (2, self.durations.percent_over_active(delta_time)),
            EnemyLifetime::Cooldown => (3, self.durations.percent_over_cooldown(delta_time)),
            _ => unreachable!(),
        };
        self.outline_colors[index].ease(percent)
    }

    pub fn default_outline_color() -> [Easing<Color>; 4] {
        let red1 = Color::new(0.3, 0.1, 0.1, 0.3);
        let red2 = Color::new(0.5, 0.1, 0.1, 0.3);
        [
            Easing::linear(TRANSPARENT, red1),
            Easing::constant(red2),
            Easing::linear(LASER_RED, TRANSPARENT),
            Easing::constant(TRANSPARENT),
        ]
    }

    pub fn default_outline_keyframes() -> [Easing<f64>; 3] {
        [
            Easing::linear(1.0, 3.0),
            Easing::split_linear(6.0, 2.0, 0.6, 1.0),
            Easing::linear(1.0, 0.0),
        ]
    }
}

impl EnemyImpl for Laser {
    fn update(&mut self, curr_time: Beats) {
        let delta_time = self.delta_time(curr_time);

        let state = self.lifetime_state(curr_time);
        let (index, percent) = match state {
            EnemyLifetime::Warmup => (0, self.durations.percent_over_warmup(delta_time)),
            EnemyLifetime::Active => (1, self.durations.percent_over_active(delta_time)),
            EnemyLifetime::Cooldown => (2, self.durations.percent_over_cooldown(delta_time)),
            _ => unreachable!(),
        };

        self.outline_thickness = WorldLen(self.outline_keyframes[index].ease(percent));
        self.hitbox_thickness = WorldLen(self.hitbox_keyframes[index].ease(percent));
    }

    fn get_mesh(&self, ctx: &mut Context, curr_time: Beats) -> GameResult<Mesh> {
        let length = self.width.0 as f32;
        let hitbox_thickness = self.hitbox_thickness.0 as f32;
        let outline_thickness = self.outline_thickness.0 as f32;

        fn draw_laser_rect(
            mesh: &mut MeshBuilder,
            length: f32,
            thickness: f32,
            color: Color,
        ) -> GameResult<()> {
            let points = [util::mint(-length, 0.0), util::mint(length, 0.0)];
            // Multiply by two here so that the laser is of appropriate thickness.
            mesh.line(&points, thickness * 2.0, color)?;
            Ok(())
        }
        let mut mesh = MeshBuilder::new();
        // outline
        draw_laser_rect(
            &mut mesh,
            length,
            outline_thickness,
            self.outline_color(curr_time),
        )?;
        // hitbox
        draw_laser_rect(&mut mesh, length, hitbox_thickness, WHITE)?;

        mesh.build(ctx)
    }

    fn sdf(&self, pos: WorldPos, _curr_time: Beats) -> WorldLen {
        let width = self.hitbox_thickness;
        let dist_to_laser = shortest_distance_to_line(
            (pos.x, pos.y),
            (self.position.x, self.position.y),
            self.angle,
        );
        WorldLen(dist_to_laser) - width
    }

    fn durations(&self) -> EnemyDurations {
        self.durations
    }

    fn start_time(&self) -> Beats {
        self.start_time
    }

    fn position_info(&self, _curr_time: Beats) -> (WorldPos, f64) {
        (self.position, self.angle)
    }
}

pub struct CircleBomb {
    // The start time of this laser. Note that this is when the laser starts to
    // appear on screen (ie: when the Predelay phase occurs)
    start_time: Beats,
    position: WorldPos,
    max_radius: WorldLen,
}

impl CircleBomb {
    pub fn new(start_time: Beats, position: WorldPos) -> CircleBomb {
        CircleBomb {
            start_time,
            position,
            max_radius: WorldLen(10.0),
        }
    }

    fn radius(&self, curr_time: Beats) -> WorldLen {
        match self.lifetime_state(curr_time) {
            EnemyLifetime::Active => {
                let t = self
                    .durations()
                    .percent_over_active(self.delta_time(curr_time));
                let t = (t * 4.0).clamp(0.0, 1.0);
                WorldLen::lerp(WorldLen(0.0), self.max_radius, t)
            }
            _ => WorldLen(0.0),
        }
    }
}

impl EnemyImpl for CircleBomb {
    fn durations(&self) -> EnemyDurations {
        EnemyDurations {
            warmup: BOMB_WARMUP,
            active: Beats(1.0),
            cooldown: Beats(0.25),
        }
    }

    fn start_time(&self) -> Beats {
        self.start_time
    }

    fn sdf(&self, pos: WorldPos, curr_time: Beats) -> WorldLen {
        WorldPos::distance(pos, self.position) - self.radius(curr_time)
    }

    fn update(&mut self, _curr_time: Beats) {
        // Nothing lmao
    }

    fn get_mesh(&self, ctx: &mut Context, curr_time: Beats) -> GameResult<Mesh> {
        let mut mesh = MeshBuilder::new();
        let origin = WorldPos::origin().as_mint();
        let t = self.percent_over_curr_state(curr_time);

        // outline
        let outline_radius = self.max_radius.0 as f32;
        let outline_color = match self.lifetime_state(curr_time) {
            EnemyLifetime::Warmup => color::WARNING_RED,
            EnemyLifetime::Active => color::RED,
            EnemyLifetime::Cooldown => color::TRANSPARENT,
            _ => unreachable!(),
        };

        mesh.circle(
            DrawMode::stroke(OUTLINE_THICKNESS),
            origin,
            outline_radius,
            TOLERANCE,
            outline_color,
        )?;

        // inner solid circle
        let inner_radius = match self.lifetime_state(curr_time) {
            EnemyLifetime::Warmup => WorldLen::lerp(WorldLen(0.0), self.max_radius, t),
            EnemyLifetime::Active => self.radius(curr_time),
            EnemyLifetime::Cooldown => WorldLen::lerp(self.max_radius, WorldLen(0.0), t),
            _ => unreachable!(),
        }
        .0 as f32;
        let inner_color = match self.lifetime_state(curr_time) {
            EnemyLifetime::Warmup => Color::lerp(color::DARK_WARNING_RED, color::WARNING_RED, t),
            EnemyLifetime::Active => color::RED,
            EnemyLifetime::Cooldown => Color::lerp(color::RED, color::TRANSPARENT, t),
            _ => unreachable!(),
        };

        mesh.circle(
            DrawMode::fill(),
            origin,
            inner_radius,
            TOLERANCE,
            inner_color,
        )?;

        mesh.build(ctx)
    }

    fn position_info(&self, _curr_time: Beats) -> (WorldPos, f64) {
        (self.position, 0.0)
    }
}

/// Return the shortest distance from `pos` to the line defined by `line_pos`
/// and `angle`. `angle` is in radians and measure the angle between a horizontal
/// line and the line in question.
pub fn shortest_distance_to_line(
    pos: impl Into<cg::Point2<f64>>,
    line_pos: impl Into<cg::Point2<f64>>,
    angle: f64,
) -> f64 {
    let pos = pos.into();
    let line_pos = line_pos.into();
    // We have the vector LP,
    #[allow(non_snake_case)]
    let LP_vec = pos - line_pos;
    // The unit vector along the laser
    let laser_unit_vec = cg::Vector2::new(angle.cos(), angle.sin());

    // We now find the angle between the two vectors
    let dot_prod = LP_vec.dot(laser_unit_vec);

    // Project LP_vec along laser_unit_vec
    let proj = dot_prod * laser_unit_vec;

    // Now get the perpendicular, this is the distance to the laser.
    let perp = LP_vec - proj;
    perp.magnitude()
}

/// Rotate `point` about `rot_point` by `rot_angle` radians.
pub fn rotate_point(point: WorldPos, rot_point: WorldPos, rot_angle: f64) -> WorldPos {
    // first translate the point so that the rotation point is at the origin
    let (pos_x, pos_y) = (point.x - rot_point.x, point.y - rot_point.y);
    // now rotate the point by rot_angle
    let (pos_x, pos_y) = (
        pos_x * rot_angle.cos() - pos_y * rot_angle.sin(),
        pos_x * rot_angle.sin() + pos_y * rot_angle.cos(),
    );
    // finally translate the point back
    let (pos_x, pos_y) = (pos_x + rot_point.x, pos_y + rot_point.y);

    WorldPos::from((pos_x, pos_y))
}

#[cfg(test)]
mod test {
    use crate::enemy::shortest_distance_to_line;
    use cg::EuclideanSpace;
    use cgmath as cg;

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

        let origin = cg::Point2::<f64>::origin();
        let pos = cg::Point2::new(1.0, sqrt_3);
        assert_eq_delta!(shortest_distance_to_line(pos, origin, 0.0), pos.y.abs());

        let pos = cg::Point2::new(1.0, -sqrt_3);
        assert_eq_delta!(shortest_distance_to_line(pos, origin, pi), pos.y.abs());

        let pos = cg::Point2::new(1.0, -sqrt_3);
        assert_eq_delta!(
            shortest_distance_to_line(pos, origin, 2.0 * pi),
            pos.y.abs()
        );
    }

    #[test]
    pub fn test_shortest_distance_to_line_vert() {
        let pi = std::f64::consts::PI;
        let sqrt_3 = 3.0_f64.sqrt();

        let origin = cg::Point2::<f64>::origin();
        let pos = cg::Point2::new(1.0, sqrt_3);
        assert_eq_delta!(
            shortest_distance_to_line(pos, origin, pi / 2.0),
            pos.x.abs()
        );

        let pos = cg::Point2::new(1.0, sqrt_3);
        assert_eq_delta!(
            shortest_distance_to_line(pos, origin, 3.0 * pi / 2.0),
            pos.x.abs()
        );

        let pos = cg::Point2::new(-1.0, -sqrt_3);
        assert_eq_delta!(
            shortest_distance_to_line(pos, origin, -pi / 2.0),
            pos.x.abs()
        );
    }
}
