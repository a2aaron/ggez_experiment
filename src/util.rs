use ggez::mint;
use rand::{thread_rng, Rng};

use crate::ease::Lerp;
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

#[allow(dead_code)]
/// Return a random WorldPos along the edge of a circle.
pub fn rand_circle_edge(center: WorldPos, radius: f64) -> WorldPos {
    let angle = thread_rng().gen_range(0.0..2.0) * std::f64::consts::PI;
    let x = angle.cos() * radius;
    let y = angle.sin() * radius;
    WorldPos {
        x: x + center.x,
        y: y + center.y,
    }
}

/// Return a random WorldPos within some grid. The grid will have side_len total
/// points, and every point will be scaled to fit inside of the bounds.
pub fn random_grid(bound_x: (f64, f64), bound_y: (f64, f64), side_len: usize) -> WorldPos {
    let x_percent = thread_rng().gen_range(0..=side_len) as f64 / (side_len as f64);
    let y_percent = thread_rng().gen_range(0..=side_len) as f64 / (side_len as f64);

    let x = f64::lerp(bound_x.0, bound_x.1, x_percent);
    let y = f64::lerp(bound_y.0, bound_y.1, y_percent);
    WorldPos { x, y }
}

pub fn quartic(n: f64) -> f64 {
    n * n * n * n
}

pub fn rev_quartic(n: f64) -> f64 {
    1.0 - quartic(1.0 - n)
}
