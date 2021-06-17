use derive_more::{Add, From, Sub};

use ggez::graphics::{Color, DrawMode, Mesh, Rect};
use ggez::{nalgebra as na, Context, GameResult};

use crate::color::{RED, WHITE};
use crate::ease::{color_lerp, Lerp};
use crate::keyboard::KeyboardState;
use crate::{WINDOW_HEIGHT, WINDOW_WIDTH};

const HIT_TIME_LENGTH: f64 = 3.0; // How many seconds the hit timer should be

/// How many pixels that a unit distance in WorldPosition translates to. Here,
/// this means that if two things are 1.0 WorldPos units apart, they are 4 pixels
/// apart in screen space.
pub const WORLD_SCALE_FACTOR: f32 = 4.0;

/// A position in "world space". This is defined as a square whose origin is at
/// the center of the world, and may range from positive to negative along both
/// axes. The axes are oriented like a standard Cartesian plane.
#[derive(Debug, Clone, Copy, From, Add, Sub)]
pub struct WorldPos {
    pub x: f64,
    pub y: f64,
}

impl WorldPos {
    pub fn origin() -> WorldPos {
        WorldPos { x: 0.0, y: 0.0 }
    }
    pub fn as_screen_coords(&self) -> na::Point2<f32> {
        // The origin, in screen coordinates. This is the spot that WorldPos at
        // (0.0, 0.0) shows up at.
        let screen_origin = (WINDOW_WIDTH / 2.0, WINDOW_HEIGHT / 2.0);
        na::Point2::new(
            screen_origin.0 + WORLD_SCALE_FACTOR * self.x as f32,
            screen_origin.1 - WORLD_SCALE_FACTOR * self.y as f32,
        )
    }

    pub fn as_screen_length(x: f64) -> f32 {
        x as f32 * WORLD_SCALE_FACTOR
    }

    // Return a Rect with its units in screen-space. Note th
    pub fn as_screen_rect(center_point: WorldPos, w: f64, h: f64) -> Rect {
        let (x, y) = (center_point.x, center_point.y);
        // Get the upper left corner of the rectangle.
        let corner_point = WorldPos {
            x: x - w / 2.0,
            y: y + h / 2.0,
        };
        let screen_point = corner_point.as_screen_coords();
        Rect::new(
            screen_point.x,
            screen_point.y,
            WorldPos::as_screen_length(w),
            WorldPos::as_screen_length(h),
        )
    }

    pub fn distance(a: WorldPos, b: WorldPos) -> f64 {
        ((a.x - b.x).abs().powi(2) + (a.y - b.y).abs().powi(2)).sqrt()
    }
}

impl Lerp for WorldPos {
    fn lerp_unclamped(a: Self, b: Self, t: f64) -> Self {
        WorldPos {
            x: f64::lerp_unclamped(a.x, b.x, t),
            y: f64::lerp_unclamped(a.y, b.y, t),
        }
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
            speed: 100.0,
            size: 2.0,
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

            self.pos.x += delta_x * dt * self.speed;
            self.pos.y += delta_y * dt * self.speed;

            self.pos.y = self.pos.y.clamp(-100.0, 100.0);
            self.pos.x = self.pos.x.clamp(-100.0, 100.0);
        }

        self.hit_timer -= dt;
        self.color = color_lerp(WHITE, RED, (self.hit_timer as f64) / HIT_TIME_LENGTH as f64);
    }

    pub fn get_mesh(&self, ctx: &mut Context) -> GameResult<Mesh> {
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
