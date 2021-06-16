#![feature(trait_alias)]

use std::env;
use std::path::PathBuf;

use ggez::audio::{SoundSource, Source};
use ggez::GameError;

use ggez::event::{KeyCode, KeyMods};
use ggez::graphics::mint::Point2;
use ggez::graphics::{
    Color, DrawMode, DrawParam, Drawable, Font, Mesh, Rect, Scale, Text, TextFragment,
};
use ggez::{
    audio, conf, event, graphics, nalgebra as na, timer, Context, ContextBuilder, GameResult,
};

use keyboard::KeyboardState;
use player::{Player, WorldPos};
use time::Time;

mod color;
mod ease;
mod keyboard;
mod player;
mod time;
mod util;

const TARGET_FPS: u32 = 60;

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
    time: Time,
    music: Source,
    keyboard: KeyboardState,
    assets: Assets,
    started: bool,
    player: Player,
}

impl MainState {
    fn new(ctx: &mut Context) -> GameResult<MainState> {
        let s = MainState {
            keyboard: KeyboardState::default(),
            music: audio::Source::new(ctx, MUSIC_PATH)?,
            time: Time::new(BPM, time::Seconds(0.0)),
            started: false,
            assets: Assets::new(ctx),
            player: Player::new(),
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
        let delta = ggez::timer::delta(ctx);
        let text = format!(
            "Beat: {:.2?}\nPlayer position: {:.2?} ({:.2?}, {:.2?})\nDelta: {:.2?}",
            beat_time.0,
            self.player.pos,
            self.player.pos.as_screen_coords().x,
            self.player.pos.as_screen_coords().y,
            delta
        );
        let fragment = TextFragment {
            text,
            color: Some(color::DEBUG_RED),
            font: Some(self.assets.debug_font),
            scale: Some(Scale::uniform(18.0)),
        };
        let text = Text::new(fragment);
        let text_height = text.height(ctx) as f32;
        let screen = graphics::screen_coordinates(ctx);
        text.draw(
            ctx,
            DrawParam::default().dest(Point2 {
                x: screen.x,
                y: screen.y + screen.h - text_height,
            }),
        )?;
        Ok(())
    }

    fn draw_debug_world_lines(&self, ctx: &mut Context) -> Result<(), GameError> {
        let origin = WorldPos::origin().as_screen_coords();
        Mesh::new_line(
            ctx,
            &[
                (origin + na::Vector2::new(-5.0, 0.0)),
                (origin + na::Vector2::new(5.0, 0.0)),
            ],
            2.0,
            crate::color::DEBUG_RED,
        )?
        .draw(ctx, DrawParam::default())?;
        Mesh::new_line(
            ctx,
            &[
                (origin + na::Vector2::new(0.0, 5.0)),
                (origin + na::Vector2::new(0.0, -5.0)),
            ],
            2.0,
            crate::color::DEBUG_RED,
        )?
        .draw(ctx, DrawParam::default())?;

        let rect = WorldPos::as_screen_rect(WorldPos::origin(), 100.0, 100.0);
        Mesh::new_rectangle(ctx, DrawMode::stroke(2.0), rect, crate::color::DEBUG_RED)?
            .draw(ctx, DrawParam::default())?;

        let rect = WorldPos::as_screen_rect(WorldPos::origin(), 10.0, 10.0);
        Mesh::new_rectangle(ctx, DrawMode::stroke(2.0), rect, crate::color::DEBUG_RED)?
            .draw(ctx, DrawParam::default())?;

        Ok(())
    }
}

impl event::EventHandler for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        // Lock the framerate at 60 FPS
        while timer::check_update_time(ctx, TARGET_FPS) {
            if !self.started {
                return Ok(());
            }
            let physics_delta_time = 1.0 / f64::from(TARGET_FPS);

            self.time.update();

            self.player.update(physics_delta_time, &self.keyboard);
            ggez::graphics::window(ctx).set_title(&format!("{}", ggez::timer::fps(ctx)));
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
        if keycode == KeyCode::P {
            if self.started {
                // Stop the game, pausing the music, fetching a new Source instance, and
                // rebuild the scheduler work queue.
                self.started = false;
                self.music.stop();
                self.music = audio::Source::new(ctx, MUSIC_PATH).unwrap();
                // self.world.reset();
            } else {
                // Start the game. Also play the music.
                self.started = true;
                // self.world.reset();
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
        graphics::clear(ctx, ggez::graphics::BLACK);
        // ggez::graphics::set_screen_coordinates(ctx, Rect::new(-320.0, 240.0, 640.0, -480.0))?;

        let beat_number = self.time.get_beats().0 as i32 % 4;
        let beat_percent = self.time.get_beat_percentage() as f32;
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

        let player_mesh = self.player.get_mesh(ctx)?;
        player_mesh.draw(
            ctx,
            DrawParam::default().dest(self.player.pos.as_screen_coords()),
        )?;

        self.draw_debug_time(ctx)?;
        self.draw_debug_world_lines(ctx)?;

        graphics::present(ctx)?;
        ggez::timer::yield_now();
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
