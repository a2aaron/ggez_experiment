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

pub fn distance(a: &mint::Point2<f32>, b: &mint::Point2<f32>) -> f32 {
    ((a.x - b.x) * (a.x - b.x) + (a.y - b.y) * (a.y - b.y)).sqrt()
}

pub fn distance_f64(a: &mint::Point2<f64>, b: &mint::Point2<f64>) -> f64 {
    ((a.x - b.x) * (a.x - b.x) + (a.y - b.y) * (a.y - b.y)).sqrt()
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

pub fn quartic(n: f64) -> f64 {
    n * n * n * n
}

pub fn rev_quartic(n: f64) -> f64 {
    1.0 - quartic(1.0 - n)
}
