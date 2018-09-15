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

pub const GREEN: Color = Color {
    r: 0.0,
    g: 1.0,
    b: 0.0,
    a: 1.0,
};

pub const TRANSPARENT: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

pub const GUIDE_GREY: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 0.2,
};

pub const DEBUG_RED: Color = Color {
    r: 1.0,
    g: 0.1,
    b: 0.1,
    a: 1.0,
};

/// Convience wrapper around Point2, for type enforcement
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct GridPoint(pub Point2);

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

pub fn lerp(a: GridPoint, b: GridPoint, t: f32) -> GridPoint {
    GridPoint(a.0 + (b.0 - a.0) * t)
}

// todo : make this not stupid
pub fn color_lerp(a: Color, b: Color, t: f32) -> Color {
    fn f32_lerp(a: f32, b: f32, t: f32) -> f32 {
        a + (b - a) * t
    }

    Color::new(
        f32_lerp(a.r, b.r, t),
        f32_lerp(a.b, b.b, t),
        f32_lerp(a.g, b.g, t),
        f32_lerp(a.a, b.a, t),
    )
}

/// Generate from [lower, upper). If they are equal then return lower.
pub fn gen_range(lower: isize, upper: isize) -> isize {
    if lower == upper {
        return lower;
    }
    thread_rng().gen_range(lower, upper)
}

// todo: this is an awful way to do this but w/e make it compile
/// Return a random GridPoint around another point.
pub fn rand_around(grid_size: (usize, usize), pos: GridPoint, noise: isize) -> GridPoint {
    let (pos_x, pos_y) = (pos.0[0] as isize, pos.0[1] as isize);
    GridPoint(Point2::new(
        clamp(
            gen_range(pos_x - noise, pos_x + noise),
            0,
            grid_size.0 as isize,
        ) as f32,
        clamp(
            gen_range(pos_y - noise, pos_y + noise),
            0,
            grid_size.1 as isize,
        ) as f32,
    ))
}

/// Return a random GridPoint along an edge.
pub fn rand_edge(grid_size: (usize, usize)) -> GridPoint {
    let width = grid_size.0 as isize;
    let height = grid_size.1 as isize;
    use Direction4::*;
    let (x, y) = match Direction4::rand() {
        Left => (0, gen_range(0, height)),
        Right => (width, gen_range(0, height)),
        Up => (gen_range(0, width), 0),
        Down => (gen_range(0, width), height),
    };
    GridPoint(Point2::new(x as f32, y as f32))
}

pub fn clamp(n: isize, lower: isize, upper: isize) -> isize {
    n.min(upper).max(lower)
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

pub fn quartic(n: f64) -> f64 {
    n * n * n * n
}
