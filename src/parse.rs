use std::cmp::{Ordering, Reverse};
use std::collections::HashSet;
use std::collections::{binary_heap::PeekMut, BinaryHeap};
use std::f32::consts::PI;
use std::fmt;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;

use enemy;
use enemy::{Bullet, Enemy, Laser};
use time::{Beat, Time};
use util;
use World;

#[derive(Debug, Default)]
pub struct Scheduler {
    work_queue: BinaryHeap<BeatAction>,
}

#[derive(Debug)]
struct ParseState {
    section_start: u32,      // The starting measure number of this section
    section_end: u32,        // The ending measure number of this section
    measure_frequency: u32,  // How often to apply the event
    beat_frequency: BeatSet, // *Which beats* to apply the event
}

impl ParseState {
    /// Return a set of Beats based on the current measure/beat frequency and section.
    /// Note that these are absolute offsets from the start of the song.
    fn beats(&self) -> Vec<Beat> {
        let mut beats = vec![];
        for measure in
            (self.section_start..self.section_end).step_by(self.measure_frequency as usize)
        {
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
    beats: HashSet<Beat>, // TODO: more stuff??
}

impl BeatSet {
    /// Construct a new BeatSet from the iteratior. All will have offset 0.
    fn new<'a>(iter: impl Iterator<Item = &'a u32>) -> BeatSet {
        let mut beats: HashSet<Beat> = Default::default();
        for quarter_note in iter {
            beats.insert(Beat {
                beat: *quarter_note,
                offset: 0,
            });
        }
        BeatSet { beats: beats }
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
                    }
                    ["on", ref rest..] => parse_on_keyword(&mut parse_state, rest),
                    ["spawn", spawn, "spread", spread] => {
                        // Spawn the appropriate bullet and push it into the queue
                        let action = SpawnBullet {
                            num: spawn.parse().unwrap(),
                            spread: spread.parse().unwrap(),
                            duration: Beat { beat: 4, offset: 0 },
                        };
                        for beat in parse_state.beats() {
                            scheduler.work_queue.push(BeatAction {
                                beat: Reverse(beat),
                                action: Box::new(action),
                            });
                        }
                    }
                    ["laser", "spread", spread] => {
                        let action = SpawnLaser {
                            spread: spread.parse().unwrap(),
                        };
                        if spread.parse::<isize>().is_err() {
                            panic!("Expected integer, got {:?}", spread)
                        }
                        for beat in parse_state.beats() {
                            let beat = beat - enemy::LASER_PREDELAY_BEATS;
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
    let beat_index = measure_beat_frequency
        .iter()
        .position(|&e| e == "beat")
        .unwrap();
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

/// An action makes some modification to the world.
pub trait Action {
    fn preform(&self, world: &mut World, time: &Time);
}

impl fmt::Debug for Action {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Action")
    }
}

#[derive(Clone, Copy)]
pub struct SpawnLaser {
    spread: isize,
}

impl Action for SpawnLaser {
    fn preform(&self, world: &mut World, time: &Time) {
        let mut laser = Laser::new_through_point(
            util::rand_around(world.grid.grid_size, world.player.position(), self.spread),
            util::gen_range(0, 6) as f32 * (PI / 6.0),
            0.4,
            1.0,
        );
        laser.on_spawn(time.f64_time());
        world.enemies.push(Box::new(laser));
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
            let start_pos = util::rand_edge(world.grid.grid_size);
            let end_pos =
                util::rand_around(world.grid.grid_size, world.player.position(), self.spread);
            let mut bullet = Bullet::new(start_pos, end_pos, self.duration.into());
            bullet.on_spawn(time.f64_time());
            world.enemies.push(Box::new(bullet));
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
    assert_eq!(
        parse_state.beat_frequency,
        BeatSet::new(vec![1, 2, 3, 4].iter())
    );
}

#[test]
fn test_new_beat_set() {
    let beat_set1 = BeatSet::new(vec![1, 2, 3, 4].iter());
    let mut beat_set2: BeatSet = Default::default();
    beat_set2.beats.insert(Beat { beat: 1, offset: 0 });
    beat_set2.beats.insert(Beat { beat: 2, offset: 0 });
    beat_set2.beats.insert(Beat { beat: 3, offset: 0 });
    beat_set2.beats.insert(Beat { beat: 4, offset: 0 });
    assert_eq!(beat_set1, beat_set2);
}
