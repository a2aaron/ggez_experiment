use ggez::graphics::DrawMode;
use ggez::graphics::Point2;
use ggez::*;

use rand::Rng;

use util::*;
use grid::Grid;

#[derive(Debug)]
pub struct Enemy {
    pub pos: Point2,
    start_pos: Point2,
    end_pos: Point2,
    pub alive: bool,
    time: f32,
}

impl Enemy {
    pub fn update(&mut self) {
        self.alive = self.time < 1.0;
        self.pos = lerp(self.start_pos, self.end_pos, self.time);
        self.time += 0.01;
    }

    pub fn spawn(grid: &Grid, player_pos: (isize, isize), direction: Direction4) -> Enemy {
        use rand::thread_rng;
        use Direction4::*;

        let width = grid.grid_size.0 as isize;
        let height = grid.grid_size.1 as isize;

        let (pos_x, pos_y) = match direction {
            Left => (0, thread_rng().gen_range(0, height)),
            Right => (width, thread_rng().gen_range(0, height)),
            Up => (thread_rng().gen_range(0, width), 0),
            Down => (thread_rng().gen_range(0, width), height),
        };

        let end_pos_x = thread_rng().gen_range(player_pos.0 - 3, player_pos.0 + 3);
        let end_pos_y = thread_rng().gen_range(player_pos.1 - 3, player_pos.1 + 3);

        Enemy {
            pos: grid.to_screen_coord((pos_x, pos_y)),
            start_pos: grid.to_screen_coord((pos_x, pos_y)),
            end_pos: grid.to_screen_coord((end_pos_x, end_pos_y)),
            alive: true,
            time: 0.0,
        }
    }

    pub fn draw(&self, ctx: &mut Context) -> GameResult<()> {
        graphics::set_color(ctx, RED)?;
        graphics::circle(ctx, DrawMode::Fill, self.pos, 5.0, 2.0)?;
        graphics::set_color(ctx, GREEN)?;
        graphics::circle(ctx, DrawMode::Line(0.5), self.end_pos, 10.0, 2.0)?;
        Ok(())
    }
}