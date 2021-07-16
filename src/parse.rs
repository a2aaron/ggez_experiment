use std::io::Read;
use std::path::Path;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;

use ggez::graphics::Color;
use ggez::Context;
use midly::{Header, Smf, TrackEvent};
use rand::Rng;
use rlua::{Lua, Table, ToLua};

use crate::chart::{BeatAction, BeatSplitter, LiveWorldPos, SpawnCmd};
use crate::ease::Lerp;
use crate::enemy::EnemyDurations;
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
        let mut file = ggez::filesystem::open(ctx, path)?;
        let mut source = Vec::new();
        file.read_to_end(&mut source)?;
        Ok(SongMap::run_lua(&source)?)
    }

    pub fn run_lua(source: &[u8]) -> Result<SongMap, rlua::Error> {
        let lua = Lua::new();
        lua.context(|ctx| {
            let source = ctx.load(source);

            let read_midi =
                ctx.create_function(|_, (path, bpm): (String, f64)| {
                    match parse_midi(path.as_str(), bpm, midi_to_beats_ungrouped) {
                        Ok(beats) => Ok(beats),
                        Err(err) => Err(rlua::Error::external(err)),
                    }
                })?;
            ctx.globals().set("read_midi", read_midi)?;

            let read_midi =
                ctx.create_function(|_, (path, bpm): (String, f64)| {
                    match parse_midi(path.as_str(), bpm, midi_to_beats_grouped) {
                        Ok(beats) => Ok(beats),
                        Err(err) => Err(rlua::Error::external(err)),
                    }
                })?;
            ctx.globals().set("read_midi_grouped", read_midi)?;

            source.eval::<SongMap>()
        })
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
                    Ok(result) => Ok(result),
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
                    Ok(result) => Ok(result),
                    Err(err) => Err(err.to_string().into()),
                },
            );

        // MarkedBeat
        engine
            .register_type::<MarkedBeat>()
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
            .register_fn("normalize_pitch", |x: Vec<MarkedBeat>| {
                MarkedBeat::normalize_pitch(&x)
            })
            .register_fn("get_beat", |x: MarkedBeat| x.beat.0)
            .register_fn("get_percent", |x: MarkedBeat| x.percent)
            .register_fn("get_pitch", |x: MarkedBeat| x.pitch);

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

        // EnemyDurations
        engine
            .register_type::<EnemyDurations>()
            .register_fn("durations", |warmup: f64, active: f64, cooldown: f64| {
                EnemyDurations {
                    warmup: Beats(warmup),
                    active: Beats(active),
                    cooldown: Beats(cooldown),
                }
            })
            .register_fn("default_laser_duration", || {
                EnemyDurations::default_laser(Beats(1.0))
            });

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
            .register_fn("laser", |a, b, durations| SpawnCmd::LaserThruPoints {
                a,
                b,
                durations,
            })
            .register_fn("laser_angle", |position, angle: f64, durations| {
                SpawnCmd::Laser {
                    position,
                    angle: angle.to_radians(),
                    durations,
                }
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
            .register_fn("set_render_warmup", SpawnCmd::ShowWarmup)
            .register_fn("set_render", SpawnCmd::SetRender)
            .register_fn("clear_enemies", || SpawnCmd::ClearEnemies);

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

impl<'lua> rlua::FromLua<'lua> for SongMap {
    fn from_lua(lua_value: rlua::Value<'lua>, lua: rlua::Context<'lua>) -> rlua::Result<Self> {
        let mut songmap = SongMap::default();
        // dump_value(&lua_value);
        let table = Table::from_lua(lua_value, lua)?;
        for entry in table.sequence_values() {
            let entry = Table::from_lua(entry?, lua)?;

            if let Ok(bpm) = get_key::<f64>(&entry, "bpm") {
                songmap.set_bpm(bpm);
            } else if let Ok(skip) = get_key::<f64>(&entry, "skip") {
                songmap.set_skip_amount(skip);
            } else {
                let action = BeatAction::from_table(entry, lua)?;
                songmap.add_action(action)
            }
        }

        Ok(songmap)
    }
}

impl BeatAction {
    fn from_table<'lua>(
        beat_action: rlua::Table<'lua>,
        lua: rlua::Context<'lua>,
    ) -> rlua::Result<Self> {
        let start_time = get_key::<f64>(&beat_action, "beat")?;
        let group_number = get_key::<usize>(&beat_action, "enemygroup")?;
        let action = SpawnCmd::from_table(beat_action, lua)?;

        Ok(BeatAction::new(Beats(start_time), group_number, action))
    }
}

impl SpawnCmd {
    fn from_table<'lua>(
        spawn_cmd: rlua::Table<'lua>,
        lua: rlua::Context<'lua>,
    ) -> rlua::Result<Self> {
        match get_key::<String>(&spawn_cmd, "spawn_cmd")?.as_str() {
            "bullet" => {
                let start = get_key::<LiveWorldPos>(&spawn_cmd, "start_pos")?;
                let end = get_key::<LiveWorldPos>(&spawn_cmd, "end_pos")?;
                Ok(SpawnCmd::Bullet { start, end })
            }
            "laser" => {
                let durations = get_key::<EnemyDurations>(&spawn_cmd, "durations")
                    .unwrap_or_else(|_| EnemyDurations::default_laser(Beats(1.0)));
                if spawn_cmd.contains_key("a")? {
                    let a = get_key::<LiveWorldPos>(&spawn_cmd, "a")?;
                    let b = get_key::<LiveWorldPos>(&spawn_cmd, "b")?;
                    Ok(SpawnCmd::LaserThruPoints { a, b, durations })
                } else {
                    let position = get_key::<LiveWorldPos>(&spawn_cmd, "position")?;
                    let angle = get_key::<f64>(&spawn_cmd, "angle")?;
                    Ok(SpawnCmd::Laser {
                        position,
                        angle: angle.to_radians(),
                        durations,
                    })
                }
            }
            "bomb" => {
                let pos = get_key::<LiveWorldPos>(&spawn_cmd, "pos")?;
                Ok(SpawnCmd::CircleBomb { pos })
            }
            "set_rotation_on" => {
                let start_angle = get_key::<f64>(&spawn_cmd, "start_angle")?;
                let end_angle = get_key::<f64>(&spawn_cmd, "end_angle")?;
                let duration = get_key::<f64>(&spawn_cmd, "duration")?;
                let rot_point = get_key::<LiveWorldPos>(&spawn_cmd, "rot_point")?;

                Ok(SpawnCmd::SetGroupRotation(Some((
                    start_angle.to_radians(),
                    end_angle.to_radians(),
                    Beats(duration),
                    rot_point,
                ))))
            }
            "set_rotation_off" => Ok(SpawnCmd::SetGroupRotation(None)),
            "set_fadeout_on" => {
                let color = if spawn_cmd.contains_key("color")? {
                    let color = get_key::<rlua::Value>(&spawn_cmd, "color")?;
                    from_lua_color(color)?
                } else {
                    println!("asdf");
                    Color::new(1.0, 1.0, 1.0, 0.0)
                };
                let duration = get_key::<f64>(&spawn_cmd, "duration")?;
                Ok(SpawnCmd::SetFadeOut(Some((color, Beats(duration)))))
            }
            "set_fadeout_off" => Ok(SpawnCmd::SetFadeOut(None)),
            "set_render" => {
                let value = get_key::<bool>(&spawn_cmd, "value")?;
                Ok(SpawnCmd::SetRender(value))
            }
            "set_hitbox" => {
                let value = get_key::<bool>(&spawn_cmd, "value")?;
                Ok(SpawnCmd::SetHitbox(value))
            }
            "clear_enemies" => Ok(SpawnCmd::ClearEnemies),
            x => Err(rlua::Error::FromLuaConversionError {
                from: "table",
                to: "SpawnCmd",
                message: Some(format!("Unknown spawn_cmd: {}", x)),
            }),
        }
    }
}

