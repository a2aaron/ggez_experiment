use ggez::graphics::{Color, DrawMode};
use ggez::{Context, GameResult, graphics};

use grid::Grid;
use util::{GREEN, GUIDE_GREY, GridPoint, RED, lerp, quartic, smooth_step};
use time::Time;

/// A bullet is a simple enemy that moves from point A to point B in some amount
/// of time. It also has a cool glowy decoration thing for cool glowiness.
// TODO: Add a predelay for fairness
#[derive(Debug)]
pub struct Bullet {
    pub pos: GridPoint, // Current position
    start_pos: GridPoint, // Position bullet started from
    end_pos: GridPoint, // Position bullet must end up at
    start_time: f64, // Start of bullet existance.
    duration: f64, // Time over which this bullet lives, in beats.
    pub alive: bool,
    glow_size: f32,
    glow_trans: f32,
}

impl Bullet {
    pub fn on_spawn(&mut self, start_time: f64) {
        self.start_time = start_time;
    }

    // TODO: Make this use some sort of percent over duration.
    /// Move bullet towards end position. Also do the cool glow thing.
    pub fn update(&mut self, curr_time: f64) {
        let delta_time = curr_time - self.start_time;
        self.alive = delta_time < self.duration;

        let total_percent = Time::percent_over_duration(self.start_time, curr_time, self.duration);
        self.pos = lerp(self.start_pos, self.end_pos, total_percent as f32);

        let beat_percent = delta_time % 1.0;
        self.glow_size = 15.0 * smooth_step(beat_percent) as f32;
        self.glow_trans = 1.0 - quartic(beat_percent) as f32;
    }

    pub fn new(start_pos: GridPoint, end_pos: GridPoint, duration: f64) -> Bullet {
        Bullet {
            pos: start_pos,
            start_pos: start_pos,
            end_pos: end_pos,
            alive: true,
            start_time: 0.0,
            duration: duration,
            glow_size: 0.0,
            glow_trans: 0.0,
        }
    }

    pub fn draw(&self, ctx: &mut Context, grid: &Grid) -> GameResult<()> {
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
}
