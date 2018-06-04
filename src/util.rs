use std::time::Duration;

use ggez::graphics::{Color, Point2};
use ggez::timer;

use rand::{thread_rng, Rng};

pub const WHITE: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

pub const RED: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

pub const TRANSPARENT: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,  
};

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

pub enum Direction4 {
    Left,
    Right,
    Up,
    Down,
}

impl Direction4 {
    pub fn rand() -> Direction4 {
        use Direction4::*;
        match thread_rng().gen_range(0, 4) {
            0 => Left,
            1 => Right,
            2 => Up,
            3 => Down,
            _ => unreachable!(),
        }
    }
}

pub fn lerp(current: Point2, goal: Point2, time: f32) -> Point2 {
    current + (goal - current) * time
}

pub fn distance(a: Point2, b: Point2) -> f32 {
    ((a[0] - b[0]).powf(2.0) + (a[1] - b[1]).powf(2.0)).sqrt()
}

pub fn bpm_to_duration(bpm: f64) -> Duration {
    timer::f64_to_duration(60.0 / bpm)
}

pub fn rev_quad(n: f64) -> f64 {
    (1.0 - n) * (1.0 - n)
}

pub fn smooth_step(n: f64) -> f64 {
    -2.0 * n * n + 3.0 * n * n
}
