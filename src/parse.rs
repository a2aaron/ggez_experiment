use std::io::Read;
use std::path::Path;

use ggez::Context;

use crate::time::Beats;

pub struct SongMap {
    pub skip_amount: Beats,
}

impl SongMap {
    pub fn parse_file<P: AsRef<Path>>(
        ctx: &mut Context,
        path: P,
    ) -> Result<SongMap, Box<dyn std::error::Error>> {
        let mut file = ggez::filesystem::open(ctx, path)?;
        let mut buf = String::new();
        file.read_to_string(&mut buf);
        let mut map = SongMap::default();
        for line in buf.lines() {
            let tokens = line.split_whitespace().collect::<Vec<&str>>();
            if tokens
                .get(0)
                .ok_or("Zero index missing")?
                .starts_with("SKIP")
            {
                let skip_amount = tokens.get(1).ok_or("One index missing")?.parse::<f64>()?;
                map.skip_amount = Beats(skip_amount);
            }
        }
        Ok(map)
    }
}

impl Default for SongMap {
    fn default() -> Self {
        SongMap {
            skip_amount: Beats(0.0),
        }
    }
}

pub enum ParseError {}
