use std::collections::VecDeque;

use ggez::graphics::DrawMode;
use ggez::graphics::Point2;
use ggez::*;

use util::*;

pub struct Ball {
    pos: Point2,
    goal: Point2,
    pub speed: f32, // TODO: make not public
    keyframes: VecDeque<Point2>,
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

    pub fn update(&mut self, ctx: &mut Context) {
        if let Some(goal) = self.keyframes.pop_front() {
            let speed = (self.speed * (self.keyframes.len() + 1) as f32).min(1.0);
            self.pos = lerp(self.pos, goal, speed);
            if distance(self.pos, goal) > 0.01 {
                self.keyframes.push_front(goal);
            }
        }

        self.handle_boundaries(
            ctx.conf.window_mode.width as f32,
            ctx.conf.window_mode.height as f32,
        );
    }

    pub fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::set_color(ctx, WHITE)?;
        graphics::circle(ctx, DrawMode::Fill, self.pos, 10.0, 2.0)?;
        graphics::set_color(ctx, RED)?;
        graphics::circle(ctx, DrawMode::Fill, self.goal, 3.0, 2.0)?;
        Ok(())
    }

    pub fn key_down_event(&mut self, direction: Direction) {
        use Direction::*;
        match direction {
            Left | LeftDown | LeftUp => self.goal[0] += -40.0,
            Right | RightDown | RightUp => self.goal[0] += 40.0,
            Up | Down | None => (),
        }
        match direction {
            Up | LeftUp | RightUp => self.goal[1] += -40.0,
            Down | LeftDown | RightDown => self.goal[1] += 40.0,
            Left | Right | None => (),
        }
        self.keyframes.push_back(self.goal.clone());
    }
}

impl Default for Ball {
    fn default() -> Self {
        Ball {
            pos: Point2::new(0.0, 0.0),
            goal: Point2::new(0.0, 0.0),
            speed: 0.0,
            keyframes: VecDeque::new(),
        }
    }
}
