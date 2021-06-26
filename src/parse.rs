use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use ggez::Context;
use midly::Smf;

use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{alphanumeric1, char, none_of, space0, space1};
use nom::combinator::{complete, map, map_opt, map_res, recognize};
use nom::error::ParseError;
use nom::multi::{many0, many1, separated_list0};
use nom::number::complete::double;
use nom::sequence::{delimited, preceded, separated_pair, terminated, tuple};
use nom::{Compare, Finish, IResult, InputTake, Parser};

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
                Ok((_remaining, cmd)) => match cmd {
                    SongChartCmds::Skip(skip) => map.skip_amount = Beats(skip),
                    SongChartCmds::Bpm(bpm) => map.bpm = bpm,
                    SongChartCmds::MidiBeat(name, path) => {
                        match parse_midi_to_beats(ctx, &path, map.bpm) {
                            Ok(beats) => {
                                if map.midi_beats.insert(name.clone(), beats).is_some() {
                                    println!(
                                        "{}: Warning: Replaced {:?} with new midibeat",
                                        line_num, name
                                    )
                                }
                            }
                            Err(err) => {
                                println!(
                                    "{}: Warning: Couldn't parse midi file {:?}. Reason: {:?}",
                                    line_num, path, err
                                );
                            }
                        }
                    }
                    SongChartCmds::Position(name, pos) => {
                        let pos = WorldPos::from(pos);
                        if map.positions.insert(name.clone(), pos).is_some() {
                            println!(
                                "{}: Warning: Replaced {:?} with new position",
                                line_num, name
                            );
                        }
                    }
                    SongChartCmds::BulletLerper(kwargs) => {
                        match KwargList::new(&kwargs, &["start", "freq", "lerpstart", "lerpend"]) {
                            Err(err) => println!(
                                "{}: Warning: Kwarg error, {}. Kwargs {:?} Reason: {}",
                                line_num, raw_line, kwargs, err
                            ),
                            Ok(kwargs) => {
                                let start = kwargs.get("start");
                                let freq = kwargs.get("freq");
                                let lerp_start = kwargs.get("lerpstart");
                                let lerp_end = kwargs.get("lerpend");

                                let start = start.parse::<f64>();
                                let freq = freq.parse::<f64>();
                                let lerp_start =
                                    PositionData::parse_lookup(&lerp_start, &map.positions);
                                let lerp_end =
                                    PositionData::parse_lookup(&lerp_end, &map.positions);
                                if let (Ok(start), Ok(freq), Some(lerp_start), Some(lerp_end)) =
                                    (start, freq, lerp_start, lerp_end)
                                {
                                    let cmd_batch = CmdBatch::Bullet {
                                        start: CmdBatchPos::Lerped(
                                            lerp_start.tuple(),
                                            lerp_end.tuple(),
                                        ),
                                        end: CmdBatchPos::Lerped((0.0, 0.0), (0.0, 0.0)),
                                    };
                                    let splitter = BeatSplitter {
                                        start,
                                        frequency: freq,
                                        ..Default::default()
                                    };
                                    // let splitter = timing.to_beat_splitter().unwrap();
                                    map.cmd_batches.push((splitter, cmd_batch))
                                } else {
                                    println!(
                                    "{}: Could not find positions for lerp_start: {:?}, lerp_end {:?}",
                                    line_num, lerp_start, lerp_end
                                );
                                    continue;
                                }
                            }
                        }
                    }
                },
            }
        }
        Ok(map)
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
    fn parse_lookup(input: &str, vars: &HashMap<String, WorldPos>) -> Option<WorldPos> {
        match PositionData::parse(input) {
            Ok((_, pos)) => pos.lookup(vars),
            Err(err) => {
                println!("Warning: Couldn't parse position data: {}", err);
                None
            }
        }
    }

    fn parse(input: &str) -> IResult<&str, PositionData> {
        alt((
            map(string, PositionData::Variable),
            map(literal_tuple, |pos| {
                PositionData::Literal(WorldPos::from(pos))
            }),
        ))(input)
    }

    fn lookup(&self, vars: &HashMap<String, WorldPos>) -> Option<WorldPos> {
        match self {
            PositionData::Literal(pos) => Some(*pos),
            PositionData::Variable(name) => {
                let pos = vars.get(name).cloned();
                if pos.is_none() {
                    println!("Warning: Couldn't lookup position {}", name);
                }
                pos
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
