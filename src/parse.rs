/// This module handles all of the parsing of song chart files, which describe
/// how levels should be constructed. The actual parsing is handled by a bunch
/// of small nom parsers. The files are line based, and each line is parsed into
/// a SongChartCmd. These are used to drive the various things in SongMap. The
/// main output from SongMap is cmd_batches, which is used in chart.rs to actually
/// construct the list of BeatActions for the song.
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use ggez::Context;
use midly::Smf;

use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{alpha1, alphanumeric0, char, none_of, space0, space1};
use nom::combinator::{complete, map, map_opt, recognize};
use nom::multi::{many0, separated_list0};
use nom::number::complete::double;
use nom::sequence::{delimited, preceded, separated_pair, terminated, tuple};
use nom::{Finish, IResult, Parser};

use anyhow::{anyhow, bail};
use derive_more::Display;

use crate::chart::{BeatSplitter, CmdBatch, CmdBatchPos, LiveWorldPos};
use crate::time;
use crate::time::Beats;
use crate::world::WorldPos;

/// This struct essentially acts as an interpreter for a song's file. All parsing
/// occurs before the actual level is played, with the file format being line
/// based.
pub struct SongMap {
    pub skip_amount: Beats,
    pub bpm: f64,
    pub midi_beats: HashMap<String, Vec<Beats>>,
    pub cmd_batches: Vec<(TimingData, CmdBatch)>,
    positions: Positions,
}

impl SongMap {
    /// Parse a file. Note that the only errors this function returns are errors
    /// in reading the file. Any parsing errors are printed to stdout, but are
    /// ignored otherwise.
    pub fn parse_file<P: AsRef<Path>>(
        ctx: &mut Context,
        path: P,
    ) -> Result<SongMap, Box<dyn std::error::Error>> {
        let mut file = ggez::filesystem::open(ctx, path)?;
        let mut buf = String::new();
        file.read_to_string(&mut buf)?;
        let mut map = SongMap::default();
        for (line_num, raw_line) in buf.lines().enumerate() {
            let raw_line = raw_line.trim();
            // skip comments
            if raw_line.starts_with('#') || raw_line.is_empty() {
                continue;
            }
            match parse(raw_line).finish() {
                Err(err) => println!(
                    "{}: Warning: Couldn't parse line \"{}\".\n\tReason: {:#?}",
                    line_num + 1,
                    raw_line,
                    err
                ),
                Ok((remaining, cmd)) => {
                    if let Err(warning) = map.execute_cmd(ctx, &cmd) {
                        println!(
                            "Warning on line {} \"{}\":\n\t{}. \n\tParsed as: {:#?}, {:#?}",
                            line_num + 1,
                            raw_line,
                            warning,
                            remaining,
                            cmd
                        );
                    }
                }
            }
        }
        Ok(map)
    }

    fn execute_cmd(&mut self, ctx: &mut Context, cmd: &SongChartCmds) -> anyhow::Result<()> {
        match cmd {
            SongChartCmds::Skip(skip) => self.skip_amount = Beats(*skip),
            SongChartCmds::Bpm(bpm) => self.bpm = *bpm,
            SongChartCmds::MidiBeat(name, path) => {
                match parse_midi_to_beats(ctx, &path, self.bpm) {
                    Ok(beats) => {
                        if self.midi_beats.insert(name.clone(), beats).is_some() {
                            bail!("Replaced midibeat {}.", name);
                        }
                    }
                    Err(err) => bail!("Error parsing midi, reason: {}", err),
                }
            }
            SongChartCmds::Position(name, pos) => {
                let pos = WorldPos::from(*pos);
                if self.positions.0.insert(name.clone(), pos).is_some() {
                    bail!("Replaced position {}.", name);
                }
            }
            SongChartCmds::SpawnEnemy(kwargs) => {
                let timing_data = KwargList::make_timing_data(&kwargs, &self.midi_beats)?;
                let cmd_batch = KwargList::make_enemy(&kwargs, &self.positions)?;

                self.cmd_batches.push((timing_data, cmd_batch))
            }
        }
        Ok(())
    }

    pub fn get_beats(&self, name: &str) -> Vec<Beats> {
        match self.midi_beats.get(name) {
            Some(beats) => beats.clone(),
            None => {
                println!(
                    "Warning: Could not find midi beat {}. Returning empty vector...",
                    name
                );
                vec![]
            }
        }
    }
}

impl Default for SongMap {
    fn default() -> Self {
        SongMap {
            skip_amount: Beats(0.0),
            bpm: 150.0,
            midi_beats: HashMap::new(),
            positions: Positions(HashMap::new()),
            cmd_batches: vec![],
        }
    }
}

