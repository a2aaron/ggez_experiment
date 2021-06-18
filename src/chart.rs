/// This module handles the playing of charts, which define what happens during
/// a particular song.
use std::cmp::{Ordering, Reverse};
use std::collections::binary_heap::PeekMut;
use std::collections::BinaryHeap;

use crate::ease::Lerp;
use crate::enemy::{Bullet, Enemy, Laser, LASER_PREDELAY};
use crate::time::Beats;
use crate::world::WorldPos;

/// This struct contains all the events that occur during a song. It will perform
/// a set of events every time update is called.
#[derive(Debug, Default)]
pub struct Scheduler {
    work_queue: BinaryHeap<BeatAction>,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        fn make_actions<'a>(
            beats: &'a [(Beats, f64)],
            action_spawner: impl Fn(f64) -> SpawnCmd + 'static,
        ) -> impl Iterator<Item = BeatAction> + 'a {
            beats
                .iter()
                .map(move |(beat, t)| BeatAction::new(*beat, action_spawner(*t)))
        }

        fn lerp_spawn(
            beat_split: BeatSplitter,
            start_poses: ((f64, f64), (f64, f64)),
            end_poses: ((f64, f64), (f64, f64)),
        ) -> Vec<BeatAction> {
            let beats = beat_split.split();
            let start_poses = (WorldPos::from(start_poses.0), WorldPos::from(start_poses.1));
            let end_poses = (WorldPos::from(end_poses.0), WorldPos::from(end_poses.1));

            make_actions(&beats, move |t| {
                let start = LiveWorldPos::Constant(WorldPos::lerp(start_poses.0, start_poses.1, t));
                let end = LiveWorldPos::Constant(WorldPos::lerp(end_poses.0, end_poses.1, t));
                SpawnCmd::SpawnBullet { start, end }
            })
            .collect()
        }

        fn lerp_spawn_player(
            beat_split: BeatSplitter,
            start_poses: ((f64, f64), (f64, f64)),
        ) -> Vec<BeatAction> {
            let beats = beat_split.split();
            let start_poses = (WorldPos::from(start_poses.0), WorldPos::from(start_poses.1));
            make_actions(&beats, move |t| {
                let start = LiveWorldPos::Constant(WorldPos::lerp(start_poses.0, start_poses.1, t));
                let end = LiveWorldPos::PlayerPos;
                SpawnCmd::SpawnBullet { start, end }
            })
            .collect()
        }

        fn lerp_spawn_laser_player(
            beat_split: BeatSplitter,
            start_poses: ((f64, f64), (f64, f64)),
        ) -> Vec<BeatAction> {
            let start_poses = (WorldPos::from(start_poses.0), WorldPos::from(start_poses.1));
            beat_split
                .split()
                .iter()
                .map(|&(beat, t)| {
                    let start =
                        LiveWorldPos::Constant(WorldPos::lerp(start_poses.0, start_poses.1, t));
                    let end = LiveWorldPos::PlayerPos;
                    let action = SpawnCmd::SpawnLaserThruPoints {
                        a: start,
                        b: end,
                        start_time: beat,
                    };
                    // beat here is unneeded technically since it uses the SpawnCmd's start_time
                    BeatAction::new(beat, action)
                })
                .collect()
        }

        let mut work_queue = BinaryHeap::new();
        let origin = (0.0, 0.0);
        let bottom_left = (-50.0, -50.0);
        let bottom_right = (50.0, -50.0);
        let top_left = (-50.0, 50.0);
        let top_right = (50.0, 50.0);

        let every_4_beats = BeatSplitter {
            duration: 4.0 * 4.0,
            frequency: 4.0,
            ..Default::default()
        };

        let every_2_beats = BeatSplitter {
            duration: 4.0 * 4.0,
            frequency: 2.0,
            ..Default::default()
        };

        let every_beat = BeatSplitter {
            duration: 4.0 * 4.0,
            frequency: 1.0,
            ..Default::default()
        };

        work_queue.extend(lerp_spawn(
            every_4_beats.with_start(4.0 * 4.0),
            (bottom_left, bottom_right),
            (origin, origin),
        ));

        work_queue.extend(lerp_spawn(
            every_4_beats.with_start(4.0 * 4.0),
            (top_right, top_left),
            (origin, origin),
        ));

        work_queue.extend(lerp_spawn(
            every_2_beats.with_start(8.0 * 4.0),
            (top_left, bottom_left),
            (origin, origin),
        ));

        work_queue.extend(lerp_spawn(
            every_2_beats.with_start(8.0 * 4.0),
            (bottom_right, top_right),
            (origin, origin),
        ));

        work_queue.extend(lerp_spawn(
            every_2_beats.with_start(12.0 * 4.0),
            (top_right, bottom_right),
            (top_left, bottom_left),
        ));

        work_queue.extend(lerp_spawn(
            every_2_beats.with_start(12.0 * 4.0).with_offset(1.0),
            (bottom_left, top_left),
            (bottom_right, top_right),
        ));

        work_queue.extend(lerp_spawn_player(
            every_beat.with_start(16.0 * 4.0),
            (top_right, top_left),
        ));

        work_queue.extend(lerp_spawn_player(
            every_beat.with_start(16.0 * 4.0).with_offset(0.5),
            (bottom_left, bottom_right),
        ));

        // DROP
        work_queue.extend(lerp_spawn_laser_player(
            every_beat.with_start(20.0 * 4.0),
            (origin, origin),
        ));

        Scheduler { work_queue }
    }

    /// Preform the scheduled actions up to the new beat_time
    /// Note that this will execute every action since the last beat_time and
    /// current beat_time.
    pub fn update(&mut self, time: Beats, enemies: &mut Vec<Box<dyn Enemy>>, player_pos: WorldPos) {
        let rev_beat = Reverse(time);
        loop {
            match self.work_queue.peek_mut() {
                Some(peaked) => {
                    if (*peaked).beat > rev_beat {
                        PeekMut::pop(peaked)
                            .action
                            .preform(time, enemies, player_pos)
                    } else {
                        return;
                    }
                }
                None => return,
            }
        }
    }
}

