use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use ggez::Context;
use midly::Smf;

use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{alphanumeric1, none_of, space1};
use nom::combinator::map;
use nom::error::ParseError;
use nom::multi::many0;
use nom::number::complete::double;
use nom::sequence::{delimited, separated_pair, tuple};
use nom::{Finish, IResult, Parser};

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
                    SongChartCmds::BulletLerper {
                        timing,
                        lerp_start,
                        lerp_end,
                    } => {
                        let lerp_start = lerp_start.lookup(&map.positions);
                        let lerp_end = lerp_end.lookup(&map.positions);
                        if let (Some(lerp_start), Some(lerp_end)) = (lerp_start, lerp_end) {
                            let cmd_batch = CmdBatch::Bullet {
                                start: CmdBatchPos::Lerped(lerp_start.tuple(), lerp_end.tuple()),
                                end: CmdBatchPos::Lerped((0.0, 0.0), (0.0, 0.0)),
                            };
                            let splitter = timing.to_beat_splitter().unwrap();
                            map.cmd_batches.push((splitter, cmd_batch))
                        } else {
                            println!(
                                "{}: Could not find positions for lerp_start: {:?}, lerp_end {:?}",
                                line_num, lerp_start, lerp_end
                            );
                            continue;
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

pub enum SongChartCmds {
    Skip(f64),
    Bpm(f64),
    MidiBeat(String, String),
    Position(String, (f64, f64)),
    BulletLerper {
        timing: TimingData,
        lerp_start: PositionData,
        lerp_end: PositionData,
    },
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
    fn lookup(&self, vars: &HashMap<String, WorldPos>) -> Option<WorldPos> {
        match self {
            PositionData::Literal(pos) => Some(*pos),
            PositionData::Variable(name) => vars.get(name).cloned(),
        }
    }
}

/// Matches an object from the first parser and discards it,
/// then gets an object from the second parser and discards it,
/// then gets an object from the third parser.
pub fn double_preceded<I, O1, O2, O3, E: ParseError<I>, F, G, H>(
    mut first: F,
    mut second: G,
    mut third: H,
) -> impl FnMut(I) -> IResult<I, O3, E>
where
    F: Parser<I, O1, E>,
    G: Parser<I, O2, E>,
    H: Parser<I, O3, E>,
{
    move |input: I| {
        let (input, _) = first.parse(input)?;
        let (input, _) = second.parse(input)?;
        third.parse(input)
    }
}

pub fn parse(input: &str) -> IResult<&str, SongChartCmds> {
    fn literal_tuple(input: &str) -> IResult<&str, (f64, f64)> {
        delimited(tag("("), separated_pair(double, tag(","), double), tag(")"))(input)
    }

    fn literal_string(input: &str) -> IResult<&str, String> {
        let the_string = many0(none_of("\"")).map(|chars| chars.into_iter().collect::<String>());
        delimited(tag("\""), the_string, tag("\""))(input)
    }

    fn string(input: &str) -> IResult<&str, String> {
        let normal_string = alphanumeric1.map(|chars: &str| chars.to_owned());
        alt((literal_string, normal_string))(input)
    }

    let mut parser = alt((
        double_preceded(tag("SKIP"), space1, map(double, SongChartCmds::Skip)),
        double_preceded(tag("BPM"), space1, map(double, SongChartCmds::Bpm)),
        double_preceded(
            tag("midibeat"),
            space1,
            map(separated_pair(string, space1, string), |(name, path)| {
                SongChartCmds::MidiBeat(name, path)
            }),
        ),
    ));
    parser(input)
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
