/// This module handles the playing of charts, which define what happens during
/// a particular song.
use std::cmp::{Ordering, Reverse};
use std::collections::binary_heap::PeekMut;
use std::collections::BinaryHeap;

use ggez::graphics::Color;
use ggez::Context;

use crate::ease::{BeatEasing, Easing};
use crate::enemy::{Bullet, CircleBomb, EnemyDurations, Laser, BOMB_WARMUP, LASER_WARMUP};
use crate::parse::{MarkedBeat, SongMap};
use crate::time::Beats;
use crate::world::WorldPos;
use crate::{EnemyGroup, WorldState};

/// This struct contains all the events that occur during a song. It will perform
/// a set of events every time update is called.
#[derive(Debug, Default)]
pub struct Scheduler {
    work_queue: BinaryHeap<BeatAction>,
}

impl Scheduler {
    pub fn new(_ctx: &mut Context, song_map: &SongMap) -> Scheduler {
        Scheduler {
            work_queue: BinaryHeap::from(song_map.actions.clone()),
        }
    }

    /// Preform the scheduled actions up to the new beat_time
    /// Note that this will execute every action since the last beat_time and
    /// current beat_time.
    pub fn update(&mut self, time: Beats, world: &mut WorldState) {
        let rev_beat = Reverse(time);
        loop {
            match self.work_queue.peek_mut() {
                Some(peaked) => {
                    if (*peaked).start_time > rev_beat {
                        let beat_action = PeekMut::pop(peaked);

                        beat_action.action.preform(
                            beat_action.group_number,
                            beat_action.start_time.0,
                            world,
                        )
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

impl IntoIterator for BeatSplitter {
    type Item = MarkedBeat;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.split().into_iter()
    }
}

impl BeatSplitter {
    pub fn split(&self) -> Vec<MarkedBeat> {
        let mut beats = vec![];
        let mut this_beat = self.start;
        while self.duration > this_beat - self.start {
            beats.push(MarkedBeat {
                beat: Beats(this_beat + self.delay + self.offset),
                percent: (this_beat + self.offset - self.start) / self.duration,
                pitch: None,
                midigroup: None,
            });
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
#[derive(Debug, Clone)]
pub struct BeatAction {
    // Stored in reverse ordering so that we can get the _earliest_ beat when in
    // the scheduler, rather than the latest.
    start_time: Reverse<Beats>, // for the binary heap's ordering
    group_number: usize,
    action: SpawnCmd,
}

impl BeatAction {
    /// Create a BeatAction. The action is scheduled at time `beat` if the
    /// SpawnCmd has no start time of its own, otherwise the action is scheduled
    /// (probably slightly earlier than the SpawnCmd's start time).
    pub fn new(start_time: Beats, group_number: usize, action: SpawnCmd) -> BeatAction {
        let beat = match action {
            // Schedule the lasers slightly earlier than their actual time
            // so that the laser pre-delays occurs at the right time.
            // Since the laser predelay is 4 beats, but the laser constructors
            // all assume the passed time is for the active phase, if we want
            // a laser to _fire_ on beat 20, it needs to be spawned in, at latest
            // beat 16, so that it works correctly.
            SpawnCmd::Laser { .. } => start_time - LASER_WARMUP,
            SpawnCmd::LaserThruPoints { .. } => start_time - LASER_WARMUP,
            SpawnCmd::CircleBomb { .. } => start_time - BOMB_WARMUP,
            _ => start_time,
        };
        BeatAction {
            start_time: Reverse(beat),
            group_number,
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
#[derive(Debug, Clone)]
pub enum LiveWorldPos {
    Constant(WorldPos),
    PlayerPos,
    OffsetPlayer(Box<LiveWorldPos>),
}

impl LiveWorldPos {
    fn world_pos(&self, player_pos: WorldPos) -> WorldPos {
        match self {
            &LiveWorldPos::Constant(pos) => pos,
            LiveWorldPos::PlayerPos => player_pos,
            LiveWorldPos::OffsetPlayer(offset) => player_pos + offset.world_pos(player_pos),
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

#[derive(Debug, Clone)]
pub enum SpawnCmd {
    Bullet {
        start: LiveWorldPos,
        end: LiveWorldPos,
    },
    BulletAngleStart {
        angle: f64,
        length: f64,
        start: LiveWorldPos,
    },
    BulletAngleEnd {
        angle: f64,
        length: f64,
        end: LiveWorldPos,
    },
    Laser {
        position: LiveWorldPos,
        angle: f64,
        durations: EnemyDurations,
    },
    LaserThruPoints {
        a: LiveWorldPos,
        b: LiveWorldPos,
        durations: EnemyDurations,
    },
    CircleBomb {
        pos: LiveWorldPos,
    },
    SetFadeOut(Option<(Color, Beats)>),
    SetGroupRotation(Option<(f64, f64, Beats, LiveWorldPos)>),
    SetHitbox(bool),
    ShowWarmup(bool),
    SetRender(bool),
    ClearEnemies,
}

impl SpawnCmd {
    fn preform(&self, group_number: usize, start_time: Beats, world: &mut WorldState) {
        let player_pos = world.player.pos;

        if group_number >= world.groups.len() {
            world.groups.resize_with(group_number + 1, EnemyGroup::new)
        }
        let group = &mut world.groups[group_number];
        match self {
            SpawnCmd::Bullet { start, end } => {
                let bullet = Bullet::new(
                    start.world_pos(player_pos),
                    end.world_pos(player_pos),
                    start_time,
                    Beats(4.0),
                );
                group.enemies.push(Box::new(bullet));
            }
            SpawnCmd::BulletAngleStart {
                angle,
                length,
                start,
            } => {
                let (unit_x, unit_y) = (angle.cos(), angle.sin());
                let start_pos = start.world_pos(player_pos);
                let end_pos = WorldPos {
                    x: start_pos.x + unit_x * length,
                    y: start_pos.y + unit_y * length,
                };
                let bullet = Bullet::new(start_pos, end_pos, start_time, Beats(4.0));
                group.enemies.push(Box::new(bullet));
            }
            SpawnCmd::BulletAngleEnd { angle, length, end } => {
                let (unit_x, unit_y) = (angle.cos(), angle.sin());
                let end_pos = end.world_pos(player_pos);
                let start_pos = WorldPos {
                    x: end_pos.x - unit_x * length,
                    y: end_pos.y - unit_y * length,
                };

                let bullet = Bullet::new(start_pos, end_pos, start_time, Beats(4.0));
                group.enemies.push(Box::new(bullet));
            }
            SpawnCmd::Laser {
                position,
                angle,
                durations,
            } => {
                let laser = Laser::new_through_point(
                    position.world_pos(player_pos),
                    *angle,
                    start_time,
                    *durations,
                );
                group.enemies.push(Box::new(laser));
            }
            SpawnCmd::LaserThruPoints { a, b, durations } => {
                let laser = Laser::new_through_points(
                    a.world_pos(player_pos),
                    b.world_pos(player_pos),
                    start_time,
                    *durations,
                );
                group.enemies.push(Box::new(laser));
            }
            SpawnCmd::CircleBomb { pos } => {
                let bomb = CircleBomb::new(start_time, pos.world_pos(player_pos));
                group.enemies.push(Box::new(bomb))
            }
            &SpawnCmd::SetFadeOut(fadeout) => {
                if let Some((color, duration)) = fadeout {
                    group.fadeout = Some(BeatEasing {
                        easing: Easing::linear(Color::WHITE, color),
                        start_time,
                        duration,
                    });
                } else {
                    group.fadeout = None;
                }
            }
            &SpawnCmd::SetHitbox(use_hitbox) => group.use_hitbox = use_hitbox,
            &SpawnCmd::ShowWarmup(show) => group.render_warmup = show,
            &SpawnCmd::SetRender(show) => group.do_render = show,
            SpawnCmd::SetGroupRotation(rotation) => {
                if let Some((start_angle, end_angle, duration, rot_point)) = rotation {
                    group.rotation = Some((
                        BeatEasing {
                            easing: Easing::linear(*start_angle, *end_angle),
                            start_time,
                            duration: *duration,
                        },
                        rot_point.world_pos(player_pos),
                    ));
                } else {
                    group.rotation = None;
                }
            }
            SpawnCmd::ClearEnemies => group.enemies.clear(),
        }
    }
}
