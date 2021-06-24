use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

use ggez::Context;

use crate::chart;
use crate::time::Beats;

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
                    match chart::parse_midi_to_beats(ctx, path, map.bpm) {
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
            } else {
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
