use ggez::graphics::DrawMode;
use ggez::graphics::Point2;
use ggez::*;

use rand::Rng;

use util::*;

#[derive(Debug)]
pub struct Enemy {
    pos: Point2,
    start_pos: Point2,
    end_pos: Point2,
    pub alive: bool,
    time: f32,
}

impl Enemy {
    fn handle_boundaries(&mut self, width: f32, height: f32) {
        if self.pos[0] < 0.0 || self.pos[0] > width || self.pos[1] > height || self.pos[1] < 0.0 {
            self.alive = self.time < 1.0;
        }
    }

    pub fn update(&mut self, ctx: &mut Context) {
        self.handle_boundaries(
            ctx.conf.window_mode.width as f32,
            ctx.conf.window_mode.height as f32,
        );
        self.pos = lerp(self.start_pos, self.end_pos, self.time);
        self.time += 0.01;
    }

    pub fn spawn(width: f32, height: f32, wall: Wall) -> Enemy {
        use rand::thread_rng;
        use Wall::*;
        let (pos_x, end_pos_x) = match wall {
            Left => (0.0, width),
            Right => (width, 0.0),
            Up => (
                thread_rng().gen_range(0.0, width),
                thread_rng().gen_range(0.0, width),
            ),
            Down => (
                thread_rng().gen_range(0.0, width),
                thread_rng().gen_range(0.0, width),
            ),
        };

        let (pos_y, end_pos_y) = match wall {
            Left => (
                thread_rng().gen_range(0.0, height),
                thread_rng().gen_range(0.0, height),
            ),
            Right => (
                thread_rng().gen_range(0.0, height),
                thread_rng().gen_range(0.0, height),
            ),
            Up => (0.0, height),
            Down => (height, 0.0),
        };

        Enemy {
            pos: Point2::new(pos_x, pos_y),
            start_pos: Point2::new(pos_x, pos_y),
            end_pos: Point2::new(end_pos_x, end_pos_y),
            alive: true,
            time: 0.0,
        }
    }

    pub fn draw(&self, ctx: &mut Context) -> GameResult<()> {
        graphics::set_color(ctx, RED)?;
        graphics::circle(ctx, DrawMode::Fill, self.pos, 5.0, 2.0)?;
        Ok(())
    }
}