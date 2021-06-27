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

use anyhow::bail;

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
                Ok((_remaining, cmd)) => {
                    if let Err(warning) = map.execute_cmd(ctx, cmd) {
                        println!("Warning on line {} \"{}\": {}", line_num, raw_line, warning);
                    }
                }
            }
        }
        Ok(map)
    }

    fn execute_cmd(&mut self, ctx: &mut Context, cmd: SongChartCmds) -> anyhow::Result<()> {
        match cmd {
            SongChartCmds::Skip(skip) => self.skip_amount = Beats(skip),
            SongChartCmds::Bpm(bpm) => self.bpm = bpm,
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
                let pos = WorldPos::from(pos);
                if self.positions.insert(name.clone(), pos).is_some() {
                    bail!("Replaced position {}.", name);
                }
            }
            SongChartCmds::BulletLerper(kwargs) => {
                let kwargs = KwargList::new(&kwargs, &["start", "freq", "lerpstart", "lerpend"])?;
                let start = kwargs.get("start");
                let freq = kwargs.get("freq");
                let lerp_start = kwargs.get("lerpstart");
                let lerp_end = kwargs.get("lerpend");

                let start = start.parse::<f64>()?;
                let freq = freq.parse::<f64>()?;
                let lerp_start = PositionData::parse_lookup(&lerp_start, &self.positions)?;
                let lerp_end = PositionData::parse_lookup(&lerp_end, &self.positions)?;
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

    pub fn lookup_position(&self, name: &str) -> Option<&WorldPos> {
        if !self.positions.contains_key(name) {
            println!("Warning: Position {} does not exist", name);
        }
        self.positions.get(name)
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

struct LineInfo {
    raw_line: String,
    line_num: usize,
    kwargs: KwargList,
}

enum SongMapWarning {
    ReplacedMidibeat(String),
    MidibeatParseError(Box<dyn std::error::Error>),
    ReplacedPosition(String),
    KwargError(KwargError),
    ParseError(),
}

impl std::fmt::Display for SongMapWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::result::Result<(), std::fmt::Error> {
        match self {
            SongMapWarning::ReplacedMidibeat(old) => {
                write!(f, "Replaced midibeat {}.", old)
            }
            SongMapWarning::MidibeatParseError(err) => {
                write!(f, "Error parsing midi. Reason: {}.", err)
            }
            SongMapWarning::ReplacedPosition(old) => {
                write!(f, "Replaced position {}.", old)
            }
            SongMapWarning::KwargError(err) => write!(f, "Kwarg Error: {}", err),
            SongMapWarning::ParseError() => write!(f, "Parse Error: {}", 2),
        }
    }
}

struct KwargList {
    kwargs: HashMap<String, String>,
}

impl KwargList {
    fn new(kwargs: &[(String, String)], required: &[&str]) -> Result<KwargList, KwargError> {
        let mut hash_map = HashMap::new();
        for kwarg in kwargs.iter().cloned() {
            if let Some(old) = hash_map.insert(kwarg.0.clone(), kwarg.1.clone()) {
                return Err(KwargError::DuplicateKwarg(kwarg.0, kwarg.1, old));
            }
        }

        for &required_kwarg in required {
            if !hash_map.contains_key(required_kwarg) {
                return Err(KwargError::MissingKwarg(required_kwarg.to_owned()));
            }
        }
        Ok(KwargList { kwargs: hash_map })
    }

    fn get(&self, kwarg: &str) -> String {
        self.kwargs[kwarg].clone()
    }
}

#[derive(Debug)]
enum KwargError {
    MissingKwarg(String),
    DuplicateKwarg(String, String, String),
}

impl std::fmt::Display for KwargError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KwargError::MissingKwarg(kwarg) => write!(f, "Missing required kwarg: {}", kwarg),
            KwargError::DuplicateKwarg(key, val1, val2) => {
                write!(f, "Duplicate kwargs: {}={} vs {}={}", key, val1, key, val2)
            }
        }
    }
}

impl std::error::Error for KwargError {}

pub enum SongChartCmds {
    Skip(f64),
    Bpm(f64),
    MidiBeat(String, String),
    Position(String, (f64, f64)),
    BulletLerper(Vec<(String, String)>), // timing: BeatSplitter,
                                         // lerp_start: PositionData,
                                         // lerp_end: PositionData
}

pub enum TimingData {
    BeatSplitter {
        start: f64,
        frequency: f64,
        duration: Option<f64>,
        offset: Option<f64>,
        delay: Option<f64>,
    },
    // MidiBeat(String),
}

impl TimingData {
    fn to_beat_splitter(&self) -> Option<BeatSplitter> {
        match *self {
            TimingData::BeatSplitter {
                start,
                frequency,
                duration,
                offset,
                delay,
            } => Some(BeatSplitter {
                start,
                frequency,
                duration: duration.unwrap_or(4.0 * 4.0),
                offset: offset.unwrap_or(0.0),
                delay: delay.unwrap_or(0.0),
            }),
        }
    }
}

pub enum PositionData {
    Literal(WorldPos),
    Variable(String),
}

impl PositionData {
    fn parse_lookup(input: &str, vars: &HashMap<String, WorldPos>) -> anyhow::Result<WorldPos> {
        match PositionData::parse(input) {
            Ok((_, pos)) => pos.lookup(vars),
            Err(err) => bail!(
                "Couldn't parse \"{}\" as position data, reason: {}",
                input,
                err
            ),
        }
    }

    fn parse(input: &str) -> IResult<&str, PositionData> {
        alt((
            map(literal_tuple, |pos| {
                PositionData::Literal(WorldPos::from(pos))
            }),
            map(string, PositionData::Variable),
        ))(input)
    }

    fn lookup(&self, vars: &HashMap<String, WorldPos>) -> anyhow::Result<WorldPos> {
        match self {
            PositionData::Literal(pos) => Ok(*pos),
            PositionData::Variable(name) => {
                let pos = vars.get(name).cloned();
                if let Some(pos) = pos {
                    Ok(pos)
                } else {
                    bail!("Couldn't find position {}", name);
                }
            }
        }
    }
}

fn tag_ws0<'i>(the_tag: &'static str) -> impl FnMut(&'i str) -> IResult<&'i str, &'i str> {
    terminated(tag(the_tag), space0)
}

fn tag_ws1<'i>(the_tag: &'static str) -> impl FnMut(&'i str) -> IResult<&'i str, &'i str> {
    terminated(tag(the_tag), space1)
}

fn literal_tuple(input: &str) -> IResult<&str, (f64, f64)> {
    delimited(
        tag_ws0("("),
        separated_pair(double, tag_ws0(","), double),
        tag_ws0(")"),
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

// pub fn kwarg<'i, O3, E: ParseError<&'i str>, H>(
//     mut second: H,
// ) -> impl FnMut(&'i str) -> IResult<&'i str, (String, O3), E>
// where
//     H: Parser<&'i str, O3, E>,
// {
//     separated_pair(string, char('='), second)
// }

fn kwarg(input: &str) -> IResult<&str, (String, String)> {
    separated_pair(string, char('='), string)(input)
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
//                 let start = match kwargs.get("start") {
//                     Some(input) => double(input.as_str()),
//                     None => Err((input, nom::error::ErrorKind::)),
//                 };
//                 let timing = BeatSplitter {
//                     start: start?.1,
//                     frequency: 1.0,
//                     duration: 4.0 * 4.0,
//                     ..Default::default()
//                 };
// SongChartCmds::BulletLerper {
//     timing,
//     lerp_start: (),
//     lerp_end: (),
// }
//             }),

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
