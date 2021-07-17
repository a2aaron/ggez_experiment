use std::io::Read;
use std::path::Path;

use ggez::graphics::Color;
use ggez::Context;
use midly::{Header, Smf, TrackEvent};
use rlua::{FromLua, Lua, Table};

use crate::chart::{BeatAction, LiveWorldPos, SpawnCmd};
use crate::ease::{Easing, EasingKind};
use crate::enemy::{EnemyDurations, Laser};
use crate::time;
use crate::time::Beats;

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

    fn set_bpm(&mut self, bpm: f64) {
        self.bpm = bpm;
    }

    fn set_skip_amount(&mut self, skip_amount: f64) {
        self.skip_amount = Beats(skip_amount);
    }

    fn add_action(&mut self, action: BeatAction) {
        self.actions.push(action);
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

impl<'lua> FromLua<'lua> for SongMap {
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
                if spawn_cmd.contains_key("angle")? {
                    let angle = get_key::<f64>(&spawn_cmd, "angle")?;
                    let length = get_key::<f64>(&spawn_cmd, "length")?;
                    if spawn_cmd.contains_key("start_pos")? {
                        let start = get_key::<LiveWorldPos>(&spawn_cmd, "start_pos")?;
                        Ok(SpawnCmd::BulletAngleStart {
                            angle: angle.to_radians(),
                            length,
                            start,
                        })
                    } else {
                        let end = get_key::<LiveWorldPos>(&spawn_cmd, "end_pos")?;
                        Ok(SpawnCmd::BulletAngleEnd {
                            angle: angle.to_radians(),
                            length,
                            end,
                        })
                    }
                } else {
                    let start = get_key::<LiveWorldPos>(&spawn_cmd, "start_pos")?;
                    let end = get_key::<LiveWorldPos>(&spawn_cmd, "end_pos")?;

                    Ok(SpawnCmd::Bullet { start, end })
                }
            }
            "laser" => {
                let durations = get_key_or(
                    &spawn_cmd,
                    "durations",
                    EnemyDurations::default_laser(Beats(1.0)),
                )?;

                let outline_colors = if spawn_cmd.contains_key("outline_colors")? {
                    let outline_colors: [rlua::Value; 4] = get_key(&spawn_cmd, "outline_colors")?;
                    [
                        Easing::<Color>::from_lua(outline_colors[0].clone(), lua)?,
                        Easing::<Color>::from_lua(outline_colors[1].clone(), lua)?,
                        Easing::<Color>::from_lua(outline_colors[2].clone(), lua)?,
                        Easing::<Color>::from_lua(outline_colors[3].clone(), lua)?,
                    ]
                } else {
                    Laser::default_outline_color()
                };

                let outline_keyframes = get_key_or(
                    &spawn_cmd,
                    "outline_keyframes",
                    Laser::default_outline_keyframes(),
                )?;

                if spawn_cmd.contains_key("a")? {
                    let a = get_key::<LiveWorldPos>(&spawn_cmd, "a")?;
                    let b = get_key::<LiveWorldPos>(&spawn_cmd, "b")?;
                    Ok(SpawnCmd::LaserThruPoints {
                        a,
                        b,
                        durations,
                        outline_colors,
                        outline_keyframes,
                    })
                } else {
                    let position = get_key::<LiveWorldPos>(&spawn_cmd, "position")?;
                    let angle = get_key::<f64>(&spawn_cmd, "angle")?;
                    Ok(SpawnCmd::Laser {
                        position,
                        angle: angle.to_radians(),
                        durations,
                        outline_colors,
                        outline_keyframes,
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
            "set_render_warmup" => {
                let value = get_key::<bool>(&spawn_cmd, "value")?;
                Ok(SpawnCmd::SetRenderWarmup(value))
            }
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

impl<'lua> FromLua<'lua> for LiveWorldPos {
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

impl<'lua> FromLua<'lua> for EnemyDurations {
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

impl<'lua, T: FromLua<'lua>> FromLua<'lua> for Easing<T> {
    fn from_lua(lua_value: rlua::Value<'lua>, lua: rlua::Context<'lua>) -> rlua::Result<Self> {
        let table = rlua::Table::from_lua(lua_value, lua)?;

        let start = get_key(&table, "start_val")?;
        let end = get_key(&table, "end_val")?;
        let kind = get_key_or(&table, "ease_kind", EasingKind::Linear)?;
        Ok(Easing { start, end, kind })
    }
}

impl<'lua> Easing<Color> {
    fn from_lua(lua_value: rlua::Value<'lua>, lua: rlua::Context<'lua>) -> rlua::Result<Self> {
        let table = rlua::Table::from_lua(lua_value, lua)?;

        let start = get_key_color(&table, "start_val")?;
        let end = get_key_color(&table, "end_val")?;
        let kind = get_key_or(&table, "ease_kind", EasingKind::Linear)?;
        Ok(Easing { start, end, kind })
    }
}

impl<'lua> FromLua<'lua> for EasingKind {
    fn from_lua(lua_value: rlua::Value<'lua>, _lua: rlua::Context<'lua>) -> rlua::Result<Self> {
        match lua_value {
            rlua::Value::String(string) => match string.to_str()? {
                "constant" => Ok(EasingKind::Constant),
                "linear" => Ok(EasingKind::Linear),
                "exponential" => Ok(EasingKind::Exponential),
                x => Err(rlua::Error::FromLuaConversionError {
                    from: "string",
                    to: "EasingKind",
                    message: Some(format!("Invalid EasingKind: {:?}", x)),
                }),
            },
            rlua::Value::Table(table) => {
                if table.contains_key("mid_val")? {
                    Ok(EasingKind::SplitLinear {
                        mid_val: get_key(&table, "mid_val")?,
                        mid_t: get_key(&table, "mid_t")?,
                    })
                } else {
                    Ok(EasingKind::EaseOut {
                        easing: Box::new(get_key(&table, "easing")?),
                    })
                }
            }
            x => Err(rlua::Error::FromLuaConversionError {
                from: "lua value",
                to: "EasingKind",
                message: Some(format!("Expected a String or Table. Got: {:?}", x)),
            }),
        }
    }
}

#[allow(dead_code)]
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

// This is the ggez Color struct, so I can't implement FromLua on it, but
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

fn get_key<'lua, T: FromLua<'lua>>(table: &Table<'lua>, key: &'lua str) -> rlua::Result<T> {
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

fn get_key_or<'lua, T: FromLua<'lua>>(
    table: &Table<'lua>,
    key: &'lua str,
    default: T,
) -> rlua::Result<T> {
    if table.contains_key(key)? {
        get_key(table, key)
    } else {
        Ok(default)
    }
}

fn get_key_color<'lua>(table: &Table<'lua>, key: &'lua str) -> rlua::Result<Color> {
    let value: rlua::Value = get_key(table, key)?;
    from_lua_color(value)
}

#[derive(Debug, Clone, Copy)]
pub struct MarkedBeat {
    pub beat: Beats,
    pub percent: f64,
    pub pitch: Option<f64>,
    // a triple describing the midigroup_id, the ith note into the midigroup, and
    // total length of the midigroup
    pub midigroup: Option<(usize, usize, usize)>,
}

impl<'lua> rlua::ToLua<'lua> for MarkedBeat {
    fn to_lua(self, lua: rlua::Context<'lua>) -> rlua::Result<rlua::Value<'lua>> {
        let table = lua.create_table()?;
        table.set("beat", self.beat.0)?;
        table.set("percent", self.percent)?;
        table.set("pitch", self.pitch)?;
        let (midigroup_id, midigroup_i, midigroup_len) = if let Some((id, i, len)) = self.midigroup
        {
            (Some(id), Some(i), Some(len))
        } else {
            (None, None, None)
        };
        table.set("midigroup_id", midigroup_id)?;
        table.set("midigroup_i", midigroup_i)?;
        table.set("midigroup_len", midigroup_len)?;
        Ok(rlua::Value::Table(table))
    }
}

fn get_track_duration(track: &[TrackEvent], ticks_per_beat: f64) -> Beats {
    let ticks: u32 = track.iter().map(|event| event.delta.as_int()).sum();
    Beats(ticks as f64 / ticks_per_beat)
}

fn normalized_absolute_pitch(pitch: midly::num::u7) -> f64 {
    pitch.as_int() as f64 / midly::num::u7::max_value().as_int() as f64
}

pub fn midi_to_beats_grouped(track: &[TrackEvent], ticks_per_beat: f64) -> Vec<MarkedBeat> {
    let ungrouped_beats = midi_to_beats_ungrouped(track, ticks_per_beat);
    let mut grouped_beats = vec![];
    let mut this_group = vec![];
    let mut last_note = None;

    for beat in ungrouped_beats {
        if let Some(last_note) = last_note {
            // this is okay because the pitches have no arithmetic done to them
            #[allow(clippy::float_cmp)]
            if last_note != beat.pitch.unwrap() {
                grouped_beats.push(this_group.clone());
                this_group.clear();
            }
        }

        this_group.push(beat);
        last_note = Some(beat.pitch.unwrap());
    }

    for (midigroup_id, group) in grouped_beats.iter_mut().enumerate() {
        let midigroup_len = group.len();
        for (midigroup_i, beat) in group.iter_mut().enumerate() {
            beat.midigroup = Some((midigroup_id, midigroup_i, midigroup_len));
        }
    }

    grouped_beats.into_iter().flatten().collect()
}

pub fn midi_to_beats_ungrouped(track: &[TrackEvent], ticks_per_beat: f64) -> Vec<MarkedBeat> {
    let mut tick_number = 0;
    let mut beats = vec![];

    let duration = get_track_duration(track, ticks_per_beat);

    for event in track {
        tick_number += event.delta.as_int();
        if let midly::TrackEventKind::Midi {
            message: midly::MidiMessage::NoteOn { key, .. },
            ..
        } = event.kind
        {
            let beat = Beats(tick_number as f64 / ticks_per_beat);

            beats.push(MarkedBeat {
                beat,
                percent: beat.0 / duration.0,
                pitch: Some(normalized_absolute_pitch(key)),
                midigroup: None,
            });
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
) -> anyhow::Result<T> {
    let buffer = std::fs::read(path)?;
    let smf = Smf::parse(&buffer)?;
    let ticks_per_beat = get_ticks_per_beat(&smf.header, bpm);
    Ok(func(&smf.tracks[0], ticks_per_beat))
}
