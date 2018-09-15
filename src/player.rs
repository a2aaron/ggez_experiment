use std::collections::VecDeque;
use std::iter::FromIterator;

use ggez::graphics::{Color, DrawMode, Point2};
use ggez::*;

use enemy::Bullet;
use grid::Grid;
use util::*;

pub struct Player {
    pos: GridPoint, // The current position of the Player
    speed: f32,
    // A list of positions the Player attempts to move to, used for animation
    // purposes. Should never be empty (at least 1 value always)
    keyframes: VecDeque<GridPoint>,
    size: f32,
    color: Color,
    hit_timer: usize,
}

impl Player {
    /// Reset the player in bounds if they try to go out of bounds.
    /// TODO: make sure Keyframes are in bounds as well
    fn handle_boundaries(&mut self, width: f32, height: f32) {
        let pos = &mut self.pos.0;
        if pos[1] > height {
            pos[1] = height;
        } else if pos[1] < 0.0 {
            pos[1] = 0.0;
        }

        if pos[0] < 0.0 {
            pos[0] = 0.0;
        } else if pos[0] > width {
            pos[0] = width;
        }
    }

    pub fn on_hit(&mut self) {
        self.hit_timer = 100;
    }

    /// TODO: Make more general. This is super specific right now.
    pub fn hit(&self, enemy: &Bullet) -> bool {
        distance(self.pos.0, enemy.pos.0) < self.size
    }

    /// Move the Player closer to the next keyframe, and drop that keyframe if
    /// sufficiently close. The last keyframe never drops as that is the latest
    /// intended position.
    pub fn update(&mut self, ctx: &mut Context) {
        if let Some(goal) = self.keyframes.pop_front() {
            let speed = (self.speed * (self.keyframes.len() * 4 + 2) as f32).min(1.0);
            self.pos = lerp(self.pos, goal, speed);
            if distance(self.pos.0, goal.0) > 0.01 || self.keyframes.len() == 0 {
                self.keyframes.push_front(goal);
            }
        }

        self.handle_boundaries(
            ctx.conf.window_mode.width as f32,
            ctx.conf.window_mode.height as f32,
        );

        self.hit_timer = self.hit_timer.saturating_sub(1);
        self.color = color_lerp(WHITE, RED, (self.hit_timer as f32) / 100.0);
    }

    pub fn draw(&mut self, ctx: &mut Context, grid: &Grid) -> GameResult<()> {
        let pos = grid.to_screen_coord(self.pos);
        let goal = grid.to_screen_coord(self.position());
        graphics::set_color(ctx, self.color)?;
        graphics::circle(
            ctx,
            DrawMode::Fill,
            pos,
            grid.to_screen_length(self.size),
            2.0,
        )?;
        graphics::set_color(ctx, RED)?;
        graphics::circle(ctx, DrawMode::Fill, goal, 3.0, 2.0)?;
        Ok(())
    }

    pub fn key_down_event(&mut self, direction: Direction8) {
        use Direction8::*;
        let mut goal: GridPoint = self.position();
        goal.0[0] += match direction {
            Left | LeftDown | LeftUp => -1.0,
            Right | RightDown | RightUp => 1.0,
            Up | Down => 0.0,
        };
        goal.0[1] += match direction {
            Up | LeftUp | RightUp => -1.0,
            Down | LeftDown | RightDown => 1.0,
            Left | Right => 0.0,
        };
        self.keyframes.push_back(goal);
    }

    /// Return the last keyframe, which is the latest intended position.
    pub fn position(&self) -> GridPoint {
        *self.keyframes.back().unwrap()
    }
}

impl Default for Player {
    fn default() -> Self {
        Player {
            pos: GridPoint(Point2::new(0.0, 0.0)),
            speed: 0.2,
            keyframes: VecDeque::from_iter(vec![GridPoint(Point2::new(0.0, 0.0))]),
            size: 0.2,
            color: WHITE,
            hit_timer: 0,
        }
    }
}
