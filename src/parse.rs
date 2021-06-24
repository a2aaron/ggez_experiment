use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use ggez::Context;
use midly::Smf;

use crate::time::Beats;
use crate::{chart, time};

pub struct SongMap {
    pub skip_amount: Beats,
    pub bpm: f64,
    pub midi_beats: HashMap<String, Vec<Beats>>,
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
        for (line_num, line) in tokenize(&buf).iter().enumerate() {
            use Token::*;
            match &line[..] {
                [String(x), Float(skip)] if x == "SKIP" => map.skip_amount = Beats(*skip),
                [String(x), Float(bpm)] if x == "BPM" => map.bpm = *bpm,
                [String(x), String(name), String(path)] if x == "midibeat" => {
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
                _ => println!("{}: Warning: Unrecognized line: {:?}", line_num, line),
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
}

impl Default for SongMap {
    fn default() -> Self {
        SongMap {
            skip_amount: Beats(0.0),
            bpm: 150.0,
            midi_beats: HashMap::new(),
        }
    }
}

pub fn tokenize(file: &str) -> Vec<Vec<Token>> {
    file.lines().map(|line| tokenize_line(line)).collect()
}

fn tokenize_line(line: &str) -> Vec<Token> {
    let mut tokens = vec![];
    let mut token = String::new();
    let mut in_string = false;
    for character in line.chars() {
        if character == '"' {
            if in_string {
                tokens.push(Token::String(token.clone()));
                token.clear();
            }
            in_string = !in_string
        } else if character == ' ' && !in_string {
            if let Ok(num) = token.parse::<f64>() {
                tokens.push(Token::Float(num));
            } else if !token.is_empty() {
                tokens.push(Token::new(&token));
            }
            token.clear();
        } else {
            token.push(character);
        }
    }

    if !token.is_empty() {
        tokens.push(Token::new(&token))
    }

    tokens
}

#[derive(Debug, Clone)]
pub enum Token {
    String(String),
    Float(f64),
}

impl Token {
    fn new(string: &str) -> Token {
        if let Ok(num) = string.parse::<f64>() {
            Token::Float(num)
        } else {
            Token::String(string.to_owned())
        }
    }
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
