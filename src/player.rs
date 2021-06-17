use ggez::graphics::{Color, DrawMode, Mesh};
use ggez::{Context, GameResult};

use crate::color::{RED, WHITE};
use crate::ease::color_lerp;
use crate::keyboard::KeyboardState;
use crate::world::{WorldLen, WorldPos};

const HIT_TIME_LENGTH: f64 = 3.0; // How many seconds the hit timer should be

pub struct Player {
    pub pos: WorldPos, // The current position of the Player
    speed: f64,        // In WorldLen units per second
    pub size: WorldLen,
    color: Color,
    hit_timer: f64,
}

impl Player {
    pub fn new() -> Player {
        Player {
            pos: WorldPos { x: 0.0, y: 0.0 },
            speed: 100.0,
            size: WorldLen(2.0),
            color: crate::color::WHITE,
            hit_timer: 0.0,
        }
    }

    pub fn on_hit(&mut self) {
        self.hit_timer = HIT_TIME_LENGTH;
    }

    /// Move the Player closer to the next keyframe, and drop that keyframe if
    /// sufficiently close. The last keyframe never drops as that is the latest
    /// intended position.
    pub fn update(&mut self, dt: f64, keyboard: &KeyboardState) {
        if let Ok(direction) = keyboard.direction() {
            use crate::util::Direction8::*;
            let delta_x = match direction {
                Left | LeftDown | LeftUp => -1.0,
                Right | RightDown | RightUp => 1.0,
                Up | Down => 0.0,
            };
            let delta_y = match direction {
                Up | LeftUp | RightUp => 1.0,
                Down | LeftDown | RightDown => -1.0,
                Left | Right => 0.0,
            };

            let slow = if keyboard.space.is_down { 0.2 } else { 1.0 };

            self.pos.x += delta_x * dt * self.speed * slow;
            self.pos.y += delta_y * dt * self.speed * slow;

            self.pos.y = self.pos.y.clamp(-50.0, 50.0);
            self.pos.x = self.pos.x.clamp(-50.0, 50.0);
        }

        self.hit_timer -= dt;
        self.color = color_lerp(WHITE, RED, (self.hit_timer as f64) / HIT_TIME_LENGTH as f64);
    }

    pub fn get_mesh(&self, ctx: &mut Context) -> GameResult<Mesh> {
        Mesh::new_circle(
            ctx,
            DrawMode::fill(),
            [0.0, 0.0],
            self.size.as_screen_length(),
            0.1,
            self.color,
        )
    }
}