struct Positions(HashMap<String, WorldPos>);
impl Positions {
    fn lookup(&self, input: &TokenValue) -> anyhow::Result<LiveWorldPos> {
        match input {
            TokenValue::String(name) => match name.as_str() {
                "player" => Ok(LiveWorldPos::PlayerPos),
                name => {
                    let pos = self
                        .0
                        .get(name)
                        .cloned()
                        .ok_or_else(|| anyhow!("Couldn't find position {}", name))?;
                    Ok(LiveWorldPos::from(pos))
                }
            },
            TokenValue::Tuple(vec) => match vec[..] {
                [TokenValue::Float(x), TokenValue::Float(y)] => Ok(LiveWorldPos::from((x, y))),
                _ => bail!("Expected tuple of two floats, got {:?}", input),
            },
            _ => bail!("Expected tuple of two floats or string, got {:?}", input),
        }
    }
}

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

type TypePredicate = dyn Fn(&TokenValue) -> bool;

struct KwargList {
    kwargs: HashMap<String, TokenValue>,
}

impl KwargList {
    fn new(
        kwargs: &[(String, TokenValue)],
        required: &[(&str, &[&TypePredicate])],
        optional: &[(&str, &[&TypePredicate])],
    ) -> Result<KwargList, KwargError> {
        let mut hash_map = HashMap::new();
        for kwarg in kwargs.iter().cloned() {
            if let Some(old) = hash_map.insert(kwarg.0.clone(), kwarg.1.clone()) {
                return Err(KwargError::DuplicateKwarg(kwarg.0, kwarg.1, old));
            }
        }

        fn type_check(
            kwarg: &str,
            kwarg_val: &TokenValue,
            allowed_values: &[impl Fn(&TokenValue) -> bool],
        ) -> Result<(), KwargError> {
            if !allowed_values.iter().any(|check| check(&kwarg_val)) {
                Err(KwargError::ValueMismatch(
                    kwarg.to_owned(),
                    kwarg_val.clone(),
                ))
            } else {
                Ok(())
            }
        }

        for &(required_kwarg, allowed_types) in required {
            match hash_map.get(required_kwarg) {
                None => {
                    return Err(KwargError::MissingKwarg(required_kwarg.to_owned()));
                }
                Some(kwarg_val) => type_check(required_kwarg, kwarg_val, allowed_types)?,
            }
        }
        for &(optional_kwarg, allowed_types) in optional {
            match hash_map.get(optional_kwarg) {
                None => continue,
                Some(kwarg_val) => type_check(optional_kwarg, kwarg_val, allowed_types)?,
            }
        }

        Ok(KwargList { kwargs: hash_map })
    }

    fn get(&self, kwarg: &str) -> Option<TokenValue> {
        self.kwargs.get(kwarg).cloned()
    }

    fn get_float(&self, kwarg: &str) -> Option<f64> {
        match self.get(kwarg) {
            Some(TokenValue::Float(x)) => Some(x),
            _ => None,
        }
    }

    fn get_string(&self, kwarg: &str) -> Option<String> {
        match self.get(kwarg) {
            Some(TokenValue::String(x)) => Some(x),
            _ => None,
        }
    }

    fn make_timing_data(
        kwargs: &[(String, TokenValue)],
        midibeats: &HashMap<String, Vec<Beats>>,
    ) -> anyhow::Result<TimingData> {
        let splitter = KwargList::make_splitter(kwargs);
        if let Ok(splitter) = splitter {
            return Ok(TimingData::BeatSplitter(splitter));
        }

        let midibeat = KwargList::make_midibeat(kwargs, midibeats);
        if let Ok(midibeat) = midibeat {
            Ok(TimingData::MidiBeat(midibeat))
        } else {
            bail!("Failed to make beat splitter and midibeat with kwargs. splitter: {:?}, midibeat: {:?}", splitter, midibeat)
        }
    }

    fn make_splitter(kwargs: &[(String, TokenValue)]) -> Result<BeatSplitter, KwargError> {
        let kwargs = KwargList::new(
            &kwargs,
            &[
                ("start", &[&TokenValue::is_float]),
                ("freq", &[&TokenValue::is_float]),
            ],
            &[
                ("delay", &[&TokenValue::is_float]),
                ("offset", &[&TokenValue::is_float]),
                ("duration", &[&TokenValue::is_float]),
            ],
        )?;
        Ok(BeatSplitter {
            start: kwargs.get_float("start").unwrap(),
            frequency: kwargs.get_float("freq").unwrap(),
            offset: kwargs.get_float("offset").unwrap_or(0.0),
            delay: kwargs.get_float("delay").unwrap_or(0.0),
            duration: kwargs.get_float("duration").unwrap_or(4.0 * 4.0),
        })
    }

