/// This module handles the playing of charts, which define what happens during
/// a particular song.
use std::cmp::{Ordering, Reverse};
use std::collections::binary_heap::PeekMut;
use std::collections::BinaryHeap;

use ggez::Context;

use crate::ease::Lerp;
use crate::enemy::{Bullet, CircleBomb, Enemy, Laser, BOMB_WARMUP, LASER_WARMUP};
use crate::parse::SongMap;
use crate::time::Beats;
use crate::util;
use crate::world::WorldPos;

/// This struct contains all the events that occur during a song. It will perform
/// a set of events every time update is called.
#[derive(Debug, Default)]
pub struct Scheduler {
    work_queue: BinaryHeap<BeatAction>,
}

impl Scheduler {
    pub fn new(_ctx: &mut Context, song_map: &SongMap) -> Scheduler {
        let kick1laser = song_map.get_beats("kick1laser");
        let kick1bomb = song_map.get_beats("kick1bomb");
        let kick1solo = song_map.get_beats("kick1solo");

        let song_map_actions: Vec<BeatAction> = song_map
            .cmd_batches
            .iter()
            .map(|(splitter, cmd_batch)| splitter.make_actions(*cmd_batch))
            .flatten()
            .collect();

        let stage = [
            // DROP 20 - 23
            make_actions(20.0 * 4.0, &kick1laser, |_, _| CmdBatch::Laser {
                a: CmdBatchPos::player(),
                b: CmdBatchPos::origin(),
            }),
            make_actions(20.0 * 4.0, &kick1bomb, |_, _| CmdBatch::CircleBomb {
                pos: CmdBatchPos::RandomGrid,
            }),
            // INSTANT 103 (measure 26 beat 3-4)
            make_actions_custom(20.0 * 4.0, &kick1solo, |i, _| {
                fn vert_laser(x: f64) -> SpawnCmd {
                    const HALF_PI: f64 = std::f64::consts::PI / 2.0;
                    SpawnCmd::Laser {
                        angle: HALF_PI,
                        position: LiveWorldPos::from((x, 0.0)),
                    }
                }

                let sign = match i % 2 {
                    0 => -1.0,
                    _ => 1.0,
                };
                vec![
                    vert_laser(sign * 50.0),
                    vert_laser(sign * 40.0),
                    vert_laser(sign * 30.0),
                ]
            }),
        ];
        Scheduler {
            work_queue: stage
                .iter()
                .flatten()
                .cloned()
                .chain(song_map_actions)
                .collect::<BinaryHeap<_>>(),
        }
    }

    /// Preform the scheduled actions up to the new beat_time
    /// Note that this will execute every action since the last beat_time and
    /// current beat_time.
    pub fn update(&mut self, time: Beats, enemies: &mut Vec<Box<dyn Enemy>>, player_pos: WorldPos) {
        let rev_beat = Reverse(time);
        loop {
            match self.work_queue.peek_mut() {
                Some(peaked) => {
                    if (*peaked).start_time > rev_beat {
                        let beat_action = PeekMut::pop(peaked);

                        beat_action
                            .action
                            .preform(beat_action.start_time.0, enemies, player_pos)
                    } else {
                        return;
                    }
                }
                None => return,
            }
        }
    }
}

fn make_actions(
    start: f64,
    beats: &[Beats],
    batcher: impl Fn(usize, f64) -> CmdBatch,
) -> Vec<BeatAction> {
    mark_beats(start, beats)
        .iter()
        .enumerate()
        .map(|(i, (beat, t))| {
            let action = batcher(i, *t).get(*t);
            BeatAction::new(*beat, action)
        })
        .collect()
}

fn make_actions_custom(
    start: f64,
    beats: &[Beats],
    spawner: impl Fn(usize, f64) -> Vec<SpawnCmd>,
) -> Vec<BeatAction> {
    mark_beats(start, beats)
        .iter()
        .enumerate()
        .map(|(i, (beat, t))| {
            spawner(i, *t)
                .iter()
                .map(|cmd| BeatAction::new(*beat, *cmd))
                .collect::<Vec<BeatAction>>()
        })
        .flatten()
        .collect()
}

/// Split a length of time into a number of individual beats. This is useful for
/// doing something a number of times in a row.
#[derive(Debug, Clone, Copy)]
pub struct BeatSplitter {
    pub start: f64,
    pub duration: f64,
    pub frequency: f64,
    // Amount to shift all beats. This does effect the `t` value returned by split
    pub offset: f64,
    // Amount to shift all beats. This does not effect the `t` value returned by split
    pub delay: f64,
}

impl Default for BeatSplitter {
    fn default() -> BeatSplitter {
        BeatSplitter {
            start: 0.0,
            duration: 4.0 * 4.0, // 4 measures total
            frequency: 4.0,
            offset: 0.0,
            delay: 0.0,
        }
    }
}

impl BeatSplitter {
    fn with_start(self, start: f64) -> Self {
        BeatSplitter {
            start,
            duration: self.duration,
            frequency: self.frequency,
            offset: self.offset,
            delay: self.delay,
        }
    }

