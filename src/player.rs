use ggez::graphics::{Color, DrawMode, Mesh};
use ggez::{nalgebra as na, Context};

use crate::keyboard::KeyboardState;

const HIT_TIME_LENGTH: f64 = 3.0; // How many seconds the hit timer should be

// The size, in pixels, that the "world screen" is. This is a square. Note that
// the center of this square is considered the origin.
const WORLD_SCREEN_SIZE: f64 = 400.0;
// The position that the very edge of the world screen is considered to be. This
// means the top right corner is at WorldPos (100.0, 100.0), while the bottom
// left is at WorldPos (-100.0, -100.0)
const WORLD_BOUNDS: f64 = 100.0;

/// A position in "world space". This is defined as a square whose origin is at
/// the center of the world, and may range from positive to negative along both
/// axes. The axes are oriented like a standard Cartesian plane.
#[derive(Debug, Clone, Copy)]
pub struct WorldPos {
    x: f64,
    y: f64,
}

impl WorldPos {
    pub fn as_screen_coords(&self) -> na::Point2<f32> {
        let center_x = WORLD_SCREEN_SIZE / 2.0;
        let center_y = WORLD_SCREEN_SIZE / 2.0;
        let scale_factor = WORLD_SCREEN_SIZE / WORLD_BOUNDS;
        na::Point2::new(
            (center_x + self.x * scale_factor) as f32,
            (center_y - self.y * scale_factor) as f32,
        )
    }

    pub fn as_screen_length(x: f64) -> f32 {
        let scale_factor = WORLD_SCREEN_SIZE / WORLD_BOUNDS;
        (x * scale_factor) as f32
    }
}

pub struct Player {
    pub pos: WorldPos, // The current position of the Player
    speed: f64,
    pub size: f64,
    color: Color,
    hit_timer: f64,
}

impl Player {
    pub fn new() -> Player {
        Player {
            pos: WorldPos { x: 0.0, y: 0.0 },
            speed: 50.0,
            size: 5.0,
            color: crate::color::WHITE,
            hit_timer: 0.0,
        }
    }

    /// Reset the player in bounds if they try to go out of bounds.
    /// TODO: make sure Keyframes are in bounds as well
    fn handle_boundaries(&mut self, width: f64, height: f64) {
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

            self.pos.x += delta_x * dt * self.speed;
            self.pos.y += delta_y * dt * self.speed;
        }

        // self.hit_timer = self.hit_timer.saturating_sub(1);
        // self.color = color_lerp(WHITE, RED, (self.hit_timer as f64) / HIT_TIME_LENGTH as f64);
    }

    pub fn get_mesh(&self, ctx: &mut Context) -> Result<Mesh, ggez::GameError> {
        Mesh::new_circle(
            ctx,
            DrawMode::fill(),
            [0.0, 0.0],
            WorldPos::as_screen_length(self.size),
            0.1,
            self.color,
        )
    }
}
