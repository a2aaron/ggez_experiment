use std::cmp::{Ordering, Reverse};
use std::collections::{binary_heap::PeekMut, BinaryHeap};
use std::fmt;
use std::io::BufRead;
use std::io::BufReader;
use std::collections::HashSet;
use std::fs::File;
use std::time::{Duration, Instant};

use ggez::timer;

use enemy::Bullet;
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
    /// Return a set of Beats based on the current measure/beat frequency and section.
    /// Note that these are absolute offsets from the start of the song.
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

/// Convience struct for a set of beats.
#[derive(Debug, Default, PartialEq, Eq)]
struct BeatSet {
    beats: HashSet<Beat>
    // TODO: more stuff??
}

impl BeatSet {
    /// Construct a new BeatSet from the iteratior. All will have offset 0.
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
    /// Preform the scheduled actions up to the new beat_time
    /// Note that this will execute every action since the last beat_time and
    /// current beat_time.
    pub fn update(&mut self, time: &Time, world: &mut World) {
        let rev_beat = Reverse(time.beat_time());
        loop {
            match self.work_queue.peek_mut() {
                Some(peaked) => {
                    if (*peaked).beat > rev_beat {
                        PeekMut::pop(peaked).action.preform(world, time)
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
                        // Spawn the appropriate bullet and push it into the queue
                        let action = SpawnBullet {
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
            bpm: bpm_to_duration(bpm),
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
    pub fn f64_time(&self) -> f64 {
        timer::duration_to_f64(self.current_duration) / timer::duration_to_f64(self.bpm)
    }

    /// Get the number of beats since the start
    pub fn beat_time(&self) -> Beat {
        self.f64_time().into()
    }

    /// Return a value from 0.0 to 1.0 indicating the percent through the beat we are at
    pub fn beat_percent(beat: Beat) -> f64 {
        Into::<f64>::into(beat) % 1.0
    }
}

/// A wrapper struct of a Beat and a Boxed Action. The beat has reversed ordering
/// to allow for the Scheduler to actually get the latest beat times.
#[derive(Debug)]
struct BeatAction {
    // Stored in reverse ordering so that we can get the _earliest_ beat when in
    // the scheduler, rather than the latest.
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
    pub beat: u32,
    pub offset: u8, // offset from the beat, in 1/256th increments
}

/// Beat time is scaled such that 1.0 = 1 beat and 1.5 = 1 beat 128 offset, etc
impl From<f64> for Beat {
    fn from(beat_time: f64) -> Self {
        Beat {
            beat: beat_time as u32,
            offset: (beat_time.fract() * 256.0) as u8,
        }
    }
}

/// Similar to From. 1.0f64 = 1 beat 0 offset
impl Into<f64> for Beat {
    fn into(self) -> f64 {
        self.beat as f64 + (self.offset as f64) / 256.0
    }
}

/// An action makes some modification to the world.
pub trait Action {
    fn preform(&self, world: &mut World, time: &Time);
}

impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Action")
    }
}

/// An Action which adds `num` bullets around the player, with some random factor
#[derive(Clone, Copy)]
pub struct SpawnBullet {
    num: usize,
    spread: isize,
    duration: Beat,
}

impl Action for SpawnBullet {
    fn preform(&self, world: &mut World, time: &Time) {
        for _ in 0..self.num {
            let start_pos = rand_edge(world.grid.grid_size);
            let end_pos = rand_around(world.grid.grid_size, world.player.position(), self.spread);
            let mut bullet = Bullet::new(start_pos, end_pos, self.duration.into());
            bullet.on_spawn(time.f64_time().into());
            world.enemies.push(bullet);
        }
    }
}

#[test]
fn test_parse_on_keyword() {
    let mut parse_state: ParseState = Default::default();
    parse_on_keyword(&mut parse_state, &["measure", "420", "beat", "6", "9"]);
    assert_eq!(parse_state.measure_frequency, 420);
    assert_eq!(parse_state.beat_frequency, BeatSet::new(vec![6, 9].iter()));

    let mut parse_state: ParseState = Default::default();
    parse_on_keyword(&mut parse_state, &["measure", "*", "beat", "*"]);
    assert_eq!(parse_state.measure_frequency, 1);
    assert_eq!(parse_state.beat_frequency, BeatSet::new(vec![1, 2, 3, 4].iter()));
}

#[test]
fn test_new_beat_set() {
    let beat_set1 = BeatSet::new(vec![1, 2, 3, 4].iter());
    let mut beat_set2: BeatSet = Default::default();
    beat_set2.beats.insert(Beat {beat: 1, offset: 0});
    beat_set2.beats.insert(Beat {beat: 2, offset: 0});
    beat_set2.beats.insert(Beat {beat: 3, offset: 0});
    beat_set2.beats.insert(Beat {beat: 4, offset: 0});
    assert_eq!(beat_set1, beat_set2);
}

#[test]
fn test_beat_to_f64() {
    assert_eq!(1.0f64, Beat { beat: 1, offset: 0}.into());
    assert_eq!(2.0f64 + 1.0/256.0 as f64, Beat { beat: 2, offset: 1}.into());
    assert_eq!(3.5f64, Beat { beat: 3, offset: 128}.into());
    assert_eq!(4.25f64, Beat { beat: 4, offset: 64}.into());
}

#[test]
fn test_f64_to_beat() {
    assert_eq!(Beat::from(1.0f64), Beat { beat: 1, offset: 0});
    assert_eq!(Beat::from(2.0f64 + 1.0/256.0 as f64), Beat { beat: 2, offset: 1});
    assert_eq!(Beat::from(3.5f64), Beat { beat: 3, offset: 128});
    assert_eq!(Beat::from(4.25f64), Beat { beat: 4, offset: 64});
}
