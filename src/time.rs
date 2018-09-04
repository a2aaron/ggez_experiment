use std::cmp::{Ordering, Reverse};
use std::collections::{binary_heap::PeekMut, BinaryHeap};
use std::fmt;
use std::io::BufRead;
use std::io::BufReader;
use std::collections::HashSet;
use std::fs::File;

use enemy::Enemy;
use util::*;
use World;

#[derive(Debug, Default)]
pub struct Scheduler {
    work_queue: BinaryHeap<BeatAction>,
}

#[derive(Debug)]
struct ParseState {
    section_start: u32, // The starting measure number of this section
    section_end: u32, // The ending measure number of this section
    measure_frequency: u32, // How often to apply the event
    beat_frequency: BeatSet // *Which beats* to apply the event
}

impl ParseState {
    fn beats(&self) -> Vec<Beat> {
        let mut beats = vec![];
        for measure in (self.section_start..self.section_end).step_by(self.measure_frequency as usize) {
            for beat in self.beat_frequency.beats.iter() {
                beats.push(Beat {
                    beat: measure * 4 + beat.beat - 1,
                    offset: beat.offset,
                });
            }
        }
        println!("{:?}", self.beat_frequency);
        beats
    }
}

impl Default for ParseState {
    fn default() -> Self {
        ParseState {
            section_start: 0,
            section_end: 0,
            beat_frequency: BeatSet::new(vec![1, 2, 3, 4].iter()),
            measure_frequency: 1,
        }
    }
}

#[derive(Debug)]
struct BeatSet {
    beats: HashSet<Beat>
    // TODO: more stuff??
}

impl BeatSet {
    fn new<'a>(iter: impl Iterator<Item=&'a u32>) -> BeatSet {
        let mut beats: HashSet<Beat> = Default::default(); 
        for quarter_note in iter {
            beats.insert(Beat {
                beat: *quarter_note, offset: 0,
            });
        }
        BeatSet {
            beats: beats
        }
    }
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


    pub fn read_file(file: File) -> Scheduler {
        let mut scheduler: Scheduler = Default::default();
        let mut parse_state: ParseState = Default::default(); 
        let reader = BufReader::new(&file);

        for (i, line) in reader.lines().enumerate() {
            if let Ok(line) = line {
                let line = line.trim();
                if line.starts_with("#") {
                    continue;
                }
                let mut line: Vec<_> = line.split_whitespace().collect();
                match line[..] {
                    ["section", start, end] => {
                        // Reset event frequency changes & update section times
                        parse_state.section_start = start.parse().unwrap();
                        parse_state.section_end = end.parse().unwrap();
                        parse_state.measure_frequency = 1;
                        parse_state.beat_frequency = BeatSet::new(vec![1, 2, 3, 4].iter());
                        },
                    ["on", ref rest..] => parse_on_keyword(&mut parse_state, rest),
                    ["spawn", spawn, "spread", spread] => {
                        // Spawn the appropriate enemy and push it into the queue
                        let action = SpawnEnemy {
                            num: spawn.parse().unwrap(),
                            spread: spread.parse().unwrap(),
                            duration: Beat {beat: 4, offset: 0},
                        };
                        for beat in parse_state.beats() {
                            scheduler.work_queue.push(BeatAction {
                                beat: Reverse(beat),
                                action: Box::new(action),
                            });
                        }
                    }
                    ["rest"] => (),
                    ["end"] => break,
                    ref x => panic!("unexpected line in map file: {:?} (line {})", x, i),
                }
            } else {
                break;
            }
        }
        scheduler
    }
}

/// Update the measure and beat frequencies based on a sliced string
/// Format: ["measure", freq, "beat", which_beats_to_apply]
fn parse_on_keyword(parse_state: &mut ParseState, measure_beat_frequency: &[&str]) {
    let beat_index = measure_beat_frequency.iter().position(|&e| e == "beat").unwrap();
    let (measure_frequency, beat_frequency) = measure_beat_frequency.split_at(beat_index);
    parse_state.measure_frequency = if measure_frequency[1] == "*" {
        1
    } else {
        measure_frequency[1].parse().unwrap()
    };
    
    let beat_frequency = beat_frequency.get(1..).unwrap();
    let mut beat_numbers: Vec<u32> = vec![];
    if beat_frequency[0] == "*" {
        beat_numbers = vec![1, 2, 3, 4]
    } else {
        for beat_number in beat_frequency {
            beat_numbers.push(beat_number.parse().unwrap())
        }
    }

    parse_state.beat_frequency = BeatSet::new(beat_numbers.iter());
    
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
    duration: Beat,
}

impl Action for SpawnEnemy {
    fn preform(&self, world: &mut World) {
        for _ in 0..self.num {
            let start_pos = rand_edge(world.grid.grid_size);
            let end_pos = rand_around(world.grid.grid_size, world.player.goal, self.spread);
            let mut enemy = Enemy::new(start_pos, end_pos, self.duration.into());
            enemy.on_spawn(world.beat_time.into());
            world.enemies.push(enemy);
        }
    }
}
