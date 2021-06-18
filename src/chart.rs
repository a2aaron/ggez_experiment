/// This module handles the playing of charts, which define what happens during
/// a particular song.
use std::cmp::{Ordering, Reverse};
use std::collections::binary_heap::PeekMut;
use std::collections::BinaryHeap;

use crate::enemy::{Bullet, Enemy, Laser};
use crate::time::Beats;
use crate::world::WorldPos;

#[derive(Debug, Default)]
pub struct Scheduler {
    work_queue: BinaryHeap<BeatAction>,
}
impl Scheduler {
    pub fn new() -> Scheduler {
        let mut work_queue = BinaryHeap::new();
        // for i in 0..24 {
        //     work_queue
        // }
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
