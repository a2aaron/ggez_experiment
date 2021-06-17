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
