use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicUsize;

use ggez::graphics::Color;
use ggez::Context;
use midly::{Header, Smf, TrackEvent};
use rand::Rng;

use crate::chart::{BeatAction, BeatSplitter, LiveWorldPos, SpawnCmd};
use crate::ease::Lerp;
use crate::time::Beats;
use crate::world::WorldPos;
use crate::{time, util};

/// This struct essentially acts as an interpreter for a song's file. All parsing
/// occurs before the actual level is played, with the file format being line
/// based.
#[derive(Debug, Clone)]
pub struct SongMap {
    pub skip_amount: Beats,
    pub bpm: f64,
    pub actions: Vec<BeatAction>,
}

impl SongMap {
    pub fn read_map<P: AsRef<Path>>(
        ctx: &mut Context,
        path: P,
    ) -> Result<SongMap, Box<dyn std::error::Error>> {
        // Read the file to a String
        let mut file = ggez::filesystem::open(ctx, path)?;
        let mut script = String::new();
        file.read_to_string(&mut script)?;

        // Spawn a seperate thread to evaluate the script. This is done because
        // some Rhai scripts seem to cause the engine to lock up forever as it
        // tries to compile it. To avoid this, we use a channel with a timeout.
        // See also: https://github.com/rhaiscript/rhai/issues/421
        // Note that this will result in the thread just dangling forever, doing
        // active work, so it would be Really Good if this bug got fixed.
        let (send, recv) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            let result = SongMap::run_rhai(script.as_str());
            match result {
                Ok(result) => match send.send(result) {
                    Ok(()) => (),
                    Err(err) => println!("Sending evulation result failed! {:?}", err),
                },
                Err(err) => println!("Evaluation of script failed! {:?}", err),
            };
        });

        match recv.recv_timeout(std::time::Duration::new(1, 0)) {
            Ok(result) => Ok(result),
            Err(err) => Err(err.into()),
        }
    }

    pub fn run_rhai(script: &str) -> Result<SongMap, Box<rhai::EvalAltResult>> {
        // Create scripting engine
        let mut engine = rhai::Engine::new();

        // Allow deeply nested expressions
        engine.set_max_expr_depths(0, 0);

        // Register functions for use within the rhai file.

        // SongMap commands
        engine
            .register_fn("default_map", SongMap::default)
            .register_fn("set_bpm", SongMap::set_bpm)
            .register_fn("set_skip_amount", SongMap::set_skip_amount)
            .register_fn("add_action", SongMap::add_action)
            .register_result_fn("add_actions", |map: &mut SongMap, arr: rhai::Array| {
                let arr: Vec<BeatAction> = arr
                    .iter()
                    .enumerate()
                    .map(
                        |(i, action)| match action.clone().try_cast::<BeatAction>() {
                            Some(x) => Ok(x),
                            None => Err(format!(
                                "Object at index {} must be type BeatAction. Got {}",
                                i,
                                action.type_name()
                            )),
                        },
                    )
                    .collect::<Result<Vec<_>, _>>()?;

                map.add_actions(arr);
                Ok(())
            });

        // Midi parsing
        engine
            .register_result_fn("parse_midi", |path: rhai::ImmutableString, bpm: f64| {
                match parse_midi(path.as_str(), bpm, midi_to_beats_ungrouped) {
                    Ok(result) => Ok(MarkedBeat::mark_beats(&result)),
                    Err(err) => Err(err.to_string().into()),
                }
            })
            .register_result_fn(
                "parse_midi_grouped",
                |path: rhai::ImmutableString, bpm: f64| match parse_midi(
                    path.as_str(),
                    bpm,
                    midi_to_beats_grouped,
                ) {
                    Ok(result) => {
                        let marked_beats: Vec<Vec<MarkedBeat>> = result
                            .iter()
                            .map(|beats| MarkedBeat::mark_beats(beats))
                            .collect();
                        Ok(marked_beats)
                    }
                    Err(err) => Err(err.to_string().into()),
                },
            );

        // MarkedBeat
        engine
            .register_type::<MarkedBeat>()
            .register_fn("to_beat_tuple", |beats: Vec<Beats>| {
                MarkedBeat::mark_beats(&beats)
            })
            .register_fn("offset_tuple", |offset: f64, beats: Vec<MarkedBeat>| {
                MarkedBeat::offset(&beats, Beats(offset))
            })
            .register_fn(
                "offset_tuple_grouped",
                |offset: f64, beat_groups: Vec<Vec<MarkedBeat>>| {
                    beat_groups
                        .iter()
                        .map(|beats| MarkedBeat::offset(beats, Beats(offset)))
                        .collect::<Vec<Vec<MarkedBeat>>>()
                },
            )
            .register_fn("len", |x: Vec<MarkedBeat>| x.len() as i64)
            .register_fn("get_beat", |x: MarkedBeat| x.beat.0)
            .register_fn("get_percent", |x: MarkedBeat| x.percent);

        engine
            .register_iterator::<Vec<MarkedBeat>>()
            .register_iterator::<Vec<Vec<MarkedBeat>>>();

        // BeatSplitter
        engine
            .register_type::<BeatSplitter>()
            .register_iterator::<BeatSplitter>()
            .register_fn("beat_splitter", |start: f64, frequency: f64| BeatSplitter {
                start,
                frequency,
                ..Default::default()
            })
            .register_fn("with_start", BeatSplitter::with_start)
            .register_fn("with_freq", BeatSplitter::with_freq)
            .register_fn("with_offset", BeatSplitter::with_offset)
            .register_fn("with_delay", BeatSplitter::with_delay)
            .register_fn("with_duration", BeatSplitter::with_duration);

        // Various LiveWorldPos functions.
        engine
            .register_type::<LiveWorldPos>()
            .register_fn("pos", |x: f64, y: f64| LiveWorldPos::from((x, y)))
            .register_fn("origin", || LiveWorldPos::from((0.0, 0.0)))
            .register_fn("player", || LiveWorldPos::PlayerPos)
            .register_fn("offset_player", |pos| {
                LiveWorldPos::OffsetPlayer(Box::new(pos))
            })
            .register_result_fn(
                "lerp_pos",
                |a: LiveWorldPos, b: LiveWorldPos, t: f64| match (a, b) {
                    (LiveWorldPos::Constant(a), LiveWorldPos::Constant(b)) => {
                        Ok(LiveWorldPos::Constant(WorldPos::lerp(a, b, t)))
                    }
                    (LiveWorldPos::PlayerPos, LiveWorldPos::PlayerPos) => {
                        Ok(LiveWorldPos::PlayerPos)
                    }
                    (a, b) => Err(format!(
                        "Expected constant LiveWorldPos. Got {:?} and {:?}",
                        a, b
                    )
                    .into()),
                },
            );

        // Splitting LiveWorldPoses
        engine
            .register_type::<LiveWorldPos>()
            .register_get_result("x", |pos: &mut LiveWorldPos| match pos {
                LiveWorldPos::Constant(pos) => Ok(pos.x),
                _ => Err("Position is a symbolic player position".into()),
            })
            .register_get_result("y", |pos: &mut LiveWorldPos| match pos {
                LiveWorldPos::Constant(pos) => Ok(pos.y),
                _ => Err("Position is a symbolic player position".into()),
            });

        // LiveWorldPos helpers
        engine.register_fn("lerp", |start: f64, end: f64, t: f64| t.lerp(start, end));

        engine.register_fn(
            "circle",
            |center_x: f64, center_y: f64, radius: f64, angle: f64| {
                let angle = angle.to_radians();
                LiveWorldPos::from((
                    angle.cos() * radius + center_x,
                    angle.sin() * radius + center_y,
                ))
            },
        );

        engine.register_fn("grid", || {
            LiveWorldPos::from(util::random_grid((-50.0, 50.0), (-50.0, 50.0), 20))
        });

        engine.register_result_fn("random", |min: f64, max: f64| {
            if min < max {
                Ok(rand::thread_rng().gen_range(min..max))
            } else {
                Err(format!("min: {} must be smaller than max: {}", min, max).into())
            }
        });

        // Colors
        engine
            .register_type::<Color>()
            .register_fn("color", |r: f64, g: f64, b: f64, a: f64| {
                Color::new(r as f32, g as f32, b as f32, a as f32)
            });

        fn try_into_usize(x: i64) -> Result<usize, Box<rhai::EvalAltResult>> {
            std::convert::TryInto::<usize>::try_into(x)
                .map_err(|x| Box::new(rhai::EvalAltResult::from(x.to_string())))
        }

        engine.register_result_fn("usize", try_into_usize);

        engine.register_type::<BeatAction>().register_fn(
            "beat_action",
            |start_time: f64, group_number: usize, action: SpawnCmd| {
                BeatAction::new(Beats(start_time), group_number, action)
            },
        );

        // All the SpawnCmds
        engine
            .register_type::<SpawnCmd>()
            .register_fn("bullet", |start, end| SpawnCmd::Bullet { start, end })
            .register_fn("bullet_angle_start", |angle, length, start| {
                SpawnCmd::BulletAngleStart {
                    angle,
                    length,
                    start,
                }
            })
            .register_fn("bullet_angle_end", |angle, length, end| {
                SpawnCmd::BulletAngleEnd { angle, length, end }
            })
            .register_fn("laser", |a, b| SpawnCmd::LaserThruPoints { a, b })
            .register_fn("laser_angle", |position, angle: f64| SpawnCmd::Laser {
                position,
                angle: angle.to_radians(),
            })
            .register_fn("bomb", |pos| SpawnCmd::CircleBomb { pos })
            .register_fn("set_fadeout_on", |color: Color, duration: f64| {
                SpawnCmd::SetFadeOut(Some((color, Beats(duration))))
            })
            .register_fn("set_fadeout_off", || SpawnCmd::SetFadeOut(None))
            .register_fn(
                "set_rotation_on",
                |start_angle: f64, end_angle: f64, duration, rot_point| {
                    SpawnCmd::SetGroupRotation(Some((
                        start_angle.to_radians(),
                        end_angle.to_radians(),
                        Beats(duration),
                        rot_point,
                    )))
                },
            )
            .register_fn("set_rotation_off", || SpawnCmd::SetGroupRotation(None))
            .register_fn("set_use_hitbox", |use_hitbox: bool| {
                SpawnCmd::SetHitbox(use_hitbox)
            })
            .register_fn("set_render_warmup", SpawnCmd::ShowWarmup);

        static CURR_GROUP: AtomicUsize = AtomicUsize::new(0);
        engine
            .register_result_fn("set_curr_group", move |group: i64| {
                CURR_GROUP.store(try_into_usize(group)?, std::sync::atomic::Ordering::Relaxed);
                Ok(())
            })
            .register_fn("get_curr_group", move || {
                CURR_GROUP.load(std::sync::atomic::Ordering::Relaxed)
            });

        engine.eval::<SongMap>(script)
    }

    fn set_bpm(&mut self, bpm: f64) {
        self.bpm = bpm;
    }

    fn set_skip_amount(&mut self, skip_amount: f64) {
        self.skip_amount = Beats(skip_amount);
    }

    fn add_action(&mut self, action: BeatAction) {
        self.actions.push(action);
    }

    fn add_actions(&mut self, actions: impl IntoIterator<Item = BeatAction>) {
        self.actions.extend(actions)
    }
}

