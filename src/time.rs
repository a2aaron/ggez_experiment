use std::ops;
use std::time::{Duration, Instant};

use ggez::timer;

use util;

/// Time keeping struct
#[derive(Debug)]
pub struct Time {
    start_time: Instant,
    last_time: Instant,
    current_duration: Duration,
    bpm: Duration,
}

impl Time {
    /// Create a new Time struct. Note that the timer will start ticking immediately
    /// After calling.
    pub fn new(bpm: f64) -> Time {
        Time {
            start_time: Instant::now(),
            last_time: Instant::now(),
            current_duration: Duration::new(0, 0),
            bpm: util::bpm_to_duration(bpm),
        }
    }

    pub fn update(&mut self) {
        self.current_duration += Instant::now().duration_since(self.last_time);
        self.last_time = Instant::now();
    }

    /// Reset the timer, resetting the current duration to 0. Note that the timer
    /// will start ticking immediately after call.
    pub fn reset(&mut self) {
        self.start_time = Instant::now();
        self.last_time = Instant::now();
        self.current_duration = Duration::new(0, 0);
    }

    /// Get the time (with 1.0 = 1 beat) since the start
    pub fn f64_time(&self) -> BeatF64 {
        timer::duration_to_f64(self.current_duration) / timer::duration_to_f64(self.bpm)
    }

    /// Get the number of beats since the start
    pub fn beat_time(&self) -> Beat {
        self.f64_time().into()
    }

    /// Return a value from 0.0 to 1.0 indicating the percent through the beat we are at
    pub fn beat_percent(beat: Beat) -> f64 {
        Into::<BeatF64>::into(beat) % 1.0
    }

    /// Return a value from 0.0 to 1.0 indicating how far along the duration we currently are
    pub fn percent_over_duration(start_time: BeatF64, curr_time: BeatF64, duration: f64) -> f64 {
        (curr_time - start_time) / duration
    }
}

/// A struct for measuring time, based on beats from the start
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Hash, Default, Copy, Clone)]
pub struct Beat {
    pub beat: u32,
    pub offset: u8, // offset from the beat, in 1/256th increments
}

impl ops::Sub for Beat {
    type Output = Beat;
    fn sub(self, other: Beat) -> Beat {
        let beat = if self.offset > other.offset {
            self.beat.saturating_sub(other.beat)
        } else {
            self.beat.saturating_sub(other.beat + 1)
        };
        let offset = self.offset.wrapping_sub(other.offset);
        Beat { beat, offset }
    }
}

/// Beat time is scaled such that 1.0 = 1 beat and 1.5 = 1 beat 128 offset, etc
impl From<BeatF64> for Beat {
    fn from(beat_time: BeatF64) -> Self {
        Beat {
            beat: beat_time as u32,
            offset: (beat_time.fract() * 256.0) as u8,
        }
    }
}

/// Similar to From. 1.0f64 = 1 beat 0 offset
impl Into<BeatF64> for Beat {
    fn into(self) -> BeatF64 {
        self.beat as f64 + (self.offset as f64) / 256.0
    }
}

/// Alias type indicating that a f64 represents a beat
/// A BeatF64 is scaled such that 1.0 BeatF64 = 1 beat 0 offset
/// and 1.5 BeatF64 = 1.5 beat 0 offset
pub type BeatF64 = f64;

#[test]
#[allow(clippy::float_cmp)]
fn test_beat_percent() {
    assert_eq!(Time::beat_percent(Beat { beat: 1, offset: 0 }), 0.0);
    assert_eq!(
        Time::beat_percent(Beat {
            beat: 2,
            offset: 128
        }),
        0.5
    );
    assert_eq!(
        Time::beat_percent(Beat {
            beat: 3,
            offset: 64
        }),
        0.25
    );
}

#[test]
#[allow(clippy::float_cmp)]
fn test_over_duration() {
    assert_eq!(Time::percent_over_duration(0.0, 1.0, 10.0), 0.1);
    assert_eq!(Time::percent_over_duration(0.0, 5.0, 10.0), 0.5);
    assert_eq!(Time::percent_over_duration(0.0, 11.0, 10.0), 1.1);
}

#[test]
fn test_time() {
    let mut time = Time::new(120.0);
    let time1 = time.f64_time();
    time.update();
    let time2 = time.f64_time();
    assert!(time2 > time1);
    time.reset();
    let time3 = time.f64_time();
    assert!(time2 > time3);
}

#[test]
#[allow(clippy::float_cmp)]
fn test_beat_to_f64() {
    assert_eq!(1.0f64, Beat { beat: 1, offset: 0 }.into());
    assert_eq!(2.0f64 + 1.0 / 256.0_f64, Beat { beat: 2, offset: 1 }.into());
    assert_eq!(
        3.5f64,
        Beat {
            beat: 3,
            offset: 128
        }
        .into()
    );
    assert_eq!(
        4.25f64,
        Beat {
            beat: 4,
            offset: 64
        }
        .into()
    );
}

#[test]
fn test_f64_to_beat() {
    assert_eq!(Beat::from(1.0f64), Beat { beat: 1, offset: 0 });
    assert_eq!(
        Beat::from(2.0f64 + 1.0 / 256.0f64),
        Beat { beat: 2, offset: 1 }
    );
    assert_eq!(
        Beat::from(3.5f64),
        Beat {
            beat: 3,
            offset: 128
        }
    );
    assert_eq!(
        Beat::from(4.25f64),
        Beat {
            beat: 4,
            offset: 64
        }
    );
}
