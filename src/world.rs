/// This file manages the various conversions to and from the internal "World
/// space" coordinates and the external screen space coordinates.
/// World space is such that 1.0 World space unit is visually 4.0 pixels on
/// screen. Additionally, World space has the y-axis increasing in the upwards
/// direction (opposite to screen space, where it increases in the downwards
/// direction)
use derive_more::{Add, From, Sub};
use ggez::graphics::Rect;
use ggez::mint;

use crate::ease::Lerp;
use crate::{WINDOW_HEIGHT, WINDOW_WIDTH};

/// How many pixels that a unit distance in WorldPosition translates to. Here,
/// this means that if two things are 1.0 WorldLen units apart, they are 4 pixels
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

    pub fn tuple(&self) -> (f64, f64) {
        (self.x, self.y)
    }

    pub fn as_screen_coords_cg(&self) -> cgmath::Point2<f32> {
        crate::util::into_cg(self.as_screen_coords())
    }

    pub fn as_screen_coords(&self) -> mint::Point2<f32> {
        // The origin, in screen coordinates. This is the spot that WorldPos at
        // (0.0, 0.0) shows up at.
        let screen_origin = (WINDOW_WIDTH / 2.0, WINDOW_HEIGHT / 2.0);
        mint::Point2 {
            x: screen_origin.0 + WORLD_SCALE_FACTOR * self.x as f32,
            y: screen_origin.1 - WORLD_SCALE_FACTOR * self.y as f32,
        }
    }

    // Return a Rect with its units in screen-space. Note that the rectangle
    // returned has its point in the upper left, while the input has the center
    // point at the center of the rectangle.
    pub fn as_screen_rect(center_point: WorldPos, w: WorldLen, h: WorldLen) -> Rect {
        let (x, y) = (center_point.x, center_point.y);
        // Get the upper left corner of the rectangle.
        let corner_point = WorldPos {
            x: x - w.0 / 2.0,
            y: y + h.0 / 2.0,
        };
        let screen_point = corner_point.as_screen_coords();
        Rect::new(
            screen_point.x,
            screen_point.y,
            w.as_screen_length(),
            h.as_screen_length(),
        )
    }

    pub fn distance(a: WorldPos, b: WorldPos) -> WorldLen {
        let dx = a.x - b.x;
        let dy = a.y - b.y;
        WorldLen((dx * dx + dy * dy).sqrt())
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

/// A length in World-space
#[derive(Debug, Clone, Copy, From, Add, Sub, PartialOrd, PartialEq)]
pub struct WorldLen(pub f64);

impl WorldLen {
    pub fn as_screen_length(&self) -> f32 {
        self.0 as f32 * WORLD_SCALE_FACTOR
    }
}

impl Lerp for WorldLen {
    fn lerp_unclamped(a: Self, b: Self, t: f64) -> Self {
        WorldLen(f64::lerp_unclamped(a.0, b.0, t))
    }
}
