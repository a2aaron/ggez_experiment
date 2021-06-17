use std::ops::{Add, Div, Mul, Sub};
pub trait Lerp: Sized + Copy {
    /// Lerp between two values. This function will clamp t.
    fn lerp(a: Self, b: Self, t: f64) -> Self {
        Lerp::lerp_unclamped(a, b, t.clamp(0.0, 1.0))
    }

    /// Lerp between two values. This function will NOT clamp t.
    fn lerp_unclamped(a: Self, b: Self, t: f64) -> Self;
}

// Inverse Lerp
pub trait InvLerp: Sized + Copy {
    /// Returns the "inverse lerp" of a value. The returned value is zero if val == start
    /// and is 1.0 if val == end. This function is clamped to the [0.0, 1.0] range.
    fn inv_lerp(start: Self, end: Self, val: Self) -> f64 {
        InvLerp::inv_lerp_unclamped(start, end, val).clamp(0.0, 1.0)
    }

    /// Returns the "inverse lerp" of a value. The returned value is zero if val == start
    /// and is 1.0 if val == end. This function is not clamped to the [0.0, 1.0] range.
    fn inv_lerp_unclamped(start: Self, end: Self, val: Self) -> f64;
}

impl Lerp for f32 {
    fn lerp_unclamped(a: Self, b: Self, t: f64) -> Self {
        a + (b - a) * t as f32
    }
}

impl Lerp for f64 {
    fn lerp_unclamped(a: Self, b: Self, t: f64) -> Self {
        a + (b - a) * t
    }
}

impl InvLerp for f32 {
    fn inv_lerp_unclamped(start: Self, end: Self, val: Self) -> f64 {
        ((val - start) / (end - start)) as f64
    }
}

impl InvLerp for f64 {
    fn inv_lerp_unclamped(start: Self, end: Self, val: Self) -> f64 {
        (val - start) / (end - start)
    }
}

/// An enum representing an ease.
pub enum Easing<T> {
    /// Linearly ease from start to end.
    Linear { start: T, end: T },
    /// Linearly ease from start to mid when `t` is between `0.0` and `split_at`,
    /// then linearly ease from mid to end when `t` is between `split_at` and `1.0`
    SplitLinear {
        start: T,
        mid: T,
        end: T,
        split_at: f64,
    },
    /// Linearly ease from start to end, but snap to the given number of steps.
    /// For example, if start = 1.0, end = 2.0, steps = 3, then the valid values
    /// are 1.0, 1.5, and 2.0
    SteppedLinear { start: T, end: T, steps: usize },
    /// Exponentially ease from start to end.
    Exponential { start: T, end: T },
}

impl<T: Lerp> Easing<T> {
    /// Ease using the given interpolation value `t`. `t` is expected to be in
    /// [0.0, 1.0] range.
    pub fn ease(&self, t: f64) -> T {
        match *self {
            Easing::Linear { start, end } => T::lerp(start, end, t),
            Easing::SplitLinear {
                start,
                mid,
                end,
                split_at,
            } => {
                if t < split_at {
                    // Map [0.0, split_at] to the [start, mid] range
                    remap(0.0, split_at, t, start, mid)
                } else {
                    // Map [split_at, 1.0] to the [mid, end] range
                    remap(split_at, 1.0, t, mid, end)
                }
            }
            Easing::SteppedLinear { start, end, steps } => {
                let stepped_t = snap_float(t, steps);
                T::lerp(start, end, stepped_t)
            }
            Easing::Exponential { start, end } => {
                let expo_t = ease_in_expo(t);
                T::lerp(start, end, expo_t)
            }
        }
    }
}

impl<T: Lerp + InvLerp> Easing<T> {
    /// Given a value, return the `t` interpolation value such that `ease(t) == val`.
    /// inv_ease assumes easing functions are invertible, which might not be true
    /// for all functions (ex: SplitLinear that does not ease all the way to 1.0)
    pub fn inv_ease(&self, val: T) -> f64 {
        match *self {
            Easing::Linear { start, end } => T::inv_lerp(start, end, val),
            Easing::SplitLinear {
                start,
                mid,
                end,
                split_at,
            } => {
                // First determine if the value fits into the lower half of the function
                // Map [start, end] to [0.0, split_at]
                let lower_val = remap(start, mid, val, 0.0, split_at);
                if lower_val < split_at {
                    lower_val
                } else {
                    // Otherwise the value is in the upper half
                    // Map [mid, end] to [split_at, 1.0]
                    remap(mid, end, val, split_at, 1.0)
                }
            }
            Easing::SteppedLinear { start, end, steps } => {
                let t = T::inv_lerp(start, end, val);
                snap_float(t, steps)
            }
            Easing::Exponential { start, end } => {
                let t = T::inv_lerp(start, end, val);
                inv_ease_in_expo(t)
            }
        }
    }
}

pub struct DiscreteLinear<T, const N: usize> {
    pub values: [T; N],
}

impl<T: Eq + Copy + Clone, const N: usize> DiscreteLinear<T, N> {
    pub fn ease(&self, t: f64) -> T {
        let index = (t * self.values.len() as f64).floor() as usize;
        self.values[index.clamp(0, self.values.len() - 1)]
    }

    pub fn inv_ease(&self, val: T) -> f64 {
        match self.values.iter().position(|&x| x == val) {
            Some(index) => (index as f64) / (self.values.len() as f64),
            None => 0.0,
        }
    }
}

/// Map the range [old_start, old_end] to [new_start, new_end]. Note that
/// lerp(start, end, t) == remap(0.0, 1.0, t, start, end)
/// inv_lerp(start, end, val) == remap(start, end, val, 0.0, 1.0)
pub fn remap<T: InvLerp, U: Lerp>(old_start: T, old_end: T, val: T, new_start: U, new_end: U) -> U {
    let t = T::inv_lerp(old_start, old_end, val);
    U::lerp(new_start, new_end, t)
}

pub fn ease_in_expo(x: f64) -> f64 {
    if x <= 0.0 {
        0.0
    } else {
        (2.0f64.powf(10.0 * x) - 1.0) / (2.0f64.powf(10.0) - 1.0)
    }
}

pub fn inv_ease_in_expo(x: f64) -> f64 {
    if x <= 0.0 {
        0.0
    } else {
        ((2.0f64.powf(10.0) - 1.0) * x + 1.0).log2() / 10.0
    }
}

pub fn ease_in_poly(x: f64, i: i32) -> f64 {
    x.powi(i)
}

/// Snap a float value in range 0.0-1.0 to the nearest f64 region
/// For example, snap_float(_, 4) will snap a float to either:
/// 0.0, 0.333, 0.666, or 1.0
pub fn snap_float(value: f64, num_regions: usize) -> f64 {
    // We subtract one from this denominator because we want there to only be
    // four jumps. See also https://www.desmos.com/calculator/esnnnbfzml
    let num_regions = num_regions as f64;
    (num_regions * value).floor() / (num_regions - 1.0)
}

// Lerp a ggez::graphics::Color
pub fn color_lerp(
    a: ggez::graphics::Color,
    b: ggez::graphics::Color,
    t: impl Into<f64>,
) -> ggez::graphics::Color {
    let t = t.into();
    ggez::graphics::Color::new(
        f32::lerp(a.r, b.r, t),
        f32::lerp(a.g, b.g, t),
        f32::lerp(a.b, b.b, t),
        f32::lerp(a.a, b.a, t),
    )
}
