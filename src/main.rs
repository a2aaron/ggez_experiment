#![feature(slice_patterns)]

extern crate ggez;
extern crate rand;

mod enemy;
mod grid;
mod keyboard;
mod player;
mod time;
mod util;

use std::env;
use std::fs::File;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use ggez::audio::Source;
use ggez::event::{Keycode, Mod};
use ggez::graphics::{Color, Text, Font, Point2, Drawable};
use ggez::*;

use enemy::Bullet;
use grid::Grid;
use keyboard::KeyboardState;
use player::Player;
use time::{Beat, Scheduler};
use util::*;

const BPM: f64 = 170.0;
const MUSIC_PATH: &str = "/bbkkbkk.ogg";
const MAP_PATH: &str = "./resources/bbkkbkk.map";
const ARIAL_PATH: &str = "/Arial.ttf";

/// Contains all the information abou the world and it's game elements
pub struct World {
    player: Player,
    enemies: Vec<Bullet>,
    grid: Grid,
    background: Color,
    beat_time: Beat, // Time since start of song
}

impl World {
    fn update(&mut self, ctx: &mut Context, beats_since_start: Beat) {
        self.beat_time = beats_since_start;
        let beat_percent: f64 = Into::<f64>::into(self.beat_time) % 1.0;
        let color = (rev_quad(beat_percent) / 10.0) as f32;
        self.background = Color::new(color, color, color, 1.0);

        self.grid.update(beat_percent);
        self.player.update(ctx);

        let mut was_hit = false;
        for enemy in self.enemies.iter_mut() {
            enemy.update(Into::<f64>::into(self.beat_time));
            if self.player.hit(enemy) {
                was_hit = true
            }
        }

        if was_hit {
            self.player.on_hit();
        }

        self.enemies.retain(|e| e.alive);
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        self.grid.draw(ctx)?;
        self.player.draw(ctx, &self.grid)?;
        for enemy in self.enemies.iter() {
            enemy.draw(ctx, &self.grid)?;
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.enemies.clear();
        self.beat_time = Default::default();
    }
}

impl Default for World {
    fn default() -> Self {
        World {
            player: Default::default(),
            enemies: Default::default(),
            grid: Default::default(),
            background: Color::new(0.0, 0.0, 0.0, 1.0),
            beat_time: Default::default(),
        }
    }
}

struct Assets {
    font: Font,
}

impl Assets {
    fn new(ctx: &mut Context) -> Assets {
        Assets {
            font: Font::new(ctx, ARIAL_PATH, 18).unwrap(),
        }
    }
}

struct MainState {
    scheduler: Scheduler,
    world: World,
    keyboard: KeyboardState,
    start_time: Instant,
    last_time: Instant,
    current_time: Duration,
    bpm: Duration,
    music: Source,
    assets: Assets,
    started: bool,
}

impl MainState {
    fn new(ctx: &mut Context) -> GameResult<MainState> {
        let s = MainState {
            keyboard: Default::default(),
            world: Default::default(),
            start_time: Instant::now(),
            last_time: Instant::now(),
            current_time: Duration::new(0, 0),
            bpm: bpm_to_duration(BPM),
            music: audio::Source::new(ctx, MUSIC_PATH)?,
            started: false,
            scheduler: Scheduler::read_file(File::open(MAP_PATH).unwrap()),
            assets: Assets::new(ctx),
        };
        Ok(s)
    }

    /// Draw debug text at the bottom of the screen showing the time in the song, in beats. 
    fn draw_debug_time(&mut self, ctx: &mut Context) -> GameResult<()> {
        let beat_time = self.world.beat_time;
        let string: &str = &format!("Measure: {:?}, Beat: {:?}, Offset: {:?}", beat_time.beat/4, beat_time.beat % 4, beat_time.offset)[..];
        let text = Text::new(ctx, string, &self.assets.font)?;
        let screen = graphics::get_screen_coordinates(ctx);
        graphics::set_color(ctx, DEBUG_RED)?;
        text.draw(ctx, Point2::new(screen.w - text.width() as f32, screen.h - text.height() as f32), 0.0)?;
        Ok(())
    }
}

impl event::EventHandler for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        if !self.started {
            return Ok(());
        }
        self.current_time += Instant::now().duration_since(self.last_time);
        self.last_time = Instant::now();
        let beats_since_start: Beat =
            (timer::duration_to_f64(self.current_time) / timer::duration_to_f64(self.bpm)).into();

        if let Ok(direction) = self.keyboard.direction() {
            self.world.player.key_down_event(direction);
        }

        self.world.update(ctx, beats_since_start);

        self.scheduler
            .update(beats_since_start, &mut self.world);
        Ok(())
    }

    fn key_down_event(&mut self, ctx: &mut Context, keycode: Keycode, _keymod: Mod, _repeat: bool) {
        use Keycode::*;
        match keycode {
            P => {
                if self.started {
                    // Stop the game, pausing the music, fetching a new Source instance, and
                    // rebuild the scheduler work queue.
                    self.started = false;
                    self.music.stop();
                    drop(self.music = audio::Source::new(ctx, MUSIC_PATH).unwrap());
                    self.world.reset();
                    self.scheduler = Scheduler::read_file(File::open(MAP_PATH).unwrap())
                } else {
                    // Start the game. Also play the music.
                    self.started = true;
                    self.start_time = Instant::now();
                    self.last_time = Instant::now();
                    self.current_time = Duration::new(0,0);
                    drop(self.music.play());
                }
            }
            _ => (),
        }

        self.keyboard.update(keycode, true);
    }

    fn key_up_event(&mut self, _ctx: &mut Context, keycode: Keycode, _keymod: Mod, _repeat: bool) {
        self.keyboard.update(keycode, false);
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::clear(ctx);
        graphics::set_background_color(ctx, self.world.background);
        self.world.draw(ctx)?;
        self.draw_debug_time(ctx)?;
        graphics::present(ctx);
        Ok(())
    }
}

pub fn main() {
    let mut cb = ContextBuilder::new("visual", "ggez")
        .window_setup(conf::WindowSetup::default().title("Rythym"))
        .window_mode(conf::WindowMode::default().dimensions(640, 480));
    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        let mut path = PathBuf::from(manifest_dir);
        path.push("resources");
        println!("Adding path {:?}", path);
        // We need this re-assignment alas, see
        // https://aturon.github.io/ownership/builders.html
        // under "Consuming builders"
        cb = cb.add_resource_path(path);
    } else {
        println!("Not building from cargo");
    }
    let ctx = &mut cb.build().unwrap();
    let state = &mut MainState::new(ctx).unwrap();
    event::run(ctx, state).unwrap();
}
