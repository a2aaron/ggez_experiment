#![feature(trait_alias)]

extern crate ggez;
extern crate rand;

use std::env;
use std::path::PathBuf;

use ggez::audio::{SoundSource, Source};

use ggez::event::{KeyCode, KeyMods};
use ggez::graphics::mint::Point2;
use ggez::graphics::{
    Color, DrawMode, DrawParam, Drawable, Font, Mesh, Rect, Scale, Text, TextFragment,
};
use ggez::{audio, conf, event, graphics, Context, ContextBuilder, GameResult};

use time::Time;

mod color;
mod ease;
mod time;

const BPM: f64 = 120.0; // 170.0;
                        // Files read via ggez (usually music/font/images)
const MUSIC_PATH: &str = "/metronome120.ogg"; // "/bbkkbkk.ogg";
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
    background: Color,
}

impl World {
    fn update(&mut self, ctx: &mut Context) {}

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        Ok(())
    }

    fn reset(&mut self) {}
}

impl Default for World {
    fn default() -> Self {
        World {
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
    world: World,
    time: Time,
    music: Source,
    assets: Assets,
    started: bool,
}

impl MainState {
    fn new(ctx: &mut Context) -> GameResult<MainState> {
        let s = MainState {
            world: Default::default(),
            music: audio::Source::new(ctx, MUSIC_PATH)?,
            time: Time::new(BPM, time::Seconds(0.0)),
            started: false,
            assets: Assets::new(ctx),
        };
        Ok(s)
    }

    /// Draw debug text at the bottom of the screen showing the time in the song, in beats.
    fn draw_debug_time(&mut self, ctx: &mut Context) -> GameResult<()> {
        let beat_time = self.time.get_beats();
        // let text = &format!(
        //     "Measure: {:2?}, Beat: {:2?}, Offset: {:3?}",
        //     beat_time.beat / 4,
        //     beat_time.beat % 4,
        //     beat_time.offset
        // )[..];
        let text = format!("Beat: {:.2?}", beat_time.0);
        let fragment = TextFragment {
            text: text.to_string(),
            color: Some(color::DEBUG_RED),
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

        ggez::graphics::window(ctx).set_title(&format!("{}", ggez::timer::fps(ctx)));

        ggez::timer::yield_now();
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
            } else {
                // Start the game. Also play the music.
                self.started = true;
                self.world.reset();
                self.time.reset();
                drop(self.music.play());
            }
        }
    }

    fn key_up_event(&mut self, _ctx: &mut Context, keycode: KeyCode, _keymod: KeyMods) {}

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        let beat_number = self.time.get_beats().0 as i32 % 4;
        let beat_percent = self.time.get_beat_percentage() as f32;
        graphics::clear(ctx, ggez::graphics::BLACK);
        let color = crate::ease::color_lerp(crate::color::RED, ggez::graphics::WHITE, beat_percent);
        let square =
            Mesh::new_rectangle(ctx, DrawMode::fill(), Rect::new(0.0, 0.0, 1.0, 1.0), color)
                .unwrap();
        let dest = match beat_number {
            0 => [100.0, 100.0],
            1 => [100.0, 200.0],
            2 => [200.0, 200.0],
            _ => [200.0, 100.0],
        };

        square.draw(ctx, DrawParam::default().dest(dest).scale([100.0, 100.0]))?;
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
                .samples(ggez::conf::NumSamples::Eight)
                .vsync(true),
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