/// Split a length of time into a number of individual beats. This is useful for
/// doing something a number of times in a row.
#[derive(Debug, Clone, Copy)]
struct BeatSplitter {
    start: f64,
    duration: f64,
    frequency: f64,
    offset: f64,
}

impl Default for BeatSplitter {
    fn default() -> BeatSplitter {
        BeatSplitter {
            start: 0.0,
            duration: 4.0 * 4.0, // 4 measures total
            frequency: 4.0,
            offset: 0.0,
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
        }
    }

    fn with_offset(self, offset: f64) -> Self {
        BeatSplitter {
            start: self.start,
            duration: self.duration,
            frequency: self.frequency,
            offset,
        }
    }

    fn with_duration(self, duration: f64) -> Self {
        BeatSplitter {
            start: self.start,
            duration,
            frequency: self.frequency,
            offset: self.offset,
        }
    }

    fn split(&self) -> Vec<(Beats, f64)> {
        let mut beats = vec![];
        let mut this_beat = self.start;
        while self.duration > this_beat - self.start {
            beats.push((
                Beats(this_beat + self.offset),
                (this_beat - self.start) / self.duration,
            ));
            this_beat += self.frequency;
        }
        beats
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
#[derive(Debug)]
struct BeatAction {
    // Stored in reverse ordering so that we can get the _earliest_ beat when in
    // the scheduler, rather than the latest.
    beat: Reverse<Beats>, // for the binary heap's ordering
    action: SpawnCmd,
}

impl BeatAction {
    /// Create a BeatAction. The action is scheduled at time `beat` if the
    /// SpawnCmd has no start time of its own, otherwise the action is scheduled
    /// (probably slightly earlier than the SpawnCmd's start time).
    fn new(beat: Beats, action: SpawnCmd) -> BeatAction {
        let beat = match action {
            SpawnCmd::SpawnBullet { .. } => beat,
            // Schedule the lasers slightly earlier than their actual time
            // so that the laser pre-delays occurs at the right time.
            // Since the laser predelay is 4 beats, but the laser constructors
            // all assume the passed time is for the active phase, if we want
            // a laser to _fire_ on beat 20, it needs to be spawned in, at latest
            // beat 16, so that it works correctly.
            SpawnCmd::SpawnLaser { start_time, .. } => start_time - LASER_PREDELAY,
            SpawnCmd::SpawnLaserThruPoints { start_time, .. } => start_time - LASER_PREDELAY,
        };
        BeatAction {
            beat: Reverse(beat),
            action,
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
        //  NOTE: We assume that weird values (like inf or NaN) never happen.
        //  and just assume that returning equal is okay here.
        self.beat
            .partial_cmp(&other.beat)
            .unwrap_or(Ordering::Equal)
    }
}

/// A WorldPosition which depends on some dynamic value (ex: the player's
/// position).
#[derive(Debug, Copy, Clone)]
enum LiveWorldPos {
    Constant(WorldPos),
    PlayerPos,
}

impl LiveWorldPos {
    fn world_pos(&self, player_pos: WorldPos) -> WorldPos {
        match self {
            LiveWorldPos::Constant(pos) => *pos,
            LiveWorldPos::PlayerPos => player_pos,
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum SpawnCmd {
    SpawnBullet {
        start: LiveWorldPos,
        end: LiveWorldPos,
    },
    SpawnLaser {
        position: LiveWorldPos,
        angle: f64,
        start_time: Beats,
    },
    SpawnLaserThruPoints {
        a: LiveWorldPos,
        b: LiveWorldPos,
        start_time: Beats,
    },
}

impl SpawnCmd {
    fn preform(&self, curr_time: Beats, enemies: &mut Vec<Box<dyn Enemy>>, player_pos: WorldPos) {
        match *self {
            SpawnCmd::SpawnBullet { start, end } => {
                let bullet = Bullet::new(
                    start.world_pos(player_pos),
                    end.world_pos(player_pos),
                    curr_time,
                    Beats(4.0),
                );
                enemies.push(Box::new(bullet));
            }
            SpawnCmd::SpawnLaser {
                position,
                angle,
                start_time,
            } => {
                let laser = Laser::new_through_point(
                    position.world_pos(player_pos),
                    angle,
                    start_time,
                    Beats(1.0),
                );
                enemies.push(Box::new(laser));
            }
            SpawnCmd::SpawnLaserThruPoints { a, b, start_time } => {
                let laser = Laser::new_through_points(
                    a.world_pos(player_pos),
                    b.world_pos(player_pos),
                    start_time,
                    Beats(1.0),
                );
                enemies.push(Box::new(laser));
            }
        }
    }
}
