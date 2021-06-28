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

pub struct SongMap {
    pub skip_amount: Beats,
    pub bpm: f64,
    pub midi_beats: HashMap<String, Vec<Beats>>,
    positions: HashMap<String, WorldPos>,
    pub cmd_batches: Vec<(BeatSplitter, CmdBatch)>,
}

impl SongMap {
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
                if self.positions.insert(name.clone(), pos).is_some() {
                    bail!("Replaced position {}.", name);
                }
            }
            SongChartCmds::BulletLerper(kwargs) => {
                fn is_float(ty: &TokenType) -> bool {
                    *ty == TokenType::Float
                }
                fn is_string(ty: &TokenType) -> bool {
                    *ty == TokenType::String
                }
                fn is_float_tuple(ty: &TokenType) -> bool {
                    *ty == TokenType::Tuple(vec![TokenType::Float; 2])
                }
                fn is_lerper(ty: &TokenType) -> bool {
                    match ty {
                        TokenType::Tuple(vec) if vec.len() == 4 => {
                            vec.iter().all(|ty| is_float_tuple(ty) || is_string(ty))
                        }
                        _ => false,
                    }
                }

                let kwargs = KwargList::new(
                    &kwargs,
                    &[
                        ("start", &[&is_float]),
                        ("freq", &[&is_float]),
                        ("lerps", &[&is_lerper]),
                    ],
                    &[("delay", &[&is_float]), ("offset", &[&is_float])],
                )?;
                let start = kwargs.get_float("start").unwrap();
                let freq = kwargs.get_float("freq").unwrap();
                let offset = kwargs.get_float("offset");
                let delay = kwargs.get_float("delay");

                let lerps = kwargs.get("lerps").unwrap();
                let (start_1, end_1, start_2, end_2) = match lerps {
                    TokenValue::Tuple(vec) => (
                        self.lookup_position(&vec[0])?,
                        self.lookup_position(&vec[1])?,
                        self.lookup_position(&vec[2])?,
                        self.lookup_position(&vec[3])?,
                    ),
                    _ => unreachable!(),
                };

                fn to_cmd_batch_pos(start: LiveWorldPos, end: LiveWorldPos) -> CmdBatchPos {
                    match (start, end) {
                        (LiveWorldPos::Constant(start), LiveWorldPos::Constant(end)) => {
                            CmdBatchPos::Lerped(start.tuple(), end.tuple())
                        }
                        (LiveWorldPos::PlayerPos, LiveWorldPos::PlayerPos) => {
                            CmdBatchPos::Constant(LiveWorldPos::PlayerPos)
                        }
                        _ => todo!(), // this should probably return None here
                    }
                }

                let cmd_batch = CmdBatch::Bullet {
                    start: to_cmd_batch_pos(start_1, start_2),
                    end: to_cmd_batch_pos(end_1, end_2),
                };
                let splitter = BeatSplitter {
                    start,
                    frequency: freq,
                    offset: offset.unwrap_or(0.0),
                    delay: delay.unwrap_or(0.0),
                    ..Default::default()
                };
                self.cmd_batches.push((splitter, cmd_batch))
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

    fn lookup_position(&self, input: &TokenValue) -> anyhow::Result<LiveWorldPos> {
        match input.get_type() {
            TokenType::String => (),
            TokenType::Tuple(x) if x == vec![TokenType::Float; 2] => (),
            x => {
                bail!(
                    "Wrong type for PositionData. Expected String or Tuple. Got {}.",
                    x
                )
            }
        };
        match input {
            TokenValue::String(name) => match name.as_str() {
                "player" => Ok(LiveWorldPos::PlayerPos),
                name => {
                    let pos = self
                        .positions
                        .get(name)
                        .cloned()
                        .ok_or_else(|| anyhow!("Couldn't find position {}", name))?;
                    Ok(LiveWorldPos::from(pos))
                }
            },
            TokenValue::Tuple(vec) => {
                let x = vec[0].as_float();
                let y = vec[1].as_float();
                Ok(LiveWorldPos::from((x, y)))
            }
            _ => unreachable!(),
        }
    }
}

impl Default for SongMap {
    fn default() -> Self {
        SongMap {
            skip_amount: Beats(0.0),
            bpm: 150.0,
            midi_beats: HashMap::new(),
            positions: HashMap::new(),
            cmd_batches: vec![],
        }
    }
}

type TypePredicate = dyn Fn(&TokenType) -> bool;

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
            allowed_types: &[impl Fn(&TokenType) -> bool],
        ) -> Result<(), KwargError> {
            if !allowed_types
                .iter()
                .any(|check| check(&kwarg_val.get_type()))
            {
                Err(KwargError::TypeMismatch(
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
}

#[derive(Debug)]
enum KwargError {
    MissingKwarg(String),
    TypeMismatch(String, TokenValue),
    DuplicateKwarg(String, TokenValue, TokenValue),
}

impl std::fmt::Display for KwargError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KwargError::MissingKwarg(kwarg) => write!(f, "Missing required kwarg: {}", kwarg),
            KwargError::DuplicateKwarg(key, val1, val2) => {
                write!(f, "Duplicate kwargs: {}={} vs {}={}", key, val1, key, val2)
            }
            KwargError::TypeMismatch(key, val) => write!(
                f,
                "Wrong kwarg type for {}={}, got {}",
                key,
                val,
                val.get_type(),
            ),
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
    BulletLerper(Vec<(String, TokenValue)>),
}

#[derive(Display, Debug, Clone, PartialEq)]
pub enum TokenValue {
    String(String),
    Float(f64),
    #[display(fmt = "{:?}", _0)]
    Tuple(Vec<TokenValue>),
}

impl TokenValue {
    fn get_type(&self) -> TokenType {
        match self {
            TokenValue::String(_) => TokenType::String,
            TokenValue::Float(_) => TokenType::Float,
            TokenValue::Tuple(values) => {
                TokenType::Tuple(values.iter().map(|x| x.get_type()).collect())
            }
        }
    }

    fn as_float(&self) -> f64 {
        match self {
            TokenValue::Float(x) => *x,
            _ => panic!("TokenValue must be float"),
        }
    }
}

#[derive(Display, Debug, Clone, PartialEq, Eq)]
pub enum TokenType {
    String,
    Float,
    #[display(fmt = "{:?}", _0)]
    Tuple(Vec<TokenType>),
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
            tag_ws1("bulletlerp"),
            map(separated_list0(space1, kwarg), |vec| {
                SongChartCmds::BulletLerper(vec)
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
    fn test_parse_bulletlerp1() {
        let input = "bulletlerp start=16.0 freq=4.0 lerpstart=botleft lerpend=botright";
        let actual = parse(input);
        assert_eq!(
            actual,
            Ok((
                "",
                SongChartCmds::BulletLerper(vec![
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
    fn test_parse_bulletlerp2() {
        let input = "bulletlerp start=16.0 freq=4.0 lerpstart=(-50.0,50.0) lerpend=botright";
        let actual = parse(input);
        let expected_tuple = vec![TokenValue::Float(-50.0), TokenValue::Float(50.0)];
        assert_eq!(
            actual,
            Ok((
                "",
                SongChartCmds::BulletLerper(vec![
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
