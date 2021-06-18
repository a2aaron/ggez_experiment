/// This module handles the playing of charts, which define what happens during
/// a particular song.
use std::cmp::{Ordering, Reverse};
use std::collections::binary_heap::PeekMut;
use std::collections::BinaryHeap;

use crate::ease::Lerp;
use crate::enemy::{Bullet, Enemy, Laser};
use crate::time::Beats;
use crate::world::{WorldLen, WorldPos};

/// This struct contains all the events that occur during a song. It will perform
/// a set of events every time update is called.
#[derive(Debug, Default)]
pub struct Scheduler {
    work_queue: BinaryHeap<BeatAction>,
}

impl Scheduler {
    pub fn new() -> Scheduler {
        fn beat_split(start: f64, duration: f64, split_length: f64) -> Vec<(Beats, f64)> {
            let mut beats = vec![];
            let mut this_beat = start;
            while duration > this_beat - start {
                beats.push((Beats(this_beat), (this_beat - start) / duration));
                this_beat += split_length;
            }
            beats
        }

        fn make_actions<'a>(
            beats: &'a [(Beats, f64)],
            action_spawner: impl Fn(f64) -> SpawnCmd + 'static,
        ) -> impl Iterator<Item = BeatAction> + 'a {
            beats
                .iter()
                .map(move |(beat, t)| BeatAction::new(*beat, action_spawner(*t)))
        }

        fn lerp_spawn(
            start: f64,
            duration: f64,
            split_length: f64,
            start_poses: ((f64, f64), (f64, f64)),
            end_poses: ((f64, f64), (f64, f64)),
        ) -> Vec<BeatAction> {
            let beats = beat_split(start, duration, split_length);
            let start_poses = (WorldPos::from(start_poses.0), WorldPos::from(start_poses.1));
            let end_poses = (WorldPos::from(end_poses.0), WorldPos::from(end_poses.1));

            make_actions(&beats, move |t| SpawnCmd::SpawnBullet {
                start: WorldPos::lerp(start_poses.0, start_poses.1, t),
                end: WorldPos::lerp(end_poses.0, end_poses.1, t),
            })
            .collect()
        }

        let mut work_queue = BinaryHeap::new();
        let origin = (0.0, 0.0);
        let bottom_left = (-50.0, -50.0);
        let bottom_right = (50.0, -50.0);
        let top_left = (-50.0, 50.0);
        let top_right = (50.0, 50.0);

        work_queue.extend(lerp_spawn(
            4.0 * 4.0,
            4.0 * 4.0,
            4.0,
            (bottom_left, bottom_right),
            (origin, origin),
        ));

        work_queue.extend(lerp_spawn(
            4.0 * 4.0,
            4.0 * 4.0,
            4.0,
            (top_right, top_left),
            (origin, origin),
        ));

        work_queue.extend(lerp_spawn(
            8.0 * 4.0,
            4.0 * 4.0,
            2.0,
            (top_left, bottom_left),
            (origin, origin),
        ));

        work_queue.extend(lerp_spawn(
            8.0 * 4.0,
            4.0 * 4.0,
            2.0,
            (bottom_right, top_right),
            (origin, origin),
        ));

        work_queue.extend(lerp_spawn(
            12.0 * 4.0,
            4.0 * 4.0,
            2.0,
            (top_right, bottom_right),
            (top_left, bottom_left),
        ));

        work_queue.extend(lerp_spawn(
            12.0 * 4.0 + 1.0,
            4.0 * 4.0,
            2.0,
            (bottom_left, top_left),
            (bottom_right, top_right),
        ));

        Scheduler { work_queue }
    }

    /// Preform the scheduled actions up to the new beat_time
    /// Note that this will execute every action since the last beat_time and
    /// current beat_time.
    pub fn update(&mut self, time: Beats, enemies: &mut Vec<Box<dyn Enemy>>) {
        let rev_beat = Reverse(time);
        loop {
            match self.work_queue.peek_mut() {
                Some(peaked) => {
                    if (*peaked).beat > rev_beat {
                        PeekMut::pop(peaked).action.preform(time, enemies)
                    } else {
                        return;
                    }
                }
                None => return,
            }
        }
    }
}

/// A wrapper struct of a Beat and a Boxed Action. The beat has reversed ordering
/// to allow for the Scheduler to actually get the latest beat times.
#[derive(Debug)]
struct BeatAction {
    // Stored in reverse ordering so that we can get the _earliest_ beat when in
    // the scheduler, rather than the latest.
    beat: Reverse<Beats>, // for the binary heap's ordering
    action: SpawnCmd,
}

impl BeatAction {
    fn new(beat: Beats, action: SpawnCmd) -> BeatAction {
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

#[derive(Debug, Copy, Clone)]
enum SpawnCmd {
    SpawnBullet { start: WorldPos, end: WorldPos },
    SpawnLaser { position: WorldPos, angle: f64 },
}

impl SpawnCmd {
    fn preform(&self, time: Beats, enemies: &mut Vec<Box<dyn Enemy>>) {
        match *self {
            SpawnCmd::SpawnBullet { start, end } => {
                let mut bullet = Bullet::new(start, end, Beats(4.0));
                bullet.on_spawn(time);
                enemies.push(Box::new(bullet));
            }
            SpawnCmd::SpawnLaser { position, angle } => {
                let mut laser = Laser::new_through_point(position, angle, Beats(1.0));
                laser.on_spawn(time);
                enemies.push(Box::new(laser));
            }
        }
    }
}