    fn make_midibeat(
        kwargs: &[(String, TokenValue)],
        midibeats: &HashMap<String, Vec<Beats>>,
    ) -> anyhow::Result<(f64, Vec<Beats>)> {
        let kwargs = KwargList::new(
            &kwargs,
            &[
                ("start", &[&TokenValue::is_float]),
                ("midibeat", &[&TokenValue::is_string]),
            ],
            &[],
        )?;
        let name = kwargs.get_string("midibeat").unwrap();
        let start = kwargs.get_float("start").unwrap();
        match midibeats.get(&name).cloned() {
            Some(midibeat) => Ok((start, midibeat)),
            None => Err(anyhow!("Couldn't find midibeat {}", name)),
        }
    }

    fn make_enemy(
        kwargs: &[(String, TokenValue)],
        positions: &Positions,
    ) -> anyhow::Result<CmdBatch> {
        fn is_enemy_type(ty: &TokenValue) -> bool {
            matches!(ty, TokenValue::String(x) if x == "bullet" || x == "laser" || x == "bomb")
        }

        let kwarg_list = KwargList::new(&kwargs, &[("enemy", &[&is_enemy_type])], &[])?;

        let cmd_batch = match kwarg_list.get_string("enemy").unwrap().as_str() {
            "bullet" => {
                let (start, end) = KwargList::make_two_point_enemy(kwargs, positions)?;
                CmdBatch::Bullet { start, end }
            }
            "laser" => {
                let (a, b) = KwargList::make_two_point_enemy(kwargs, positions)?;
                CmdBatch::Laser { a, b }
            }
            "bomb" => KwargList::make_bomb(kwargs, positions)?,
            x => bail!("Invalid enemy type: {}", x),
        };

        Ok(cmd_batch)
    }

    fn make_two_point_enemy(
        kwargs: &[(String, TokenValue)],
        positions: &Positions,
    ) -> anyhow::Result<(CmdBatchPos, CmdBatchPos)> {
        fn is_lerper(ty: &TokenValue) -> bool {
            match ty {
                TokenValue::Tuple(vec) if vec.len() == 4 => vec
                    .iter()
                    .all(|ty| TokenValue::is_float_tuple(ty) || TokenValue::is_string(ty)),
                _ => false,
            }
        }
        let kwargs = KwargList::new(&kwargs, &[("lerps", &[&is_lerper])], &[])?;

        let lerps = kwargs.get("lerps").unwrap();
        let (start_1, end_1, start_2, end_2) = match lerps {
            TokenValue::Tuple(vec) => (
                positions.lookup(&vec[0])?,
                positions.lookup(&vec[1])?,
                positions.lookup(&vec[2])?,
                positions.lookup(&vec[3])?,
            ),
            _ => unreachable!(),
        };

        fn to_cmd_batch_pos(start: LiveWorldPos, end: LiveWorldPos) -> anyhow::Result<CmdBatchPos> {
            match (start, end) {
                (LiveWorldPos::Constant(start), LiveWorldPos::Constant(end)) => {
                    Ok(CmdBatchPos::Lerped(start.tuple(), end.tuple()))
                }
                (LiveWorldPos::PlayerPos, LiveWorldPos::PlayerPos) => {
                    Ok(CmdBatchPos::Constant(LiveWorldPos::PlayerPos))
                }
                _ => bail!(
                    "Invalid CmdBatchPos combination: start: {:?} end: {:?}",
                    start,
                    end
                ),
            }
        }

        Ok((
            to_cmd_batch_pos(start_1, start_2)?,
            to_cmd_batch_pos(end_1, end_2)?,
        ))
    }

    fn make_bomb(
        kwargs: &[(String, TokenValue)],
        positions: &Positions,
    ) -> anyhow::Result<CmdBatch> {
        let kwargs = KwargList::new(
            &kwargs,
            &[("at", &[&TokenValue::is_string, &TokenValue::is_float_tuple])],
            &[],
        )?;

        let pos = match kwargs.get("at").unwrap() {
            TokenValue::String(grid) if grid == *"grid" => CmdBatchPos::RandomGrid,
            x => CmdBatchPos::Constant(positions.lookup(&x)?),
        };
        let cmd_batch = CmdBatch::CircleBomb { pos };
        Ok(cmd_batch)
    }
}

