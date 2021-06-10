extern crate ggez;
extern crate rand;

mod enemy;
mod grid;
mod keyboard;
mod parse;
mod player;
mod time;
mod util;

use std::env;
use std::path::{Path, PathBuf};

use ggez::audio::{SoundSource, Source};

use ggez::event::{KeyCode, KeyMods};
use ggez::graphics::mint::Point2;
use ggez::graphics::{Color, DrawParam, Drawable, Font, Scale, Text, TextFragment};
use ggez::{audio, conf, event, graphics, Context, ContextBuilder, GameResult};

use enemy::Enemy;
use grid::Grid;
use keyboard::KeyboardState;
use parse::Scheduler;
use player::Player;
use time::{Beat, BeatF64, Time};
use util::*;

const BPM: f64 = 170.0;
// Files read via ggez (usually music/font/images)
const MUSIC_PATH: &str = "/bbkkbkk.ogg";
// const ARIAL_PATH: &str = "/Arial.ttf";
const FIRACODE_PATH: &str = "/FiraCode-Regular.ttf";
// Files manually read by me (usually maps)
const MAP_PATH: &str = "./resources/bbkkbkk.map";

// Debug
const USE_MAP: bool = true;

pub const WINDOW_WIDTH: f32 = 640.0;
pub const WINDOW_HEIGHT: f32 = 480.0;

/// Contains all the information abou the world and it's game elements
pub struct World {
    player: Player,
    enemies: Vec<Box<dyn Enemy>>,
    grid: Grid,
    background: Color,
}

impl World {
    fn update(&mut self, ctx: &mut Context, beat_time: Beat) {
        let beat_percent = Time::beat_percent(beat_time);
        // Set the background as appropriate
        let color = (rev_quad(beat_percent) / 10.0) as f32;
        self.background = Color::new(color, color, color, 1.0);

        // Update everything
        self.grid.update(beat_percent);
        self.player.update(ctx);

        // Collision check. Also update enemies.
        let mut was_hit = false;
        for enemy in self.enemies.iter_mut() {
            enemy.update(Into::<BeatF64>::into(beat_time));
            if enemy.intersects(&self.player) {
                was_hit = true
            }
        }

        if was_hit {
            self.player.on_hit();
        }

        // Delete all non-alive enemies
        self.enemies.retain(|e| e.is_alive());
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        self.grid.draw(ctx)?;
        self.player.draw(ctx, &self.grid)?;
        for enemy in self.enemies.iter() {
            enemy.draw(ctx, &self.grid);
        }
        Ok(())
    }

    fn reset(&mut self) {
        self.enemies.clear();
    }
}

impl Default for World {
    fn default() -> Self {
        World {
            player: Default::default(),
            enemies: Default::default(),
            grid: Default::default(),
            background: Color::new(0.0, 0.0, 0.0, 1.0),
        }
    }
}

/// Stores assets like fonts, music, sprite images, etc
/// TODO: Add music stuff here.
struct Assets {
    debug_font: Font,
}

impl Assets {
    fn new(ctx: &mut Context) -> Assets {
        Assets {
            debug_font: Font::new(ctx, FIRACODE_PATH).unwrap(),
        }
    }
}

struct MainState {
    scheduler: Scheduler,
    world: World,
    keyboard: KeyboardState,
    time: Time,
    music: Source,
    assets: Assets,
    started: bool,
}

impl MainState {
    fn new(ctx: &mut Context) -> GameResult<MainState> {
        let s = MainState {
            keyboard: Default::default(),
            world: Default::default(),
            music: audio::Source::new(ctx, MUSIC_PATH)?,
            time: Time::new(BPM),
            started: false,
            scheduler: Scheduler::read_file(Path::new(MAP_PATH)),
            assets: Assets::new(ctx),
        };
        Ok(s)
    }

    /// Draw debug text at the bottom of the screen showing the time in the song, in beats.
    fn draw_debug_time(&mut self, ctx: &mut Context) -> GameResult<()> {
        let beat_time = self.time.beat_time();
        let text = &format!(
            "Measure: {:2?}, Beat: {:2?}, Offset: {:3?}",
            beat_time.beat / 4,
            beat_time.beat % 4,
            beat_time.offset
        )[..];
        let fragment = TextFragment {
            text: text.to_string(),
            color: Some(DEBUG_RED),
            font: Some(self.assets.debug_font),
            scale: Some(Scale::uniform(18.0)),
        };
        let text = Text::new(fragment);
        let text_width = text.width(ctx) as f32;
        let text_height = text.height(ctx) as f32;
        let screen = graphics::screen_coordinates(ctx);
        text.draw(
            ctx,
            DrawParam::default().dest(Point2 {
                x: screen.w - text_width,
                y: screen.h - text_height,
            }),
        )?;
        Ok(())
    }
}

impl event::EventHandler for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        if !self.started {
            return Ok(());
        }

        self.time.update();

        if let Ok(direction) = self.keyboard.direction() {
            self.world.player.key_down_event(direction);
        }

        self.world.update(ctx, self.time.beat_time());
        if USE_MAP {
            self.scheduler.update(&self.time, &mut self.world);
        }
        Ok(())
    }

    fn key_down_event(
        &mut self,
        ctx: &mut Context,
        keycode: KeyCode,
        _keymod: KeyMods,
        _repeat: bool,
    ) {
        use KeyCode::*;
        if keycode == P {
            if self.started {
                // Stop the game, pausing the music, fetching a new Source instance, and
                // rebuild the scheduler work queue.
                self.started = false;
                self.music.stop();
                self.music = audio::Source::new(ctx, MUSIC_PATH).unwrap();
                self.world.reset();
                self.scheduler = Scheduler::read_file(Path::new(MAP_PATH))
            } else {
                // Start the game. Also play the music.
                self.started = true;
                self.world.reset();
                self.time.reset();
                drop(self.music.play());
            }
        }

        self.keyboard.update(keycode, true);
    }

    fn key_up_event(&mut self, _ctx: &mut Context, keycode: KeyCode, _keymod: KeyMods) {
        self.keyboard.update(keycode, false);
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::clear(ctx, self.world.background);
        self.world.draw(ctx)?;
        self.draw_debug_time(ctx)?;
        graphics::present(ctx)?;
        Ok(())
    }
}

pub fn main() {
    let mut cb = ContextBuilder::new("visual", "a2aaron")
        .window_setup(
            conf::WindowSetup::default()
                .title("ʀᴛʜᴍ")
                .samples(ggez::conf::NumSamples::Eight),
        )
        .window_mode(conf::WindowMode::default().dimensions(WINDOW_WIDTH, WINDOW_HEIGHT));
    if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
        // Add the resources path so we can use it.
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
    let (mut ctx, mut events_loop) = cb.build().unwrap();
    let mut state = MainState::new(&mut ctx).unwrap();
    ggez::event::run(&mut ctx, &mut events_loop, &mut state).unwrap();
}
