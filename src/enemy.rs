use ggez::graphics::{Color, DrawMode, Point2};
use ggez::*;

use grid::Grid;
use util::*;

#[derive(Debug)]
pub struct Enemy {
    pub pos: Point2,
    start_pos: Point2,
    end_pos: Point2,
    pub alive: bool,
    time: f32,
    glow_size: f32,
    glow_trans: f32,
}

impl Enemy {
    pub fn update(&mut self, beat_percent: f64) {
        self.alive = self.time < 1.0;
        self.pos = lerp(self.start_pos, self.end_pos, self.time);
        self.time += 0.01;
        self.glow_size = 15.0 * smooth_step(beat_percent) as f32;
        self.glow_trans = 1.0 - quartic(beat_percent) as f32;
    }

    pub fn new(grid: &Grid, start_pos: (isize, isize), end_pos: (isize, isize)) -> Enemy {
        Enemy {
            pos: grid.to_screen_coord(start_pos),
            start_pos: grid.to_screen_coord(start_pos),
            end_pos: grid.to_screen_coord(end_pos),
            alive: true,
            time: 0.0,
            glow_size: 0.0,
            glow_trans: 0.0,
        }
    }

    pub fn draw(&self, ctx: &mut Context) -> GameResult<()> {
        graphics::set_color(ctx, RED)?;
        graphics::circle(ctx, DrawMode::Fill, self.pos, 5.0, 2.0)?;
        graphics::set_color(ctx, Color::new(1.0, 0.0, 0.0, self.glow_trans))?;
        graphics::circle(ctx, DrawMode::Fill, self.pos, self.glow_size, 2.0)?;
        graphics::set_color(ctx, GREEN)?;
        graphics::circle(ctx, DrawMode::Line(0.5), self.end_pos, 10.0, 2.0)?;
        graphics::set_color(ctx, GUIDE_GREY)?;
        graphics::line(ctx, &[self.pos, self.end_pos], 1.0)?;
        Ok(())
    }
}
