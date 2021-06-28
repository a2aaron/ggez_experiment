use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use ggez::Context;
use midly::Smf;

use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{char, none_of, space0, space1};
use nom::combinator::{complete, map, recognize};
use nom::multi::{many0, many1, separated_list0};
use nom::number::complete::double;
use nom::sequence::{delimited, preceded, separated_pair, terminated};
use nom::{Finish, IResult, Parser};

use anyhow::{anyhow, bail};
use derive_more::Display;

use crate::chart::{BeatSplitter, CmdBatch, CmdBatchPos};
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
        for (line_num, (line, raw_line)) in buf
            .lines()
            .map(|raw_line| (parse(raw_line), raw_line))
            .enumerate()
        {
            match line.finish() {
                Err(err) => println!(
                    "{}: Warning: Couldn't parse line \"{}\". Reason: {:?}",
                    line_num, raw_line, err
                ),
                Ok((remaining, cmd)) => {
                    if let Err(warning) = map.execute_cmd(ctx, &cmd) {
                        println!(
                            "Warning on line {} \"{}\": {}. \n\tParsed as: {:?}, {:?}",
                            line_num, raw_line, warning, remaining, cmd
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
                let kwargs = KwargList::new(
                    &kwargs,
                    &[
                        ("start", &[KwargType::Float]),
                        ("freq", &[KwargType::Float]),
                        ("lerpstart", &[KwargType::String, KwargType::Tuple]),
                        ("lerpend", &[KwargType::String, KwargType::Tuple]),
                    ],
                )?;
                let start = kwargs.get_float("start").unwrap();
                let freq = kwargs.get_float("freq").unwrap();
                let lerp_start = kwargs.get("lerpstart").unwrap();
                let lerp_end = kwargs.get("lerpend").unwrap();

                let lerp_start = self.lookup_position(lerp_start)?;
                let lerp_end = self.lookup_position(lerp_end)?;
                let cmd_batch = CmdBatch::Bullet {
                    start: CmdBatchPos::Lerped(lerp_start.tuple(), lerp_end.tuple()),
                    end: CmdBatchPos::Lerped((0.0, 0.0), (0.0, 0.0)),
                };
                let splitter = BeatSplitter {
                    start,
                    frequency: freq,
                    ..Default::default()
                };
                // let splitter = timing.to_beat_splitter().unwrap();
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

    fn lookup_position(&self, input: KwargValue) -> anyhow::Result<WorldPos> {
        match input {
            KwargValue::String(name) => self
                .positions
                .get(&name)
                .cloned()
                .ok_or_else(|| anyhow!("Couldn't find position {}", name)),
            KwargValue::Float(_) => {
                bail!("Wrong type for PositionData. Expected String or Tuple. Got Float.")
            }
            KwargValue::Tuple(pos) => Ok(WorldPos::from(pos)),
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

struct KwargList {
    kwargs: HashMap<String, KwargValue>,
}

impl KwargList {
    fn new(
        kwargs: &[(String, KwargValue)],
        required: &[(&str, &[KwargType])],
    ) -> Result<KwargList, KwargError> {
        let mut hash_map = HashMap::new();
        for kwarg in kwargs.iter().cloned() {
            if let Some(old) = hash_map.insert(kwarg.0.clone(), kwarg.1.clone()) {
                return Err(KwargError::DuplicateKwarg(kwarg.0, kwarg.1, old));
            }
        }

        for &(required_kwarg, allowed_types) in required {
            match hash_map.get(required_kwarg) {
                None => {
                    return Err(KwargError::MissingKwarg(required_kwarg.to_owned()));
                }
                Some(kwarg_val) => {
                    if !allowed_types.iter().any(|ty| *ty == kwarg_val.get_type()) {
                        return Err(KwargError::TypeMismatch(
                            required_kwarg.to_owned(),
                            kwarg_val.clone(),
                            allowed_types.to_vec(),
                        ));
                    }
                }
            }
        }
        Ok(KwargList { kwargs: hash_map })
    }

    fn get(&self, kwarg: &str) -> Option<KwargValue> {
        self.kwargs.get(kwarg).cloned()
    }

    fn get_float(&self, kwarg: &str) -> Option<f64> {
        match self.get(kwarg) {
            Some(KwargValue::Float(x)) => Some(x),
            _ => None,
        }
    }
}

#[derive(Debug)]
enum KwargError {
    MissingKwarg(String),
    TypeMismatch(String, KwargValue, Vec<KwargType>),
    DuplicateKwarg(String, KwargValue, KwargValue),
}

impl std::fmt::Display for KwargError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KwargError::MissingKwarg(kwarg) => write!(f, "Missing required kwarg: {}", kwarg),
            KwargError::DuplicateKwarg(key, val1, val2) => {
                write!(f, "Duplicate kwargs: {}={} vs {}={}", key, val1, key, val2)
            }
            KwargError::TypeMismatch(key, val, expected) => write!(
                f,
                "Wrong kwarg type for {}={}, got {}, expected one of {:?}",
                key,
                val,
                val.get_type(),
                expected
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
    BulletLerper(Vec<(String, KwargValue)>),
}

#[derive(Display, Debug, Clone, PartialEq)]
pub enum KwargValue {
    String(String),
    Float(f64),
    #[display(fmt = "({}, {})", "_0.0", "_0.1")]
    Tuple((f64, f64)),
}

impl KwargValue {
    fn get_type(&self) -> KwargType {
        match self {
            KwargValue::String(_) => KwargType::String,
            KwargValue::Float(_) => KwargType::Float,
            KwargValue::Tuple(_) => KwargType::Tuple,
        }
    }
}

#[derive(Display, Debug, Copy, Clone, PartialEq, Eq)]
pub enum KwargType {
    String,
    Float,
    Tuple,
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

fn literal_tuple(input: &str) -> IResult<&str, (f64, f64)> {
    delimited(
        tag_ws0("("),
        separated_pair(double, ws0_tag_ws0(","), double),
        ws0_tag(")"),
    )(input)
}

fn literal_string(input: &str) -> IResult<&str, String> {
    let the_string = recognize(many0(none_of("\""))).map(String::from);
    delimited(char('"'), the_string, char('"'))(input)
}

fn string(input: &str) -> IResult<&str, String> {
    let normal_string = many1(none_of(" =")).map(|chars| chars.into_iter().collect::<String>());
    alt((literal_string, normal_string))(input)
}

fn kwarg(input: &str) -> IResult<&str, (String, KwargValue)> {
    separated_pair(
        string,
        char('='),
        alt((
            map(literal_tuple, KwargValue::Tuple),
            map(double, KwargValue::Float),
            map(string, KwargValue::String),
        )),
    )(input)
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
            map(
                separated_pair(string, space1, literal_tuple),
                |(name, pos)| SongChartCmds::Position(name, pos),
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
    use crate::parse::{kwarg, literal_tuple, parse, string, KwargValue, SongChartCmds};

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
    fn test_parse_tuple_literal() {
        let input = "(1.0,2.0)";
        let actual = literal_tuple(input);
        assert_eq!(actual, Ok(("", (1.0, 2.0))));

        let input = "(1.0, 2.0)";
        let actual = literal_tuple(input);
        assert_eq!(actual, Ok(("", (1.0, 2.0))));

        let input = "(   1.0 \t  , \t  2.0  )";
        let actual = literal_tuple(input);
        assert_eq!(actual, Ok(("", (1.0, 2.0))));

        let input = "(-5.0,-0.0)";
        let actual = literal_tuple(input);
        assert_eq!(actual, Ok(("", (-5.0, 0.0))));
    }

    #[test]
    fn test_parse_kwarg() {
        let input = "value=string";
        let actual = kwarg(input);
        assert_eq!(
            actual,
            Ok((
                "",
                ("value".to_owned(), KwargValue::String("string".to_owned()))
            ))
        );

        let input = "value=5.0";
        let actual = kwarg(input);
        assert_eq!(
            actual,
            Ok(("", ("value".to_owned(), KwargValue::Float(5.0))))
        );

        let input = "value=(5.0, 4.0)";
        let actual = kwarg(input);
        assert_eq!(
            actual,
            Ok(("", ("value".to_owned(), KwargValue::Tuple((5.0, 4.0)))))
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
                    ("start".to_owned(), KwargValue::Float(16.0)),
                    ("freq".to_owned(), KwargValue::Float(4.0),),
                    (
                        "lerpstart".to_owned(),
                        KwargValue::String("botleft".to_owned()),
                    ),
                    (
                        "lerpend".to_owned(),
                        KwargValue::String("botright".to_owned()),
                    )
                ])
            ))
        );
    }

    #[test]
    fn test_parse_bulletlerp2() {
        let input = "bulletlerp start=16.0 freq=4.0 lerpstart=(-50.0,50.0) lerpend=botright";
        let actual = parse(input);
        assert_eq!(
            actual,
            Ok((
                "",
                SongChartCmds::BulletLerper(vec![
                    ("start".to_owned(), KwargValue::Float(16.0)),
                    ("freq".to_owned(), KwargValue::Float(4.0),),
                    ("lerpstart".to_owned(), KwargValue::Tuple((-50.0, 50.0)),),
                    (
                        "lerpend".to_owned(),
                        KwargValue::String("botright".to_owned()),
                    )
                ])
            ))
        );
    }
}
