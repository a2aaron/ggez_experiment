use ggez::graphics::{Color, DrawMode};
use ggez::*;

use grid::Grid;
use util::*;

#[derive(Debug)]
pub struct Enemy {
    pub pos: GridPoint, // Current position
    start_pos: GridPoint, // Position enemy started from
    end_pos: GridPoint, // Position enemy must end up at
    start_time: f64, // Start of bullet existance.
    duration: f64, // Beat time for which this should take place.
    pub alive: bool,
    glow_size: f32,
    glow_trans: f32,
}

impl Enemy {
    pub fn on_spawn(&mut self, start_time: f64) {
        self.start_time = start_time;
    }

    pub fn update(&mut self, curr_time: f64) {
        let delta_time = curr_time - self.start_time;
        self.alive = delta_time < self.duration;
        let total_percent = delta_time / self.duration;
        let beat_percent = delta_time % 1.0;
        self.pos = lerp(self.start_pos, self.end_pos, total_percent as f32);
        self.glow_size = 15.0 * smooth_step(beat_percent) as f32;
        self.glow_trans = 1.0 - quartic(beat_percent) as f32;
    }

    pub fn new(start_pos: GridPoint, end_pos: GridPoint, duration: f64) -> Enemy {
        Enemy {
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
        graphics::set_color(ctx, RED)?;
        graphics::circle(ctx, DrawMode::Fill, pos, 5.0, 2.0)?;
        graphics::set_color(ctx, Color::new(1.0, 0.0, 0.0, self.glow_trans))?;
        graphics::circle(ctx, DrawMode::Fill, pos, self.glow_size, 2.0)?;
        graphics::set_color(ctx, GREEN)?;
        graphics::circle(ctx, DrawMode::Line(0.5), end_pos, 10.0, 2.0)?;
        graphics::set_color(ctx, GUIDE_GREY)?;
        graphics::line(ctx, &[pos, end_pos], 1.0)?;
        Ok(())
    }
}
