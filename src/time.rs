use std::cmp::{Ordering, Reverse};
use std::collections::{binary_heap::PeekMut, BinaryHeap};
use std::fmt;
use std::io::BufRead;
use std::io::BufReader;

use std::fs::File;

use enemy::Enemy;
use util::*;
use World;

#[derive(Debug, Default)]
pub struct Scheduler {
    work_queue: BinaryHeap<BeatAction>,
}

impl Scheduler {
    pub fn update(&mut self, beat_time: Beat, world: &mut World) {
        let rev_beat = Reverse(beat_time);
        loop {
            match self.work_queue.peek_mut() {
                Some(peaked) => {
                    if (*peaked).beat > rev_beat {
                        PeekMut::pop(peaked).action.preform(world)
                    } else {
                        return;
                    }
                }
                None => return,
            }
        }
    }

    pub fn read_file(file: File) -> Self {
        let mut scheduler: Scheduler = Default::default();

        let reader = BufReader::new(&file);
        let mut beats = vec![];
        for line in reader.lines() {
            if let Ok(line) = line {
                let line = line.trim().to_string();
                if line.starts_with("#") {
                    continue;
                }

                if line.starts_with("measure") {
                    let mut line = line.split_whitespace();
                    assert_eq!(line.next().unwrap(), "measure");
                    let beat_start: usize = line.next().unwrap().parse::<usize>().unwrap() * 4;
                    let beat_end: usize = line.next().unwrap().parse::<usize>().unwrap() * 4;
                    assert_eq!(line.next().unwrap(), "per");
                    let sizing: usize = line.next().unwrap().parse().unwrap();
                    beats = to_beats(beat_start, beat_end, sizing);
                }

                if line.starts_with("spawn") {
                    let mut line = line.split_whitespace();
                    assert_eq!(line.next().unwrap(), "spawn");
                    let spawn: usize = line.next().unwrap().parse().unwrap();
                    assert_eq!(line.next().unwrap(), "spread");
                    let spread: usize = line.next().unwrap().parse().unwrap();
                    let action = SpawnEnemy {
                        num: spawn,
                        spread: spread as isize,
                    };
                    for beat in beats.iter() {
                        scheduler.work_queue.push(BeatAction {
                            beat: Reverse(*beat),
                            action: Box::new(action),
                        })
                    }
                }
            } else {
                break;
            }
        }
        scheduler
    }
}

fn to_beats(start: usize, end: usize, sizing: usize) -> Vec<Beat> {
    let mut vec = vec![];
    for i in (start..end).step_by(sizing) {
        vec.push(Beat {
                beat: i as u32,
                offset: 0,
            });
    }
    vec
}

/// A wrapper struct to be stored in a binary heap
#[derive(Debug)]
struct BeatAction {
    // Stored in reverse ordering so that we can get the _earliest_ beat
    beat: Reverse<Beat>, // for the binary heap's ordering
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

/// A struct for measuring time, based on beats from the start
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq, Hash, Default, Copy, Clone)]
pub struct Beat {
    beat: u32,
    offset: u8, // offset from the beat, in 1/256th increments
}

/// Beat time is scaled such that 1.0 = 1 beat and 1.5 = 1.5 beat, etc
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
    fn preform(&self, world: &mut World);
}

impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Action")
    }
}

#[derive(Clone, Copy)]
pub struct SpawnEnemy {
    num: usize,
    spread: isize,
}

impl Action for SpawnEnemy {
    fn preform(&self, world: &mut World) {
        for _ in 0..self.num {
            let start_pos = rand_edge(world.grid.grid_size);
            let end_pos = rand_around(world.grid.grid_size, world.player.goal, self.spread);
            world.enemies.push(Enemy::new(start_pos, end_pos));
        }
    }
}
