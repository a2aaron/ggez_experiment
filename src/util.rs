use ggez::mint;
use rand::{thread_rng, Rng};

use crate::world::WorldPos;

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Direction8 {
    Left,
    Right,
    Up,
    Down,
    LeftUp,
    LeftDown,
    RightUp,
    RightDown,
}

pub fn into_mint<T>(point: cgmath::Point2<T>) -> mint::Point2<T> {
    mint::Point2 {
        x: point.x,
        y: point.y,
    }
}

pub fn mint<T>(x: T, y: T) -> mint::Point2<T> {
    mint::Point2 { x, y }
}

pub fn into_cg<T>(point: mint::Point2<T>) -> cgmath::Point2<T> {
    cgmath::Point2::new(point.x, point.y)
}

/// Return a random WorldPos within some grid. The grid will have side_len total
/// points, and every point will be scaled to fit inside of the bounds.
pub fn random_grid(bound_x: (f64, f64), bound_y: (f64, f64), side_len: usize) -> WorldPos {
    let x_percent = thread_rng().gen_range(0..=side_len) as f64 / (side_len as f64);
    let y_percent = thread_rng().gen_range(0..=side_len) as f64 / (side_len as f64);

    let x = x_percent.lerp(bound_x.0, bound_x.1);
    let y = y_percent.lerp(bound_y.0, bound_y.1);
    WorldPos { x, y }
}

pub fn quartic(n: f64) -> f64 {
    n * n * n * n
}

pub fn rev_quartic(n: f64) -> f64 {
    1.0 - quartic(1.0 - n)
}