#[derive(Debug)]
enum KwargError {
    MissingKwarg(String),
    ValueMismatch(String, TokenValue),
    DuplicateKwarg(String, TokenValue, TokenValue),
}

impl std::fmt::Display for KwargError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KwargError::MissingKwarg(kwarg) => write!(f, "Missing required kwarg: {}", kwarg),
            KwargError::DuplicateKwarg(key, val1, val2) => {
                write!(f, "Duplicate kwargs: {}={} vs {}={}", key, val1, key, val2)
            }
            KwargError::ValueMismatch(key, val) => {
                write!(f, "Wrong kwarg value for {}={}", key, val,)
            }
        }
    }
}

impl std::error::Error for KwargError {}

#[derive(Debug, Clone, PartialEq)]
pub enum SongChartCmds {
    Skip(f64),
    Bpm(f64),
    MidiBeat(String, String),
    Position(String, (f64, f64)),
    SpawnEnemy(Vec<(String, TokenValue)>),
}

#[derive(Display, Debug, Clone, PartialEq)]
pub enum TokenValue {
    String(String),
    Float(f64),
    #[display(fmt = "{:?}", _0)]
    Tuple(Vec<TokenValue>),
}

impl TokenValue {
    fn is_float(ty: &TokenValue) -> bool {
        matches!(ty, TokenValue::Float(_))
    }

    fn is_string(ty: &TokenValue) -> bool {
        matches!(ty, TokenValue::String(_))
    }

    fn is_float_tuple(ty: &TokenValue) -> bool {
        match ty {
            TokenValue::Tuple(vec) => {
                matches!(vec[..], [TokenValue::Float(_), TokenValue::Float(_)])
            }
            _ => false,
        }
    }
}

fn tag_ws0<'i>(the_tag: &'static str) -> impl FnMut(&'i str) -> IResult<&'i str, &'i str> {
    terminated(tag(the_tag), space0)
}

fn ws0_tag<'i>(the_tag: &'static str) -> impl FnMut(&'i str) -> IResult<&'i str, &'i str> {
    preceded(space0, tag(the_tag))
}

fn ws0_tag_ws0<'i>(the_tag: &'static str) -> impl FnMut(&'i str) -> IResult<&'i str, &'i str> {
    delimited(space0, tag(the_tag), space0)
}

fn tag_ws1<'i>(the_tag: &'static str) -> impl FnMut(&'i str) -> IResult<&'i str, &'i str> {
    terminated(tag(the_tag), space1)
}

fn tuple_value(input: &str) -> IResult<&str, Vec<TokenValue>> {
    delimited(
        tag_ws0("("),
        separated_list0(ws0_tag_ws0(","), value),
        ws0_tag(")"),
    )(input)
}

fn quoted_string(input: &str) -> IResult<&str, String> {
    let the_string = recognize(many0(none_of("\""))).map(String::from);
    delimited(char('"'), the_string, char('"'))(input)
}

fn string(input: &str) -> IResult<&str, String> {
    let normal_string = recognize(tuple((alpha1, alphanumeric0))).map(String::from);
    alt((quoted_string, normal_string))(input)
}

fn value(input: &str) -> IResult<&str, TokenValue> {
    alt((
        map(tuple_value, TokenValue::Tuple),
        map(double, TokenValue::Float),
        map(string, TokenValue::String),
    ))(input)
}

fn kwarg(input: &str) -> IResult<&str, (String, TokenValue)> {
    separated_pair(string, char('='), value)(input)
}

pub fn parse(input: &str) -> IResult<&str, SongChartCmds> {
    let parser = alt((
        preceded(tag_ws1("SKIP"), map(double, SongChartCmds::Skip)),
        preceded(tag_ws1("BPM"), map(double, SongChartCmds::Bpm)),
        preceded(
            tag_ws1("midibeat"),
            map(separated_pair(string, space1, string), |(name, path)| {
                SongChartCmds::MidiBeat(name, path)
            }),
        ),
        preceded(
            tag_ws1("position"),
            map_opt(
                separated_pair(string, space1, tuple_value),
                |(name, pos)| match pos[..] {
                    [TokenValue::Float(x), TokenValue::Float(y)] => {
                        Some(SongChartCmds::Position(name, (x, y)))
                    }
                    _ => None,
                },
            ),
        ),
        preceded(
            tag_ws1("spawn"),
            map(separated_list0(space1, kwarg), |vec| {
                SongChartCmds::SpawnEnemy(vec)
            }),
        ),
    ));
    complete(parser)(input)
}

