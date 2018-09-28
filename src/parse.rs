use std::cmp::{Ordering, Reverse};
use std::collections::HashSet;
use std::collections::{binary_heap::PeekMut, BinaryHeap};
use std::f32::consts::PI;
use std::fmt;
use std::fs::read_to_string;
use std::path::Path;

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
    pub fn read_file(path: &Path) -> Scheduler {
        let mut scheduler: Scheduler = Default::default();
        let mut parse_state: ParseState = Default::default();
        let tokens = parse(split_lines(&read_to_string(path).unwrap()));
        unimplemented!();
    }
}

/// Read in a file, returing a list of strings split by whitespace
/// Lines starting with # are removed as comments
fn split_lines<'a>(text: &'a str) -> Vec<Vec<&'a str>> {
    let mut lines = vec![];
    for line in text.split("\n") {
        let line = line.trim();
        // Remove comments
        if line.starts_with("#") {
            continue;
        }
        let mut line = line.split_whitespace().collect();
        lines.push(line);
    }
    lines
}

#[derive(Debug, PartialEq, Eq)]
struct Section {
    start_measure: u32,
    end_measure: u32,
    commands: Vec<Commands>,
}

#[derive(Debug, PartialEq, Eq)]
enum Commands {
    SpawnBullet {
        spawn: u32,
        spread: u32,
        duration: Beat,
    },
    SpawnLaser {
        spread: u32,
    },
    CmdFrequency {
        measure_frequency: u32,
        beat_frequency: Vec<u32>,
    },
    Skip,
    Rest,
    End,
}

/// Convert a bunch of parsed strings to actual tokens, split by section
fn parse(lines: Vec<Vec<&str>>) -> Vec<Section> {
    let mut sections = vec![];
    let mut lines = lines.iter();
    let mut line_number = 1;

    let mut first_section = true;

    let mut start_measure = 0;
    let mut end_measure = 0;
    let mut commands = vec![];

    while let Some(next_line) = lines.next() {
        use self::Commands::*;
        match next_line[..] {
            ["section", start, end] => {
                // Skip pushing the very first section to a new section because
                // we haven't actually read the section yet.
                if first_section {
                    first_section = false
                } else {
                    sections.push(Section {
                        start_measure: start_measure,
                        end_measure: end_measure,
                        commands: commands,
                    });
                }
                commands = vec![];
                start_measure = start.parse().unwrap();
                end_measure = end.parse().unwrap();
            }
            ["on", ref rest..] => {
                commands.push(parse_on_keyword(rest));
            }
            ["spawn", spawn, "spread", spread] => {
                commands.push(SpawnBullet {
                    spawn: spawn.parse().unwrap(),
                    spread: spread.parse().unwrap(),
                    duration: Beat { beat: 4, offset: 0 },
                });
            }
            ["laser", "spread", spread] => {
                commands.push(SpawnLaser {
                    spread: spread.parse().unwrap(),
                });
            }
            ["rest"] => commands.push(Rest),
            ["end"] => commands.push(End),
            ref x => panic!(
                "unexpected line in map file: {:?} (line {})",
                x, line_number
            ),
        }
        line_number += 1;
    }
    // Push the last section (there won't be another section token to read, but
    // we need to push it anyways
    sections.push(Section {
        start_measure: start_measure,
        end_measure: end_measure,
        commands: commands,
    });
    sections
}