impl<'lua> rlua::FromLua<'lua> for LiveWorldPos {
    fn from_lua(lua_value: rlua::Value<'lua>, _lua: rlua::Context<'lua>) -> rlua::Result<Self> {
        match lua_value {
            rlua::Value::String(string) => match string.to_str()? {
                "player" => Ok(LiveWorldPos::PlayerPos),
                x => Err(rlua::Error::FromLuaConversionError {
                    from: "string",
                    to: "LiveWorldPos",
                    message: Some(format!("Invalid LiveWorldPos type: {:?}", x)),
                }),
            },
            rlua::Value::Table(table) => {
                if let Ok(offset) = get_key::<LiveWorldPos>(&table, "offset_from") {
                    Ok(LiveWorldPos::OffsetPlayer(Box::new(offset)))
                } else {
                    let x = get_key::<f64>(&table, "x")?;
                    let y = get_key::<f64>(&table, "y")?;
                    Ok(LiveWorldPos::from((x, y)))
                }
            }
            x => Err(rlua::Error::FromLuaConversionError {
                from: "lua value",
                to: "LiveWorldPos",
                message: Some(format!("Expected a String or Table. Got: {:?}", x)),
            }),
        }
    }
}

impl<'lua> rlua::FromLua<'lua> for EnemyDurations {
    fn from_lua(lua_value: rlua::Value<'lua>, lua: rlua::Context<'lua>) -> rlua::Result<Self> {
        let table = Table::from_lua(lua_value, lua)?;

        let warmup = get_key::<f64>(&table, "warmup")?;
        let active = get_key::<f64>(&table, "active")?;
        let cooldown = get_key::<f64>(&table, "cooldown")?;

        Ok(EnemyDurations {
            warmup: Beats(warmup),
            active: Beats(active),
            cooldown: Beats(cooldown),
        })
    }
}