impl Default for SongMap {
    fn default() -> Self {
        SongMap {
            skip_amount: Beats(0.0),
            bpm: 150.0,
            actions: vec![],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MarkedBeat {
    pub beat: Beats,
    pub percent: f64,
}

impl MarkedBeat {
    /// The slice is assumed to be in sorted order and the last beat is assumed
    /// to be the duration of the whole slice.
    pub fn mark_beats(beats: &[Beats]) -> Vec<MarkedBeat> {
        if beats.is_empty() {
            return vec![];
        }
        let mut marked_beats = vec![];
        let duration = beats.last().unwrap();
        for &beat in beats {
            let marked_beat = MarkedBeat {
                beat,
                percent: beat.0 / duration.0,
            };
            marked_beats.push(marked_beat)
        }
        marked_beats
    }

    fn offset(beats: &[MarkedBeat], offset: Beats) -> Vec<MarkedBeat> {
        beats
            .iter()
            .map(|old| MarkedBeat {
                beat: old.beat + offset,
                percent: old.percent,
            })
            .collect()
    }
}

pub fn midi_to_beats_grouped(track: &[TrackEvent], ticks_per_beat: f64) -> Vec<Vec<Beats>> {
    let mut tick_number = 0;
    let mut beats = vec![];
    let mut group = vec![];

    let mut last_note = None;

    for event in track {
        tick_number += event.delta.as_int();
        if let midly::TrackEventKind::Midi {
            message: midly::MidiMessage::NoteOn { key, .. },
            ..
        } = event.kind
        {
            let beat = Beats(tick_number as f64 / ticks_per_beat);
            if let Some(last_note) = last_note {
                if last_note != key {
                    beats.push(group.clone());
                    group.clear();
                }
            }
            group.push(beat);
            last_note = Some(key);
        }
    }

    beats
}

pub fn midi_to_beats_ungrouped(track: &[TrackEvent], ticks_per_beat: f64) -> Vec<Beats> {
    let mut tick_number = 0;
    let mut beats = vec![];

    for event in track {
        tick_number += event.delta.as_int();
        if let midly::TrackEventKind::Midi {
            message: midly::MidiMessage::NoteOn { .. },
            ..
        } = event.kind
        {
            beats.push(Beats(tick_number as f64 / ticks_per_beat));
        }
    }
    beats
}

pub fn get_ticks_per_beat(header: &Header, bpm: f64) -> f64 {
    match header.timing {
        midly::Timing::Metrical(ticks_per_beat) => ticks_per_beat.as_int() as f64,
        midly::Timing::Timecode(fps, num_subframes) => {
            let ticks_per_second = fps.as_f32() * num_subframes as f32;
            let seconds_per_beat = time::beat_length(bpm).0;
            ticks_per_second as f64 * seconds_per_beat
        }
    }
}

pub fn parse_midi<P: AsRef<Path>, T>(
    path: P,
    bpm: f64,
    func: impl Fn(&[TrackEvent], f64) -> T,
) -> Result<T, Box<dyn std::error::Error>> {
    let buffer = std::fs::read(path)?;
    let smf = Smf::parse(&buffer)?;
    let ticks_per_beat = get_ticks_per_beat(&smf.header, bpm);
    Ok(func(&smf.tracks[0], ticks_per_beat))
}