    fn with_offset(self, offset: f64) -> Self {
        BeatSplitter {
            start: self.start,
            duration: self.duration,
            frequency: self.frequency,
            offset,
            delay: self.delay,
        }
    }

    fn with_delay(self, delay: f64) -> Self {
        BeatSplitter {
            start: self.start,
            duration: self.duration,
            frequency: self.frequency,
            offset: self.offset,
            delay,
        }
    }

    #[allow(dead_code)]
    fn with_duration(self, duration: f64) -> Self {
        BeatSplitter {
            start: self.start,
            duration,
            frequency: self.frequency,
            offset: self.offset,
            delay: self.delay,
        }
    }

    fn split(&self) -> Vec<(Beats, f64)> {
        let mut beats = vec![];
        let mut this_beat = self.start;
        while self.duration > this_beat - self.start {
            beats.push((
                Beats(this_beat + self.delay + self.offset),
                (this_beat + self.offset - self.start) / self.duration,
            ));
            this_beat += self.frequency;
        }
        beats
    }

    fn make_actions(&self, cmd_batch: CmdBatch) -> Vec<BeatAction> {
        self.split()
            .iter()
            .enumerate()
            .map(|(_, (start_time, t))| BeatAction::new(*start_time, cmd_batch.get(*t)))
            .collect()
    }

    #[allow(dead_code)]
    fn make_actions_custom(
        &self,
        spawner: impl Fn(usize, f64) -> Vec<SpawnCmd>,
    ) -> Vec<BeatAction> {
        self.split()
            .iter()
            .enumerate()
            .map(|(i, (start_time, t))| {
                let actions: Vec<BeatAction> = spawner(i, *t)
                    .iter()
                    .map(|action| BeatAction::new(*start_time, *action))
                    .collect();
                actions
            })
            .flatten()
            .collect()
    }
}

/// Given a vector of beats, shift every value by `start` Beats. The slice is
/// assumed to be in sorted order and the last beat is assumed to be the duration
/// of the whole slice.
fn mark_beats(start: f64, beats: &[Beats]) -> Vec<(Beats, f64)> {
    if beats.is_empty() {
        return vec![];
    }
    let mut marked_beats = vec![];
    let duration = beats.last().unwrap();
    for &beat in beats {
        marked_beats.push((Beats(start) + beat, beat.0 / duration.0))
    }
    marked_beats
}

#[derive(Debug, Clone, Copy)]
pub enum CmdBatch {
    Bullet {
        start: CmdBatchPos,
        end: CmdBatchPos,
    },
    Laser {
        a: CmdBatchPos,
        b: CmdBatchPos,
    },
    CircleBomb {
        pos: CmdBatchPos,
    },
}
impl CmdBatch {
    fn bullet(starts: ((f64, f64), (f64, f64)), ends: ((f64, f64), (f64, f64))) -> CmdBatch {
        CmdBatch::Bullet {
            start: CmdBatchPos::Lerped(starts.0, starts.1),
            end: CmdBatchPos::Lerped(ends.0, ends.1),
        }
    }

    fn bullet_player(starts: ((f64, f64), (f64, f64))) -> CmdBatch {
        CmdBatch::Bullet {
            start: CmdBatchPos::Lerped(starts.0, starts.1),
            end: CmdBatchPos::Constant(LiveWorldPos::PlayerPos),
        }
    }

    fn get(&self, t: f64) -> SpawnCmd {
        match self {
            CmdBatch::Bullet { start, end } => SpawnCmd::Bullet {
                start: start.get(t),
                end: end.get(t),
            },
            CmdBatch::Laser { a, b } => SpawnCmd::LaserThruPoints {
                a: a.get(t),
                b: b.get(t),
            },
            CmdBatch::CircleBomb { pos } => SpawnCmd::CircleBomb { pos: pos.get(t) },
        }
    }
}

#[derive(Debug, Clone, Copy)]
// A position which is static and does not depend on run time information, but
// maybe depend on the time at which is meant to exist at. This is useful for
// representing batches of objects.
pub enum CmdBatchPos {
    Lerped((f64, f64), (f64, f64)),
    Constant(LiveWorldPos),
    RandomGrid,
}

impl CmdBatchPos {
    fn player() -> CmdBatchPos {
        CmdBatchPos::Constant(LiveWorldPos::PlayerPos)
    }

    fn origin() -> CmdBatchPos {
        CmdBatchPos::Constant(LiveWorldPos::origin())
    }

    fn get(&self, t: f64) -> LiveWorldPos {
        match *self {
            CmdBatchPos::Lerped(start, end) => {
                let a = WorldPos::from(start);
                let b = WorldPos::from(end);
                LiveWorldPos::Constant(WorldPos::lerp(a, b, t))
            }
            CmdBatchPos::Constant(pos) => pos,
            CmdBatchPos::RandomGrid => {
                LiveWorldPos::from(util::random_grid((-50.0, 50.0), (-50.0, 50.0), 10))
            }
        }
    }
}

