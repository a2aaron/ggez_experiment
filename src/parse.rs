use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicUsize;

use ggez::graphics::Color;
use ggez::Context;
use midly::Smf;
use rand::Rng;

use crate::chart::{mark_beats, BeatAction, BeatSplitter, LiveWorldPos, SpawnCmd};
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

                map.add_actions(&arr);
                Ok(())
            });

        // Various extras
        engine.register_result_fn("parse_midi", |path: rhai::ImmutableString, bpm: f64| {
            match parse_midi_to_beats(path.as_str(), bpm) {
                Ok(beats) => Ok(beats),
                Err(err) => Err(err.to_string().into()),
            }
        });

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

        // Iterators
        engine.register_fn("to_beat_tuple", |start: f64, beats: Vec<Beats>| {
            mark_beats(start, &beats)
        });

        engine.register_iterator::<Vec<(Beats, f64)>>();

        // Beat splitting things
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

        // Colors
        engine
            .register_type::<Color>()
            .register_fn("color", |r: f64, g: f64, b: f64, a: f64| {
                Color::new(r as f32, g as f32, b as f32, a as f32)
            });

        engine.register_type::<SpawnCmd>();

        // Needed so that we can split apart tuples.
        engine
            .register_type::<(Beats, f64)>()
            .register_fn("get_beat", |x: (Beats, f64)| x.0 .0)
            .register_fn("get_percent", |x: (Beats, f64)| x.1);

        engine.register_type::<BeatAction>().register_fn(
            "beat_action",
            |start_time: f64, group_number: usize, action: SpawnCmd| {
                BeatAction::new(Beats(start_time), group_number, action)
            },
        );

        fn try_into_usize(x: i64) -> Result<usize, Box<rhai::EvalAltResult>> {
            std::convert::TryInto::<usize>::try_into(x)
                .map_err(|x| Box::new(rhai::EvalAltResult::from(x.to_string())))
        }

        engine.register_result_fn("usize", try_into_usize);

        // SpawnCmds
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
            .register_fn("set_fadeout_on", |color: Color| {
                SpawnCmd::SetFadeOut(Some(color))
            })
            .register_fn("set_fadeout_off", || SpawnCmd::SetFadeOut(None))
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

    fn add_actions(&mut self, actions: &[BeatAction]) {
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

pub fn parse_midi_to_beats<P: AsRef<Path>>(
    path: P,
    bpm: f64,
) -> Result<Vec<Beats>, Box<dyn std::error::Error>> {
    let buffer = std::fs::read(path)?;
    let smf = Smf::parse(&buffer)?;
    let mut tick_number = 0;
    let mut beats = vec![];

    let ticks_per_beat = match smf.header.timing {
        midly::Timing::Metrical(ticks_per_beat) => ticks_per_beat.as_int() as f64,
        midly::Timing::Timecode(fps, num_subframes) => {
            let ticks_per_second = fps.as_f32() * num_subframes as f32;
            let seconds_per_beat = time::beat_length(bpm).0;
            ticks_per_second as f64 * seconds_per_beat
        }
    };

    for track in &smf.tracks[0] {
        tick_number += track.delta.as_int();
        if let midly::TrackEventKind::Midi { message, .. } = track.kind {
            match message {
                midly::MidiMessage::NoteOn { .. } => {
                    beats.push(Beats(tick_number as f64 / ticks_per_beat));
                }
                midly::MidiMessage::NoteOff { .. } => {} // explicitly ignore NoteOff
                _ => {}
            }
        }
    }
    Ok(beats)
}
