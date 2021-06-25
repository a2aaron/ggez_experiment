use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use ggez::Context;
use midly::Smf;

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
            .map(|raw_line| (tokenize_line(raw_line), raw_line))
            .enumerate()
        {
            match line {
                Err(err) => println!(
                    "{}: Warning: Couldn't parse line \"{}\". Reason: {:?}",
                    line_num, raw_line, err
                ),
                Ok(line) => {
                    use Token::*;
                    match &line.tokens[..] {
                        [String(x), Float(skip)] if x == "SKIP" => map.skip_amount = Beats(*skip),
                        [String(x), Float(bpm)] if x == "BPM" => map.bpm = *bpm,
                        [MidiBeat, String(name), String(path)] => {
                            match parse_midi_to_beats(ctx, path, map.bpm) {
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
                        [Position, String(name), Float(x), Float(y)] => {
                            let pos = WorldPos::from((*x, *y));
                            if map.positions.insert(name.clone(), pos).is_some() {
                                println!(
                                    "{}: Warning: Replaced {:?} with new position",
                                    line_num, name
                                );
                            }
                        }
                        [BulletLerper] => {
                            let start_time = line.get_kwarg("start");
                            let freq = line.get_kwarg("freq");
                            let lerp_start = line.get_kwarg("lerpstart");
                            let lerp_end = line.get_kwarg("lerpend");

                            if let (
                                Some(Float(start_time)),
                                Some(Float(freq)),
                                Some(String(lerp_start)),
                                Some(String(lerp_end)),
                            ) = (start_time, freq, lerp_start, lerp_end)
                            {
                                let lerp_start = map.lookup_position(lerp_start);
                                let lerp_end = map.lookup_position(lerp_end);
                                if let (Some(lerp_start), Some(lerp_end)) = (lerp_start, lerp_end) {
                                    let cmd_batch = CmdBatch::Bullet {
                                        start: CmdBatchPos::Lerped(
                                            lerp_start.tuple(),
                                            lerp_end.tuple(),
                                        ),
                                        end: CmdBatchPos::Lerped((0.0, 0.0), (0.0, 0.0)),
                                    };
                                    let splitter = BeatSplitter {
                                        start: *start_time,
                                        frequency: *freq,
                                        duration: 4.0 * 4.0,
                                        ..Default::default()
                                    };
                                    map.cmd_batches.push((splitter, cmd_batch))
                                } else {
                                    println!(
                                        "Bad lookup for lerp_start: {:?}, lerp_end {:?}",
                                        lerp_start, lerp_end
                                    );
                                    continue;
                                }
                            } else {
                                println!("Missing kwargs");
                                continue;
                            }
                        }
                        _ => println!(
                            "{}: Warning: Unrecognized line: {:?} parsed: {:?} kwargs: {:?}",
                            line_num, raw_line, line.tokens, line.kwargs
                        ),
                    }
                }
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

pub struct ParsedLine {
    tokens: Vec<Token>,
    kwargs: HashMap<String, Token>,
}

impl ParsedLine {
    fn new() -> Self {
        ParsedLine {
            tokens: vec![],
            kwargs: HashMap::new(),
        }
    }

    fn push(&mut self, token: Token) {
        match token {
            Token::Kwarg(first, second) => {
                if let Some(old) = self.kwargs.insert(first.clone(), *second.clone()) {
                    println!(
                        "Warning: Duplicate kwargs {}={:?} --> {}={:?}",
                        first, old, first, second
                    )
                }
            }
            _ => self.tokens.push(token),
        }
    }

    fn get_kwarg(&self, keyword: &str) -> Option<&Token> {
        if !self.kwargs.contains_key(keyword) {
            println!("Warning: Kwarg {} missing", keyword);
        }
        self.kwargs.get(keyword)
    }
}

fn tokenize_line(line: &str) -> Result<ParsedLine, ParseError> {
    let mut parsed_line = ParsedLine::new();
    let mut token = String::new();
    let mut in_string = false;
    for character in line.chars() {
        if character == '"' {
            if in_string {
                parsed_line.tokens.push(Token::String(token.clone()));
                token.clear();
            }
            in_string = !in_string
        } else if character == ' ' && !in_string {
            parsed_line.push(Token::new(&token)?);
            token.clear();
        } else {
            token.push(character);
        }
    }

    if !token.is_empty() {
        parsed_line.push(Token::new(&token)?)
    }

    Ok(parsed_line)
}

#[derive(Debug, Clone)]
pub enum Token {
    MidiBeat,
    Position,
    BulletLerper,
    Kwarg(String, Box<Token>),
    String(String),
    Float(f64),
}

impl Token {
    fn new(string: &str) -> Result<Token, ParseError> {
        if let Ok(num) = string.parse::<f64>() {
            Ok(Token::Float(num))
        } else if string == "midibeat" {
            Ok(Token::MidiBeat)
        } else if string == "position" {
            Ok(Token::Position)
        } else if string == "bulletlerp" {
            Ok(Token::BulletLerper)
        } else if let Some(index) = string.find('=') {
            let (first, second) = string.split_at(index);
            let second = second.chars().skip(1).collect::<String>();
            if first.is_empty() || second.is_empty() {
                Err(ParseError::BadKwarg(string.to_owned()))
            } else {
                Ok(Token::Kwarg(
                    first.to_owned(),
                    Box::new(Token::new(&second)?),
                ))
            }
        } else {
            Ok(Token::String(string.to_owned()))
        }
    }
}

#[derive(Debug)]
pub enum ParseError {
    BadKwarg(String),
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