impl From<WorldPos> for CmdBatchPos {
    fn from(x: WorldPos) -> Self {
        CmdBatchPos::Constant(LiveWorldPos::Constant(x))
    }
}

/// A wrapper struct of a Beat and a Boxed Action. The beat has reversed ordering
/// to allow for the Scheduler to actually get the latest beat times.
/// When used in the Scheduler, the action is `perform`'d when `beat` occurs.
/// Note that some actions may decide to spawn things in the future. For example
/// a SpawnLazer will immediately add a laser to the list of active enemies, but
/// the laser will not activate until the time specified by the SpawnLaser
/// command. This also means that some actions may be scheduled earlier than
/// needed, and that some actions have a maximum latest time at which they can
/// get scheduled at all.
#[derive(Debug, Clone, Copy)]
struct BeatAction {
    // Stored in reverse ordering so that we can get the _earliest_ beat when in
    // the scheduler, rather than the latest.
    start_time: Reverse<Beats>, // for the binary heap's ordering
    action: SpawnCmd,
}

impl BeatAction {
    /// Create a BeatAction. The action is scheduled at time `beat` if the
    /// SpawnCmd has no start time of its own, otherwise the action is scheduled
    /// (probably slightly earlier than the SpawnCmd's start time).
    fn new(start_time: Beats, action: SpawnCmd) -> BeatAction {
        let beat = match action {
            SpawnCmd::Bullet { .. } => start_time,
            // Schedule the lasers slightly earlier than their actual time
            // so that the laser pre-delays occurs at the right time.
            // Since the laser predelay is 4 beats, but the laser constructors
            // all assume the passed time is for the active phase, if we want
            // a laser to _fire_ on beat 20, it needs to be spawned in, at latest
            // beat 16, so that it works correctly.
            SpawnCmd::Laser { .. } => start_time - LASER_WARMUP,
            SpawnCmd::LaserThruPoints { .. } => start_time - LASER_WARMUP,
            SpawnCmd::CircleBomb { .. } => start_time - BOMB_WARMUP,
        };
        BeatAction {
            start_time: Reverse(beat),
            action,
        }
    }
}

impl PartialEq for BeatAction {
    fn eq(&self, other: &Self) -> bool {
        self.start_time == other.start_time
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
        //  NOTE: We assume that weird values (like inf or NaN) never happen.
        //  and just assume that returning equal is okay here.
        self.start_time
            .partial_cmp(&other.start_time)
            .unwrap_or(Ordering::Equal)
    }
}

/// A WorldPosition which depends on some dynamic value (ex: the player's
/// position). This is computed at run time.
#[derive(Debug, Copy, Clone)]
pub enum LiveWorldPos {
    Constant(WorldPos),
    PlayerPos,
}

impl LiveWorldPos {
    fn origin() -> LiveWorldPos {
        LiveWorldPos::Constant(WorldPos::origin())
    }

    fn world_pos(&self, player_pos: WorldPos) -> WorldPos {
        match self {
            LiveWorldPos::Constant(pos) => *pos,
            LiveWorldPos::PlayerPos => player_pos,
        }
    }
}

impl From<WorldPos> for LiveWorldPos {
    fn from(x: WorldPos) -> Self {
        LiveWorldPos::Constant(x)
    }
}

impl From<(f64, f64)> for LiveWorldPos {
    fn from(x: (f64, f64)) -> Self {
        LiveWorldPos::Constant(WorldPos::from(x))
    }
}

#[derive(Debug, Copy, Clone)]
enum SpawnCmd {
    Bullet {
        start: LiveWorldPos,
        end: LiveWorldPos,
    },
    Laser {
        position: LiveWorldPos,
        angle: f64,
    },
    LaserThruPoints {
        a: LiveWorldPos,
        b: LiveWorldPos,
    },
    CircleBomb {
        pos: LiveWorldPos,
    },
}

impl SpawnCmd {
    fn preform(&self, start_time: Beats, enemies: &mut Vec<Box<dyn Enemy>>, player_pos: WorldPos) {
        match *self {
            SpawnCmd::Bullet { start, end } => {
                let bullet = Bullet::new(
                    start.world_pos(player_pos),
                    end.world_pos(player_pos),
                    start_time,
                    Beats(4.0),
                );
                enemies.push(Box::new(bullet));
            }
            SpawnCmd::Laser { position, angle } => {
                let laser = Laser::new_through_point(
                    position.world_pos(player_pos),
                    angle,
                    start_time,
                    Beats(1.0),
                );
                enemies.push(Box::new(laser));
            }
            SpawnCmd::LaserThruPoints { a, b } => {
                let laser = Laser::new_through_points(
                    a.world_pos(player_pos),
                    b.world_pos(player_pos),
                    start_time,
                    Beats(1.0),
                );
                enemies.push(Box::new(laser));
            }
            SpawnCmd::CircleBomb { pos } => {
                let bomb = CircleBomb::new(start_time, pos.world_pos(player_pos));
                enemies.push(Box::new(bomb))
            }
        }
    }
}
