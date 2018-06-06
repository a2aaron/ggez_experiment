use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

pub struct Scheduler {
    work_queue: BinaryHeap<BeatAction>,
}

impl Scheduler {
    pub fn update(&self, beat_time: Beat) {
        let rev_beat = Reverse(beat_time);
        loop {
            match self.work_queue.peek() {
                Some(BeatAction { beat, .. }) if *beat > rev_beat => {
                    if let Some(BeatAction { beat, action }) = self.work_queue.pop() {
                        action.preform()
                    }
                }
                _ => return,
                None => return,
            }
        }
    }
}

struct BeatAction {
    beat: Reverse<Beat>, // for the binary heap
    action: Box<Action>,
}

impl PartialEq for BeatAction {
    fn eq(&self, other: &Self) -> bool {
        self.beat == other.beat
    }
}

impl Eq for BeatAction {}

impl PartialOrd for BeatAction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BeatAction {
    fn cmp(&self, other: &Self) -> Ordering {
        self.beat.cmp(&other.beat)
    }
}

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Beat {
    beat: u32,
    offset: u8, // offset from the beat, in 1/256th increments
}

impl From<f64> for Beat {
    fn from(beat_time: f64) -> Self {
        Beat {
            beat: beat_time as u32,
            offset: (beat_time.fract() * 256.0) as u8,
        }
    }
}

impl Into<f64> for Beat {
    fn into(self) -> f64 {
        self.beat as f64 + (self.offset as f64) / 256.0
    }
}

pub trait Action {
    fn preform(&self) {
        unimplemented!()
    }
}
