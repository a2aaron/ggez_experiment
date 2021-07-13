use ggez::graphics::Color;

use crate::time::Beats;

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
pub struct BeatEasing<T> {
    pub easing: Easing<T>,
    pub start_time: Beats,
    pub duration: Beats,
}

impl<T: Lerp> BeatEasing<T> {
    pub fn ease(&self, curr_time: Beats) -> T {
        let delta_time = curr_time.0 - self.start_time.0;
        let t = delta_time / self.duration.0;
        self.easing.ease(t)
    }
}

#[derive(Debug, Clone)]
pub struct Easing<T> {
    pub start: T,
    pub end: T,
    pub kind: EasingKind,
}

impl<T: Lerp> Easing<T> {
    pub fn constant(val: T) -> Easing<T> {
        Easing {
            start: val,
            end: val,
            kind: EasingKind::Constant,
        }
    }

    pub fn linear(start: T, end: T) -> Easing<T> {
        Easing {
            start,
            end,
            kind: EasingKind::Linear,
        }
    }

    pub fn ease(&self, t: f64) -> T {
        let t = self.kind.ease(t);
        T::lerp(self.start, self.end, t)
    }
}

impl<T: InvLerp> Easing<T> {
    pub fn split_linear(start: T, mid_val: T, mid_t: f64, end: T) -> Easing<T> {
        let mid_val = T::inv_lerp(start, end, mid_val);
        Easing {
            start,
            end,
            kind: EasingKind::SplitLinear { mid_val, mid_t },
        }
    }
}

#[derive(Debug, Clone)]
/// An enum representing an ease.
pub enum EasingKind {
    /// Returns `start` always.
    Constant,
    /// Linearly ease from start to end.
    Linear,
    /// Linearly ease from 0.0 to `mid_val` when `t` is between `0.0` and `mid_t`,
    /// then linearly ease from `mid_val` to 1.0 when `t` is between `mid_t` and `1.0`
    SplitLinear { mid_val: f64, mid_t: f64 },
    /// Exponentially ease from start to end.
    Exponential,
    /// Transform an ease into an ease-out (f(x) => 1 - f(1 - x))
    EaseOut { easing: Box<EasingKind> },
}

impl EasingKind {
    fn ease(&self, t: f64) -> f64 {
        match self {
            EasingKind::Constant => 0.0,
            EasingKind::Linear => t,
            &EasingKind::SplitLinear { mid_val, mid_t } => {
                if t < mid_t {
                    // Map [0.0, mid_t] to the [0.0, mid_val] range
                    remap(0.0, mid_t, t, 0.0, mid_val)
                } else {
                    // Map [mid_t, 1.0] to the [mid_val, 1.0] range
                    remap(mid_t, 1.0, t, mid_val, 1.0)
                }
            }
            EasingKind::Exponential => ease_in_expo(t),
            EasingKind::EaseOut { easing } => 1.0 - easing.ease(1.0 - t),
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
