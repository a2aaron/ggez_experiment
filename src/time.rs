use std::fmt::Debug;
use std::time::Instant;

use derive_more::{Add, Div, From, Mul, Rem, Sub};

/// Unit of time representing seconds
#[derive(Copy, Clone, Debug, Add, Div, From, Mul, Rem, Sub, PartialEq, PartialOrd)]
pub struct Seconds(pub f64);

/// Unit of time representing beats
#[derive(Copy, Clone, Add, Div, From, Mul, Rem, Sub, PartialEq, PartialOrd)]
pub struct Beats(pub f64);

/// Convert Beats to the number of Seconds, given some BPM. For example, if the
/// BPM is 100, and it's been 50 beats, then that equates to 30 beats total.
pub fn to_secs(beats: Beats, bpm: f64) -> Seconds {
    Seconds(beats.0 * beat_length(bpm).0)
}

/// Convert Seconds to the number of Beats, given some BPM. For example, if the
/// BPM is 100, and it's been 30 seconds, then that equates to 50 beats total
/// (100 bpm * 30 sec / (60 sec/min) = 50 beats/min * sec * min/sec = 50 beats)
pub fn to_beats(secs: Seconds, bpm: f64) -> Beats {
    Beats(secs.0 * bpm / 60.0)
}

/// Returns the length of time that a single beat takes up given a BPM, in seconds
/// For example, a single beat takes up 0.5 seconds at 120 BPM.
/// AKA: seconds / beat
pub fn beat_length(bpm: f64) -> Seconds {
    Seconds(60.0 / bpm)
}

impl Debug for Beats {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let beat = self.0 as i32;
        let quarter = (self.0.fract() * 4.0) as i32;
        let sixteenth = (self.0.fract() * 16.0) as i32 % 4;
        let sixtyfourths = (self.0.fract() * 64.0) as i32 % 4;
        match (beat, quarter, sixteenth, sixtyfourths) {
            (b, 0, 0, 0) => write!(f, "{}", b),
            (b, q, 0, 0) => write!(f, "{}.{}", b, q),
            (b, q, s, 0) => write!(f, "{}.{}.{}", b, q, s),
            (b, q, s, si) => write!(f, "{}.{}.{}+{}", b, q, s, si),
        }
    }
}

/// Time keeping struct for when music is playing
#[derive(Debug)]
pub struct Time {
    // The BPM of the currently playing song.
    bpm: f64,
    // The _exact_ time at which the song started playing. This is not affected
    // by offset.
    exact_start: Instant,
    // The time at which the most recent `update()` call occured.
    last_update: Option<Instant>,
    // The amount of time to nudge `started_at()`. This value may be negative.
    // This is useful if an audio file contains a small delay at the start of
    // the song. For example, if `offset` is 0.65 then 0.65 seconds are added to `get_time()`.
    offset: Seconds,
}

impl Time {
    // Construct a Time. Note that this timer start ticking immediately after
    // this call, so you should play your song soon after you call this function.
    pub fn new(bpm: f64, offset: Seconds) -> Time {
        Time {
            bpm,
            exact_start: Instant::now(),
            last_update: None,
            offset,
        }
    }

    pub fn update(&mut self) {
        self.last_update = Some(Instant::now());
    }

    /// Return the time sinceDuration::from_std( the SongTime started ticking. This is affected by).unwrap()
    /// the `offset` value. Specifically, it adds
    /// If `update()` has not been called since the last `reset()` or `new()` call
    /// then this function returns a duration of zero, still offset by `offset`.
    pub fn get_time(&self) -> Seconds {
        let exact = if let Some(last_update) = self.last_update {
            // It is exceedingly unlikely that the duration since the last update
            // exceeds the bounds for chrono::Durations.
            // TODO: Is it really okay to unwrap this?
            last_update.duration_since(self.exact_start).as_secs_f64()
        } else {
            0.0
        };

        Seconds(exact) + self.offset
    }

    pub fn get_beats(&self) -> Beats {
        to_beats(self.get_time(), self.bpm)
    }
}