fn dump_value(value: &rlua::Value) {
    match value.clone() {
        rlua::Value::Table(table) => {
            for pair in table.clone().pairs() {
                match pair {
                    Ok((key, value)) => {
                        dump_value(&key);
                        dump_value(&value);
                    }
                    Err(err) => println!("Err: {:?}", err),
                }
            }
        }
        value => println!("{:?}", value),
    }
}

// This is the ggez Color struct, so I can't implement rlua::FromLua on it, but
// I can just make a function.
fn from_lua_color(lua_value: rlua::Value) -> rlua::Result<Color> {
    match lua_value {
        rlua::Value::String(color_name) => match color_name.to_str()? {
            "red" => Ok(Color::new(1.0, 0.0, 0.0, 1.0)),
            "green" => Ok(Color::new(0.0, 1.0, 0.0, 1.0)),
            "blue" => Ok(Color::new(0.0, 0.0, 1.0, 1.0)),
            "black" => Ok(Color::new(0.0, 0.0, 0.0, 1.0)),
            "white" => Ok(Color::new(1.0, 1.0, 1.0, 1.0)),
            "transparent" => Ok(Color::new(1.0, 1.0, 1.0, 0.0)),
            "transparent_black" => Ok(Color::new(0.0, 0.0, 0.0, 0.0)),
            _ => Err(rlua::Error::FromLuaConversionError {
                from: "string",
                to: "Color",
                message: Some(format!("Unknown color: {:?}", color_name)),
            }),
        },
        rlua::Value::Table(table) => {
            let r = get_key::<f32>(&table, "r")?;
            let g = get_key::<f32>(&table, "g")?;
            let b = get_key::<f32>(&table, "b")?;
            let a = get_key::<f32>(&table, "a").unwrap_or(1.0);
            Ok(Color::new(r, g, b, a))
        }
        x => Err(rlua::Error::FromLuaConversionError {
            from: "lua value",
            to: "Color",
            message: Some(format!("Expected a String or Table. Got: {:?}", x)),
        }),
    }
}

