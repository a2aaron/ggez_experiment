use ggez::graphics::Color;

use rand::{thread_rng, Rng};

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
        match thread_rng().gen_range(0..4) {
            0 => Left,
            1 => Right,
            2 => Up,
            3 => Down,
            _ => unreachable!(),
        }
    }
}

pub fn lerpf32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

// todo : make this not stupid
pub fn color_lerp(a: Color, b: Color, t: f32) -> Color {
    fn f32_lerp(a: f32, b: f32, t: f32) -> f32 {
        a + (b - a) * t
    }

    Color::new(
        f32_lerp(a.r, b.r, t),
        f32_lerp(a.g, b.g, t),
        f32_lerp(a.b, b.b, t),
        f32_lerp(a.a, b.a, t),
    )
}

/// Generate from [lower, upper). If they are equal then return lower.
pub fn gen_range(lower: isize, upper: isize) -> isize {
    if lower == upper {
        return lower;
    }
    thread_rng().gen_range(lower..upper)
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

pub fn late_attack(n: f32) -> f32 {
    (n * n / 4.0) + 5.0 * (n - 0.3) * (n - 0.3) * (n - 0.3) * (n - 0.3) * (n - 0.3)
}
