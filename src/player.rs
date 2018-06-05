use std::collections::VecDeque;

use ggez::graphics::{DrawMode, Point2, Color};
use ggez::*;

use grid::Grid;
use util::*;
use enemy::Enemy;

pub struct Ball {
    pos: Point2,              // The current position of the Ball
    goal: Point2,             // The position the Ball wants to get to
    pub grid_pos: (isize, isize), // The position of the ball in discreet space
    pub speed: f32,           // TODO: make not public
    keyframes: VecDeque<Point2>,
    size: f32,
    color: Color,
    hit_timer: usize,
}

impl Ball {
    fn handle_boundaries(&mut self, width: f32, height: f32) {
        if self.pos[1] > height {
            self.pos[1] = height;
        } else if self.pos[1] < 0.0 {
            self.pos[1] = 0.0;
        }

        if self.pos[0] < 0.0 {
            self.pos[0] = 0.0;
        } else if self.pos[0] > width {
            self.pos[0] = width;
        }
    }

    pub fn on_hit(&mut self) {
        self.hit_timer = 100;
    }

    pub fn hit(&self, enemy: &Enemy) -> bool {
        distance(self.pos, enemy.pos) < self.size
    }

    pub fn update(&mut self, ctx: &mut Context) {
        if let Some(goal) = self.keyframes.pop_front() {
            let speed = (self.speed * (self.keyframes.len() * 4 + 2) as f32).min(1.0);
            self.pos = lerp(self.pos, goal, speed);
            if distance(self.pos, goal) > 0.01 {
                self.keyframes.push_front(goal);
            }
        }

        self.handle_boundaries(
            ctx.conf.window_mode.width as f32,
            ctx.conf.window_mode.height as f32,
        );

        self.hit_timer = self.hit_timer.saturating_sub(1);
        self.color = color_lerp(WHITE, RED, (self.hit_timer as f32)/100.0);
    }

    pub fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::set_color(ctx, self.color)?;
        graphics::circle(ctx, DrawMode::Fill, self.pos, self.size, 2.0)?;
        graphics::set_color(ctx, RED)?;
        graphics::circle(ctx, DrawMode::Fill, self.goal, 3.0, 2.0)?;
        Ok(())
    }

    pub fn key_down_event(&mut self, direction: Direction8, grid: &Grid) {
        use Direction8::*;
        match direction {
            Left | LeftDown | LeftUp => self.grid_pos.0 += -1,
            Right | RightDown | RightUp => self.grid_pos.0 += 1,
            Up | Down => (),
        }
        match direction {
            Up | LeftUp | RightUp => self.grid_pos.1 += -1,
            Down | LeftDown | RightDown => self.grid_pos.1 += 1,
            Left | Right => (),
        }
        self.goal = grid.to_screen_coord(self.grid_pos);
        self.keyframes.push_back(self.goal);
    }
}

impl Default for Ball {
    fn default() -> Self {
        Ball {
            pos: Point2::new(0.0, 0.0),
            goal: Point2::new(0.0, 0.0),
            grid_pos: (0, 0),
            speed: 0.2,
            keyframes: VecDeque::new(),
            size: 10.0,
            color: WHITE,
            hit_timer: 0,
        }
    }
}
