use ggez::graphics::{Color, DrawMode};
use ggez::*;

use grid::Grid;
use util::*;

#[derive(Debug)]
pub struct Enemy {
    pub pos: GridPoint,
    start_pos: GridPoint,
    end_pos: GridPoint,
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

    pub fn new(start_pos: GridPoint, end_pos: GridPoint) -> Enemy {
        Enemy {
            pos: start_pos,
            start_pos: start_pos,
            end_pos: end_pos,
            alive: true,
            time: 0.0,
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
