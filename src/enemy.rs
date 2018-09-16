use ggez::graphics::{Color, DrawMode, Drawable, Rect, Matrix4, MeshBuilder, Point2};
use ggez::{Context, GameResult, graphics};

use ggez::nalgebra::core as na;

use grid::Grid;
use util::{GREEN, GUIDE_GREY, GridPoint, RED, lerp, quartic, smooth_step, distance};
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
    duration: BeatF64,
    color: Color,
    thickness: f32,
    bounds: Rect, // Stores the height, width, and offset of the laser
    angle: f32,
    alive: bool,
}
impl Laser {
    pub fn new_through_point(point: GridPoint, angle: f32, thickness: f32, duration: BeatF64) -> Laser {
        let bounds = Rect {
            x: point.0[0],
            y: point.0[1],
            w: 3.0,
            h: thickness,
        };
        Laser {
            start_time: 0.0,
            duration: duration,
            thickness: thickness,
            bounds: bounds,
            angle: angle,
            color: RED,
            alive: true,
        }
    }
}

impl Enemy for Laser {
    fn on_spawn(&mut self, start_time: BeatF64) {
        self.start_time = start_time;
        self.alive = true;
    }

    fn update(&mut self, curr_time: BeatF64) {
        let delta_time = curr_time - self.start_time;
        self.alive = delta_time < self.duration;

        self.bounds.h = self.thickness * (1.0 - Time::percent_over_duration(self.start_time, curr_time, self.duration) as f32);
    }

    fn draw(&self, ctx: &mut Context, grid: &Grid) -> GameResult<()> {
        let position = grid.to_screen_coord(GridPoint(Point2::new(self.bounds.x, self.bounds.y)));
        let width = grid.to_screen_length(self.bounds.w);
        let height = grid.to_screen_length(self.bounds.h);
        graphics::set_color(ctx, self.color)?;
        let points = [Point2::new(0.0, 0.0),
                      Point2::new(width, 0.0),
                      Point2::new(width, height),
                      Point2::new(0.0, height)];
        let mesh = MeshBuilder::new().polygon(DrawMode::Fill, &points).build(ctx)?;
        let translate = Matrix4::new_translation(&na::Vector3::new(position.x, position.y, 0.0)) * Matrix4::from_axis_angle(&na::Unit::new_normalize(na::Vector3::new(0.0, 0.0, 1.0)), self.angle);
        graphics::push_transform(ctx, Some(translate));
        graphics::apply_transformations(ctx)?;
        mesh.draw(ctx, Point2::new(-width/2.0, -height/2.0), 0.0)?;
        graphics::set_color(ctx, GUIDE_GREY)?;
        graphics::circle(ctx, DrawMode::Fill, Point2::new(0.0, 0.0), 3.0, 2.0)?;
        graphics::pop_transform(ctx);
        graphics::apply_transformations(ctx)?;
        graphics::set_color(ctx, GREEN)?;
        graphics::circle(ctx, DrawMode::Fill, position, 3.0, 2.0)?;
        Ok(())
    }

    fn intersects(&self, player: &Player) -> bool {
        false // TODO
    }

    fn is_alive(&self) -> bool{
        self.alive
    }
}