fn get_key<'lua, T: rlua::FromLua<'lua>>(table: &Table<'lua>, key: &'lua str) -> rlua::Result<T> {
    match table.get::<&str, T>(key) {
        Ok(ok) => Ok(ok),
        Err(err) => match err {
            rlua::Error::FromLuaConversionError { from, to, message } => {
                let message = match message {
                    None => Some(format!("[No message]. Key was: {:?}", key)),
                    Some(msg) => Some(format!("{}. Key was: {:?}", msg, key)),
                };
                Err(rlua::Error::FromLuaConversionError { from, to, message })
            }
            x => Err(x),
        },
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MarkedBeat {
    pub beat: Beats,
    pub percent: f64,
    pub pitch: f64,
}

impl MarkedBeat {
    /// The slice is assumed to be in sorted order and the last beat is assumed
    /// to be the duration of the whole slice.
    pub fn mark_beats(beats: &[(Beats, f64)]) -> Vec<MarkedBeat> {
        if beats.is_empty() {
            return vec![];
        }
        let mut marked_beats = vec![];
        let duration = beats.last().unwrap().0;
        for &(beat, pitch) in beats {
            let marked_beat = MarkedBeat {
                beat,
                percent: beat.0 / duration.0,
                pitch,
            };
            marked_beats.push(marked_beat)
        }
        marked_beats
    }

    pub fn normalize_pitch(beats: &[MarkedBeat]) -> Vec<MarkedBeat> {
        if beats.is_empty() {
            return vec![];
        }
        let min = beats
            .iter()
            .map(|beat| beat.pitch)
            .reduce(f64::min)
            .unwrap();
        let max = beats
            .iter()
            .map(|beat| beat.pitch)
            .reduce(f64::max)
            .unwrap();

        beats
            .iter()
            .map(|beat| {
                let mut new_beat = *beat;
                new_beat.pitch = (beat.pitch - min) / (max - min);
                new_beat
            })
            .collect()
    }

    fn offset(beats: &[MarkedBeat], offset: Beats) -> Vec<MarkedBeat> {
        beats
            .iter()
            .map(|old| MarkedBeat {
                beat: old.beat + offset,
                percent: old.percent,
                pitch: old.pitch,
            })
            .collect()
    }
}

impl<'lua> rlua::ToLua<'lua> for MarkedBeat {
    fn to_lua(self, lua: rlua::Context<'lua>) -> rlua::Result<rlua::Value<'lua>> {
        let table = lua.create_table()?;
        table.set("beat", self.beat.0)?;
        table.set("percent", self.percent)?;
        table.set("pitch", self.pitch)?;
        Ok(rlua::Value::Table(table))
    }
}

pub fn midi_to_beats_grouped(track: &[TrackEvent], ticks_per_beat: f64) -> Vec<Vec<MarkedBeat>> {
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
                    beats.push(MarkedBeat::mark_beats(&group));
                    group.clear();
                }
            }

            let pitch = key.as_int() as f64 / midly::num::u7::max_value().as_int() as f64;
            group.push((beat, pitch));
            last_note = Some(key);
        }
    }

    beats
}

pub fn midi_to_beats_ungrouped(track: &[TrackEvent], ticks_per_beat: f64) -> Vec<MarkedBeat> {
    let mut tick_number = 0;
    let mut beats = vec![];

    for event in track {
        tick_number += event.delta.as_int();
        if let midly::TrackEventKind::Midi {
            message: midly::MidiMessage::NoteOn { key, .. },
            ..
        } = event.kind
        {
            let beat = Beats(tick_number as f64 / ticks_per_beat);
            let pitch = key.as_int() as f64 / midly::num::u7::max_value().as_int() as f64;
            beats.push((beat, pitch));
        }
    }
    MarkedBeat::mark_beats(&beats)
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
) -> anyhow::Result<T> {
    let buffer = std::fs::read(path)?;
    let smf = Smf::parse(&buffer)?;
    let ticks_per_beat = get_ticks_per_beat(&smf.header, bpm);
    Ok(func(&smf.tracks[0], ticks_per_beat))
}
