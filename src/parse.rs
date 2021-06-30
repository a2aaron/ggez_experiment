use std::fmt;
use std::io::Read;
use std::path::Path;

use ggez::Context;
use midly::Smf;

use crate::chart::{mark_beats, BeatAction, BeatSplitter, LiveWorldPos, SpawnCmd};
use crate::ease::Lerp;
use crate::time;
use crate::time::Beats;
use crate::world::WorldPos;

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
    pub fn run_rhai<P: AsRef<Path>>(
        ctx: &mut Context,
        path: P,
    ) -> Result<SongMap, Box<dyn std::error::Error>> {
        // Read the file to a String
        let mut file = ggez::filesystem::open(ctx, path)?;
        let mut script = String::new();
        file.read_to_string(&mut script)?;

        // Create scripting engine
        let mut engine = rhai::Engine::new();

        // Avoid infinite loops killing the game.
        engine.set_max_operations(10_000);

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

        engine
            .register_type::<LiveWorldPos>()
            .register_type::<SpawnCmd>();

        // Needed so that we can split apart tuples.
        engine
            .register_type::<(Beats, f64)>()
            .register_fn("get_beat", |x: (Beats, f64)| x.0 .0)
            .register_fn("get_percent", |x: (Beats, f64)| x.1);

        fn make_bullet(start_time: f64, start: LiveWorldPos, end: LiveWorldPos) -> BeatAction {
            BeatAction::new(
                Beats(start_time),
                crate::chart::SpawnCmd::Bullet { start, end },
            )
        }

        engine
            .register_type::<BeatAction>()
            .register_fn("bullet", make_bullet);

        Ok(engine.eval::<SongMap>(script.as_str())?)
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
        // dbg!(actions);
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

#[derive(Debug, Clone)]
pub enum TimingData {
    BeatSplitter(BeatSplitter),
    MidiBeat((f64, Vec<Beats>)),
}

impl TimingData {
    pub fn to_beat_vec(&self) -> Vec<(Beats, f64)> {
        match self {
            TimingData::BeatSplitter(splitter) => splitter.split(),
            TimingData::MidiBeat((start, beats)) => crate::chart::mark_beats(*start, beats),
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
