use std::collections::VecDeque;
use std::iter::FromIterator;

use ggez::graphics::{Color, DrawMode, DrawParam, Drawable, MeshBuilder};
use ggez::{nalgebra as na, Context, GameResult};

use grid::Grid;

use crate::util::{color_lerp, lerp, Direction8, GridPoint, RED, WHITE};

const HIT_TIME_LENGTH: usize = 20; // How many frames the hit timer should be

pub struct Player {
    pos: GridPoint, // The current position of the Player
    speed: f32,
    // A list of positions the Player attempts to move to, used for animation
    // purposes. Should never be empty (at least 1 value always)
    keyframes: VecDeque<GridPoint>,
    pub size: f32,
    color: Color,
    hit_timer: usize,
}

impl Player {
    /// Reset the player in bounds if they try to go out of bounds.
    /// TODO: make sure Keyframes are in bounds as well
    fn handle_boundaries(&mut self, width: f32, height: f32) {
        if self.pos.y > height {
            self.pos.y = height;
        } else if self.pos.y < 0.0 {
            self.pos.y = 0.0;
        }

        if self.pos.x < 0.0 {
            self.pos.x = 0.0;
        } else if self.pos.x > width {
            self.pos.x = width;
        }
    }

    pub fn on_hit(&mut self) {
        self.hit_timer = HIT_TIME_LENGTH;
    }

    /// Move the Player closer to the next keyframe, and drop that keyframe if
    /// sufficiently close. The last keyframe never drops as that is the latest
    /// intended position.
    pub fn update(&mut self, _ctx: &mut Context) {
        if let Some(goal) = self.keyframes.pop_front() {
            let speed = (self.speed * (self.keyframes.len() * 4 + 2) as f32).min(1.0);
            self.pos = lerp(self.pos, goal, speed);
            if na::distance(&self.pos.as_point(), &goal.as_point()) > 0.01
                || self.keyframes.len() == 0
            {
                self.keyframes.push_front(goal);
            }
        }

        self.handle_boundaries(crate::WINDOW_WIDTH, crate::WINDOW_HEIGHT);

        self.hit_timer = self.hit_timer.saturating_sub(1);
        self.color = color_lerp(WHITE, RED, (self.hit_timer as f32) / HIT_TIME_LENGTH as f32);
    }

    pub fn draw(&mut self, ctx: &mut Context, grid: &Grid) -> GameResult<()> {
        let pos = grid.to_screen_coord(self.pos);
        let goal = grid.to_screen_coord(self.position());
        let mut player = MeshBuilder::new();
        player.circle(
            DrawMode::fill(),
            pos,
            grid.to_screen_length(self.size),
            0.1,
            self.color,
        );

        let mut goal_circle = MeshBuilder::new();
        goal_circle.circle(DrawMode::fill(), goal, 3.0, 2.0, RED);

        player.build(ctx)?.draw(ctx, DrawParam::default())?;
        goal_circle.build(ctx)?.draw(ctx, DrawParam::default())?;
        Ok(())
    }

    pub fn key_down_event(&mut self, direction: Direction8) {
        use Direction8::*;
        let mut goal: GridPoint = self.position();
        goal.x += match direction {
            Left | LeftDown | LeftUp => -1.0,
            Right | RightDown | RightUp => 1.0,
            Up | Down => 0.0,
        };
        goal.y += match direction {
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
            pos: GridPoint { x: 0.0, y: 0.0 },
            speed: 0.2,
            keyframes: VecDeque::from_iter(vec![GridPoint { x: 0.0, y: 0.0 }]),
            size: 0.2,
            color: WHITE,
            hit_timer: 0,
        }
    }
}
