use ggez::graphics::Color;

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

impl Lerp for Color {
    fn lerp_unclamped(a: Self, b: Self, t: f64) -> Self {
        ggez::graphics::Color::new(
            f32::lerp_unclamped(a.r, b.r, t),
            f32::lerp_unclamped(a.g, b.g, t),
            f32::lerp_unclamped(a.b, b.b, t),
            f32::lerp_unclamped(a.a, b.a, t),
        )
    }
}

#[derive(Debug, Clone)]
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
    /// Exponentially ease from start to end.
    Exponential { start: T, end: T },
    /// Transform an ease into an ease-out (f(x) => 1 - f(1 - x))
    EaseOut {
        start: T,
        end: T,
        easing: Box<Easing<f64>>,
    },
}

impl<T: Lerp> Easing<T> {
    /// Ease using the given interpolation value `t`. `t` is expected to be in
    /// [0.0, 1.0] range.
    pub fn ease(&self, t: f64) -> T {
        match self {
            &Easing::Linear { start, end } => T::lerp(start, end, t),
            &Easing::SplitLinear {
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
            &Easing::Exponential { start, end } => {
                let expo_t = ease_in_expo(t);
                T::lerp(start, end, expo_t)
            }
            Easing::EaseOut { start, end, easing } => {
                let out_t = 1.0 - easing.ease(1.0 - t);
                T::lerp(*start, *end, out_t)
            }
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
