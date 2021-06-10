use std::cmp::{Ordering, Reverse};
use std::collections::binary_heap::PeekMut;
use std::collections::{BinaryHeap, HashSet};
use std::f32::consts::PI;
use std::fs::read_to_string;
use std::path::Path;

use enemy::{Bullet, Enemy, Laser};
use time::{Beat, Time};
use {enemy, util, World};

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

    fn new(section_start: u32, section_end: u32) -> Self {
        ParseState {
            section_start,
            section_end,
            measure_frequency: 1,
            beat_frequency: BeatSet::new(vec![1, 2, 3, 4].iter()),
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
        BeatSet { beats }
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
        let sections = parse(split_lines(&read_to_string(path).unwrap()));
        for section in sections {
            scheduler.work_queue.extend(compile_section(section));
        }
        scheduler
    }
}

/// Read in a file, returing a list of strings split by whitespace
/// Lines starting with # are removed as comments
fn split_lines(text: &str) -> Vec<Vec<&str>> {
    let mut lines = vec![];
    for line in text.lines() {
        let line = line.trim();
        // Remove comments
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let line = line.split_whitespace().collect();
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
    SpawnObject(SpawnCmd),
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
    let mut line_number = 1;

    let mut first_section = true;

    let mut start_measure = 0;
    let mut end_measure = 0;
    let mut commands = vec![];

    for next_line in lines.iter() {
        use self::Commands::*;
        use self::SpawnCmd::*;
        match next_line[..] {
            ["section", start, end] => {
                // Skip pushing the very first section to a new section because
                // we haven't actually read the section yet.
                if first_section {
                    first_section = false
                } else {
                    sections.push(Section {
                        start_measure,
                        end_measure,
                        commands,
                    });
                }
                commands = vec![];
                start_measure = start.parse().unwrap();
                end_measure = end.parse().unwrap();
            }
            ["on", ref rest @ ..] => {
                commands.push(parse_on_keyword(rest));
            }
            ["spawn", spawn, "spread", spread] => {
                commands.push(SpawnObject(SpawnBullet {
                    num: spawn.parse().unwrap(),
                    spread: spread.parse().unwrap(),
                    duration: enemy::BULLET_DURATION_BEATS,
                }));
            }
            ["laser", "spread", spread] => {
                commands.push(SpawnObject(SpawnLaser {
                    spread: spread.parse().unwrap(),
                }));
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
        start_measure,
        end_measure,
        commands,
    });
    sections
}

/// Update the measure and beat frequencies based on a sliced string
/// Format: ["measure", freq, "beat", which_beats_to_apply]
fn parse_on_keyword(measure_beat_frequency: &[&str]) -> Commands {
    println!("{:?}", measure_beat_frequency);
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
        measure_frequency,
        beat_frequency: beat_numbers,
    }
}

fn compile_section(section: Section) -> BinaryHeap<BeatAction> {
    use self::Commands::*;
    let mut parse_state = ParseState::new(section.start_measure, section.end_measure);
    let mut beat_actions = BinaryHeap::new();
    for cmd in section.commands {
        match cmd {
            SpawnObject(spawn_cmd) => {
                for beat in parse_state.beats() {
                    let beat_action = match spawn_cmd {
                        SpawnCmd::SpawnLaser { .. } => BeatAction {
                            beat: Reverse(beat - enemy::LASER_PREDELAY_BEATS),
                            action: spawn_cmd,
                        },
                        _ => BeatAction {
                            beat: Reverse(beat),
                            action: spawn_cmd,
                        },
                    };
                    beat_actions.push(beat_action)
                }
            }
            CmdFrequency {
                measure_frequency,
                beat_frequency,
            } => {
                parse_state.measure_frequency = measure_frequency;
                parse_state.beat_frequency = BeatSet::new(beat_frequency.iter());
            }
            Skip => {
                unimplemented!();
            }
            Rest => {
                continue;
            }
            End => {
                break;
            }
        }
    }
    beat_actions
}

/// A wrapper struct of a Beat and a Boxed Action. The beat has reversed ordering
/// to allow for the Scheduler to actually get the latest beat times.
#[derive(Debug)]
struct BeatAction {
    // Stored in reverse ordering so that we can get the _earliest_ beat when in
    // the scheduler, rather than the latest.
    beat: Reverse<Beat>, // for the binary heap's ordering
    action: SpawnCmd,
}

impl BeatAction {
    // Return a dummy BeatAction that only holds a beat
    fn dummy(beat: u32, offset: u8) -> BeatAction {
        BeatAction {
            beat: Reverse(Beat { beat, offset }),
            action: SpawnCmd::DummyCmd,
        }
    }
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
pub trait Action: Eq {
    fn preform(&self, world: &mut World, time: &Time);
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
enum SpawnCmd {
    /// An Action which adds `num` bullets around the player, with some random factor
    SpawnBullet {
        num: usize,
        spread: isize,
        duration: Beat,
    },
    SpawnLaser {
        spread: isize,
    },
    /// Does nothing, good for testing
    DummyCmd,
}

impl SpawnCmd {
    fn preform(&self, world: &mut World, time: &Time) {
        use self::SpawnCmd::*;
        match *self {
            SpawnBullet {
                num,
                spread,
                duration,
            } => {
                for _ in 0..num {
                    let start_pos = util::rand_edge(world.grid.grid_size);
                    let end_pos =
                        util::rand_around(world.grid.grid_size, world.player.position(), spread);
                    let mut bullet = Bullet::new(start_pos, end_pos, duration.into());
                    bullet.on_spawn(time.f64_time());
                    world.enemies.push(Box::new(bullet));
                }
            }
            SpawnLaser { spread } => {
                let mut laser = Laser::new_through_point(
                    util::rand_around(world.grid.grid_size, world.player.position(), spread),
                    util::gen_range(0, 6) as f32 * (PI / 6.0),
                    enemy::LASER_DURATION,
                );
                laser.on_spawn(time.f64_time());
                world.enemies.push(Box::new(laser));
            }
            DummyCmd => {} // Do nothing, this is a dummy
        }
    }
}

#[test]
fn test_compile_empty_section() {
    let section = Section {
        start_measure: 0,
        end_measure: 4,
        commands: vec![],
    };
    let expected = BinaryHeap::<BeatAction>::new();
    assert_eq!(
        expected.into_sorted_vec(),
        compile_section(section).into_sorted_vec()
    );
}

#[test]
fn test_compile_spawn_simple() {
    use self::Commands::*;
    use self::SpawnCmd::*;

    let section = Section {
        start_measure: 0,
        end_measure: 1,
        commands: vec![SpawnObject(DummyCmd)],
    };
    let expected: Vec<BeatAction> = vec![
        BeatAction::dummy(0, 0),
        BeatAction::dummy(1, 0),
        BeatAction::dummy(2, 0),
        BeatAction::dummy(3, 0),
    ];
    let expected: BinaryHeap<_> = expected.into_iter().collect();
    assert_eq!(
        expected.into_sorted_vec(),
        compile_section(section).into_sorted_vec()
    );
}

#[test]
fn test_compile_laser_predelay() {
    use self::Commands::*;
    use self::SpawnCmd::*;

    let section = Section {
        start_measure: 2,
        end_measure: 3,
        commands: vec![SpawnObject(SpawnLaser { spread: 1 })],
    };
    let expected: Vec<BeatAction> = vec![
        BeatAction {
            beat: Reverse(
                Beat {
                    beat: 2 * 4,
                    offset: 0,
                } - enemy::LASER_PREDELAY_BEATS,
            ),
            action: SpawnLaser { spread: 1 },
        },
        BeatAction {
            beat: Reverse(
                Beat {
                    beat: 2 * 4 + 1,
                    offset: 0,
                } - enemy::LASER_PREDELAY_BEATS,
            ),
            action: SpawnLaser { spread: 1 },
        },
        BeatAction {
            beat: Reverse(
                Beat {
                    beat: 2 * 4 + 2,
                    offset: 0,
                } - enemy::LASER_PREDELAY_BEATS,
            ),
            action: SpawnLaser { spread: 1 },
        },
        BeatAction {
            beat: Reverse(
                Beat {
                    beat: 2 * 4 + 3,
                    offset: 0,
                } - enemy::LASER_PREDELAY_BEATS,
            ),
            action: SpawnLaser { spread: 1 },
        },
    ];
    let expected: BinaryHeap<_> = expected.into_iter().collect();
    assert_eq!(
        expected.into_sorted_vec(),
        compile_section(section).into_sorted_vec()
    );
}

#[test]
fn test_compile_beat_freq() {
    use std::iter::FromIterator;

    use self::Commands::*;
    use self::SpawnCmd::*;

    let section = Section {
        start_measure: 0,
        end_measure: 4,
        commands: vec![
            CmdFrequency {
                measure_frequency: 2,
                beat_frequency: vec![1, 3],
            },
            SpawnObject(DummyCmd),
        ],
    };
    let expected: Vec<BeatAction> = vec![
        BeatAction::dummy(0, 0),
        BeatAction::dummy(2, 0),
        BeatAction::dummy(4 * 2, 0),
        BeatAction::dummy(4 * 2 + 2, 0),
    ];
    let expected: BinaryHeap<_> = expected.into_iter().collect();
    assert_eq!(
        expected.into_sorted_vec(),
        compile_section(section).into_sorted_vec()
    );
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
fn test_trim_whitespace() {
    let string =
        "   \t\t   section    6   9 \n \n   \n \n\n       measure  *     beat   *    \n \n  \n \n ";
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
    use self::SpawnCmd::*;
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
            SpawnObject(SpawnBullet {
                num: 4,
                spread: 8,
                duration: Beat { beat: 4, offset: 0 },
            }),
            SpawnObject(SpawnLaser { spread: 8 }),
            Rest,
            End,
        ],
    }];
    assert_eq!(sections, expected);
}

#[test]
fn test_parse_many_sections() {
    use self::Commands::*;
    use self::SpawnCmd::*;
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
                SpawnObject(SpawnBullet {
                    num: 1,
                    spread: 1,
                    duration: Beat { beat: 4, offset: 0 },
                }),
                SpawnObject(SpawnLaser { spread: 1 }),
            ],
        },
        Section {
            start_measure: 4,
            end_measure: 8,
            commands: vec![
                SpawnObject(SpawnBullet {
                    num: 2,
                    spread: 2,
                    duration: Beat { beat: 4, offset: 0 },
                }),
                SpawnObject(SpawnLaser { spread: 2 }),
            ],
        },
        Section {
            start_measure: 8,
            end_measure: 16,
            commands: vec![
                SpawnObject(SpawnBullet {
                    num: 3,
                    spread: 3,
                    duration: Beat { beat: 4, offset: 0 },
                }),
                SpawnObject(SpawnLaser { spread: 3 }),
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