/// Update the measure and beat frequencies based on a sliced string
/// Format: ["measure", freq, "beat", which_beats_to_apply]
fn parse_on_keyword(measure_beat_frequency: &[&str]) -> Commands {
    let beat_index = measure_beat_frequency
        .iter()
        .position(|&e| e == "beat")
        .unwrap();
    let (measure_frequency, beat_frequency) = measure_beat_frequency.split_at(beat_index);
    let measure_frequency = if measure_frequency[1] == "*" {
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
    Commands::CmdFrequency {
        measure_frequency: measure_frequency,
        beat_frequency: beat_numbers,
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
fn test_split_lines() {
    let string = "section 6 9\nmeasure * beat *";
    let split = split_lines(string);
    assert_eq!(
        vec![vec!["section", "6", "9"], vec!["measure", "*", "beat", "*"]],
        split
    );
}

#[test]
fn test_split_lines_comments() {
    let string = "section 6 9\n# I'm a comment!\nmeasure * beat *";
    let split = split_lines(string);
    assert_eq!(
        vec![vec!["section", "6", "9"], vec!["measure", "*", "beat", "*"]],
        split
    );
}

#[test]
fn test_parse_on_keyword() {
    let command = parse_on_keyword(&["measure", "420", "beat", "6", "9"]);
    assert_eq!(
        command,
        Commands::CmdFrequency {
            measure_frequency: 420,
            beat_frequency: vec![6, 9]
        }
    );

    let command = parse_on_keyword(&["measure", "*", "beat", "*"]);
    assert_eq!(
        command,
        Commands::CmdFrequency {
            measure_frequency: 1,
            beat_frequency: vec![1, 2, 3, 4]
        }
    );
}

#[test]
fn test_parse_trival() {
    let sections = parse(vec![vec!["section", "0", "1"]]);
    assert_eq!(
        sections,
        vec![Section {
            start_measure: 0,
            end_measure: 1,
            commands: vec![],
        }]
    )
}

#[test]
fn test_parse_empty_sections() {
    let sections = parse(vec![
        vec!["section", "0", "4"],
        vec!["section", "4", "8"],
        vec!["section", "8", "12"],
        vec!["section", "12", "16"],
    ]);
    let expected = vec![
        Section {
            start_measure: 0,
            end_measure: 4,
            commands: vec![],
        },
        Section {
            start_measure: 4,
            end_measure: 8,
            commands: vec![],
        },
        Section {
            start_measure: 8,
            end_measure: 12,
            commands: vec![],
        },
        Section {
            start_measure: 12,
            end_measure: 16,
            commands: vec![],
        },
    ];
    assert_eq!(sections, expected);
}

#[test]
fn test_parse_simple() {
    use self::Commands::*;
    let sections = parse(vec![
        vec!["section", "0", "4"],
        vec!["spawn", "4", "spread", "8"],
        vec!["laser", "spread", "8"],
        vec!["rest"],
        vec!["end"],
    ]);
    let expected = vec![Section {
        start_measure: 0,
        end_measure: 4,
        commands: vec![
            SpawnBullet {
                spawn: 4,
                spread: 8,
                duration: Beat { beat: 4, offset: 0 },
            },
            SpawnLaser { spread: 8 },
            Rest,
            End,
        ],
    }];
    assert_eq!(sections, expected);
}

#[test]
fn test_parse_many_sections() {
    use self::Commands::*;
    let sections = parse(vec![
        vec!["section", "0", "4"],
        vec!["spawn", "1", "spread", "1"],
        vec!["laser", "spread", "1"],
        vec!["section", "4", "8"],
        vec!["spawn", "2", "spread", "2"],
        vec!["laser", "spread", "2"],
        vec!["section", "8", "16"],
        vec!["spawn", "3", "spread", "3"],
        vec!["laser", "spread", "3"],
    ]);
    let expected = vec![
        Section {
            start_measure: 0,
            end_measure: 4,
            commands: vec![
                SpawnBullet {
                    spawn: 1,
                    spread: 1,
                    duration: Beat { beat: 4, offset: 0 },
                },
                SpawnLaser { spread: 1 },
            ],
        },
        Section {
            start_measure: 4,
            end_measure: 8,
            commands: vec![
                SpawnBullet {
                    spawn: 2,
                    spread: 2,
                    duration: Beat { beat: 4, offset: 0 },
                },
                SpawnLaser { spread: 2 },
            ],
        },
        Section {
            start_measure: 8,
            end_measure: 16,
            commands: vec![
                SpawnBullet {
                    spawn: 3,
                    spread: 3,
                    duration: Beat { beat: 4, offset: 0 },
                },
                SpawnLaser { spread: 3 },
            ],
        },
    ];
    assert_eq!(sections, expected);
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