pub fn parse_midi_to_beats<P: AsRef<Path>>(
    ctx: &mut Context,
    path: P,
    bpm: f64,
) -> Result<Vec<Beats>, Box<dyn std::error::Error>> {
    let mut file = ggez::filesystem::open(ctx, path)?;
    let mut buffer = vec![];
    file.read_to_end(&mut buffer)?;
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

#[cfg(test)]
mod test {
    use crate::parse::{kwarg, parse, string, tuple_value, SongChartCmds, TokenValue};

    #[test]
    fn test_parse_strings() {
        let input = "thestring";
        assert_eq!(string(input), Ok(("", "thestring".to_owned())));

        let input = "\"string literal\"";
        assert_eq!(string(input), Ok(("", "string literal".to_owned())));

        let input = "splits spaces";
        assert_eq!(string(input), Ok((" spaces", "splits".to_owned())));
    }

    #[test]
    fn test_parse_float_tuple() {
        let input = "(1.0,2.0)";
        let expected_tuple = vec![TokenValue::Float(1.0), TokenValue::Float(2.0)];
        let actual = tuple_value(input);
        assert_eq!(actual, Ok(("", expected_tuple)));

        let input = "(1.0, 2.0)";
        let expected_tuple = vec![TokenValue::Float(1.0), TokenValue::Float(2.0)];
        let actual = tuple_value(input);
        assert_eq!(actual, Ok(("", expected_tuple)));

        let input = "(   1.0 \t  , \t  2.0  )";
        let expected_tuple = vec![TokenValue::Float(1.0), TokenValue::Float(2.0)];
        let actual = tuple_value(input);
        assert_eq!(actual, Ok(("", expected_tuple)));

        let input = "(-5.0,-0.0)";
        let expected_tuple = vec![TokenValue::Float(-5.0), TokenValue::Float(0.0)];
        let actual = tuple_value(input);
        assert_eq!(actual, Ok(("", expected_tuple)));
    }

    #[test]
    fn test_parse_simple_tuple() {
        let input = "(hello, world)";
        let expected_tuple = vec![
            TokenValue::String("hello".to_owned()),
            TokenValue::String("world".to_owned()),
        ];
        let actual = tuple_value(input);
        assert_eq!(actual, Ok(("", expected_tuple)));
    }

    #[test]
    fn test_parse_kwarg() {
        let input = "value=string";
        let actual = kwarg(input);
        assert_eq!(
            actual,
            Ok((
                "",
                ("value".to_owned(), TokenValue::String("string".to_owned()))
            ))
        );

        let input = "value=5.0";
        let actual = kwarg(input);
        assert_eq!(
            actual,
            Ok(("", ("value".to_owned(), TokenValue::Float(5.0))))
        );

        let input = "value=(5.0, 4.0)";
        let actual = kwarg(input);
        let expected_tuple = vec![TokenValue::Float(5.0), TokenValue::Float(4.0)];
        assert_eq!(
            actual,
            Ok(("", ("value".to_owned(), TokenValue::Tuple(expected_tuple))))
        );
    }

    #[test]
    fn test_parse_spawn1() {
        let input = "spawn start=16.0 freq=4.0 lerpstart=botleft lerpend=botright";
        let actual = parse(input);
        assert_eq!(
            actual,
            Ok((
                "",
                SongChartCmds::SpawnEnemy(vec![
                    ("start".to_owned(), TokenValue::Float(16.0)),
                    ("freq".to_owned(), TokenValue::Float(4.0),),
                    (
                        "lerpstart".to_owned(),
                        TokenValue::String("botleft".to_owned()),
                    ),
                    (
                        "lerpend".to_owned(),
                        TokenValue::String("botright".to_owned()),
                    )
                ])
            ))
        );
    }

    #[test]
    fn test_parse_spawn2() {
        let input = "spawn start=16.0 freq=4.0 lerpstart=(-50.0,50.0) lerpend=botright";
        let actual = parse(input);
        let expected_tuple = vec![TokenValue::Float(-50.0), TokenValue::Float(50.0)];
        assert_eq!(
            actual,
            Ok((
                "",
                SongChartCmds::SpawnEnemy(vec![
                    ("start".to_owned(), TokenValue::Float(16.0)),
                    ("freq".to_owned(), TokenValue::Float(4.0),),
                    ("lerpstart".to_owned(), TokenValue::Tuple(expected_tuple),),
                    (
                        "lerpend".to_owned(),
                        TokenValue::String("botright".to_owned()),
                    )
                ])
            ))
        );
    }
}
