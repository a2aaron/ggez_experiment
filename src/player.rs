use std::collections::VecDeque;

use ggez::graphics::{Point2, DrawMode, MeshBuilder, Mesh, Color};
use ggez::*;

use util::*;

pub struct Ball {
    pos: Point2, // The current position of the Ball
    goal: Point2, // The position the Ball wants to get to
    grid_pos: (isize, isize), // The position of the ball in discreet space
    pub speed: f32, // TODO: make not public
    keyframes: VecDeque<Point2>,
    grid: Grid,
}

impl Ball {
    fn handle_boundaries(&mut self, width: f32, height: f32) {
        if self.pos[1] > height {
            self.pos[1] = height;
        } else if self.pos[1] < 0.0 {
            self.pos[1] = 0.0;
        }

        if self.pos[0] < 0.0 {
            self.pos[0] = width;
        } else if self.pos[0] > width {
            self.pos[0] = 0.0;
        }
    }

    pub fn update(&mut self, ctx: &mut Context, beat_percent: f64) {
        if let Some(goal) = self.keyframes.pop_front() {
            let speed = (self.speed * (self.keyframes.len() + 2) as f32).min(1.0);
            self.pos = lerp(self.pos, goal, speed);
            if distance(self.pos, goal) > 0.01 {
                self.keyframes.push_front(goal);
            }
        }

        self.handle_boundaries(
            ctx.conf.window_mode.width as f32,
            ctx.conf.window_mode.height as f32,
        );
        let color = 0.3 + 0.7 * smooth_step(1.0 - beat_percent) as f32;
        self.grid.color = Color::new(color, color, color, 1.0);
    }

    pub fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::set_color(ctx, self.grid.color)?;
        let grid_mesh = self.grid.mesh(ctx)?;
        graphics::draw(ctx, &grid_mesh, self.grid.offset, 0.0)?;
        graphics::set_color(ctx, WHITE)?;
        graphics::circle(ctx, DrawMode::Fill, self.pos, 10.0, 2.0)?;
        graphics::set_color(ctx, RED)?;
        graphics::circle(ctx, DrawMode::Fill, self.goal, 3.0, 2.0)?;
        Ok(())
    }

    pub fn key_down_event(&mut self, direction: Direction8) {
        use Direction8::*;
        match direction {
            Left | LeftDown | LeftUp => self.grid_pos.0 += -1,
            Right | RightDown | RightUp => self.grid_pos.0 += 1,
            Up | Down => (),
        }
        match direction {
            Up | LeftUp | RightUp => self.grid_pos.1 += 1,
            Down | LeftDown | RightDown => self.grid_pos.1 += -1,
            Left | Right => (),
        }
        self.goal = self.grid.to_screen_coord(self.grid_pos);
        self.keyframes.push_back(self.goal);
    }
}

impl Default for Ball {
    fn default() -> Self {
        Ball {
            pos: Point2::new(0.0, 0.0),
            goal: Point2::new(0.0, 0.0),
            grid_pos: (0, 0),
            speed: 0.0,
            keyframes: VecDeque::new(),
            grid: Default::default(),
        }
    }
}


struct Grid {
    offset: Point2,
    grid_spacing: f32,
    grid_size: (usize, usize),
    line_width: f32,
    color: Color,
}

impl Default for Grid {
    fn default() -> Self {
        Grid {
            offset: Point2::new(15.0f32, 15.0f32),
            grid_spacing: 50.0,
            grid_size: (12, 9),
            line_width: 1.0,
            color: WHITE,
        }
    }
}

impl Grid {
    fn mesh(&mut self, ctx: &mut Context) -> GameResult<Mesh> {
        let mut mb = MeshBuilder::new();
        let max_x = self.grid_spacing * self.grid_size.0 as f32;
        let max_y = self.grid_spacing * self.grid_size.1 as f32;
        for i in 0..self.grid_size.0 {
            mb.line(&[
                Point2::new(self.grid_spacing * i as f32, 0.0),
                Point2::new(self.grid_spacing * i as f32, max_y),
            ], self.line_width);
        }

        for i in 0..self.grid_size.1 {
            mb.line(&[
                Point2::new(0.0, self.grid_spacing * i as f32),
                Point2::new(max_x, self.grid_spacing * i as f32),
            ], self.line_width);
        }

        mb.line(&[
            Point2::new(max_x, 0.0),
            Point2::new(max_x, max_y),
        ], self.line_width);

        mb.line(&[
                Point2::new(0.0, max_y),
                Point2::new(max_x, max_y),
            ], self.line_width);
        mb.build(ctx)
    }

    fn to_screen_coord(&self, grid_coord: (isize, isize)) -> Point2 {
        Point2::new(grid_coord.0 as f32 * self.grid_spacing + self.offset[0], -grid_coord.1 as f32 * self.grid_spacing + self.offset[1])
    }
}