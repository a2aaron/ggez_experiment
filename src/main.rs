#![feature(trait_alias)]

use std::env;
use std::path::PathBuf;

use chart::Scheduler;
use enemy::{Bullet, Enemy, EnemyLifetime};
use ggez::audio::{SoundSource, Source};
use ggez::GameError;

use ggez::event::{KeyCode, KeyMods};
use ggez::graphics::mint::Point2;
use ggez::graphics::{DrawMode, DrawParam, Drawable, Font, Mesh, Scale, Text, TextFragment};
use ggez::{
    audio, conf, event, graphics, nalgebra as na, timer, Context, ContextBuilder, GameResult,
};

use keyboard::KeyboardState;
use player::Player;
use time::Time;
use world::{WorldLen, WorldPos};

use crate::enemy::Laser;
use crate::time::Beats;

mod chart;
mod color;
mod ease;
mod enemy;
mod keyboard;
mod player;
mod time;
mod util;
mod world;

const TARGET_FPS: u32 = 60;

const BPM: f64 = 150.0; // 120.0; // 170.0;
                        // Files read via ggez (usually music/font/images)
const MUSIC_PATH: &str = "/supersquare.mp3"; //"/metronome120.ogg"; // "/bbkkbkk.ogg";
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
    scheduler: Scheduler,
    time: Time,
    music: Source,
    keyboard: KeyboardState,
    assets: Assets,
    started: bool,
    player: Player,
    enemies: Vec<Box<dyn Enemy>>,
    last_beat: Beats,
    debug: Option<Box<dyn Enemy>>,
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
            enemies: vec![],
            last_beat: Beats(0.0),
            scheduler: Scheduler::new(),
            debug: None,
        };
        Ok(s)
    }

    fn reset(&mut self) {
        self.enemies.clear();
        self.last_beat = Beats(0.0);
        self.scheduler = Scheduler::new();
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

        let rect = WorldPos::as_screen_rect(WorldPos::origin(), WorldLen(100.0), WorldLen(100.0));
        Mesh::new_rectangle(ctx, DrawMode::stroke(2.0), rect, crate::color::DEBUG_RED)?
            .draw(ctx, DrawParam::default())?;

        let rect = WorldPos::as_screen_rect(WorldPos::origin(), WorldLen(10.0), WorldLen(10.0));
        Mesh::new_rectangle(ctx, DrawMode::stroke(2.0), rect, crate::color::DEBUG_RED)?
            .draw(ctx, DrawParam::default())?;

        Ok(())
    }

    fn draw_debug_hitbox(&self, ctx: &mut Context) -> Result<(), GameError> {
        if let Some(enemy) = &self.debug {
            // enemy.draw(ctx)?;

            for x in -20..20 {
                for y in -20..20 {
                    let pos = WorldPos {
                        x: x as f64,
                        y: y as f64,
                    };
                    let sdf = enemy.sdf(pos, self.time.get_beats());
                    let color = match sdf {
                        None => crate::color::GUIDE_GREY,
                        Some(sdf) => crate::ease::color_lerp(
                            crate::color::RED,
                            crate::color::GREEN,
                            (sdf.0.atan() / (std::f64::consts::PI / 2.0) + 1.0) / 2.0,
                        ),
                    };
                    Mesh::new_circle(
                        ctx,
                        DrawMode::fill(),
                        pos.as_screen_coords(),
                        1.0,
                        5.0,
                        color,
                    )?
                    .draw(ctx, DrawParam::default())?;
                }
            }
        }

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
            let curr_time = self.time.get_beats();

            if let Some(debug) = &mut self.debug {
                debug.update(curr_time);
            }

            self.player.update(physics_delta_time, &self.keyboard);
            for enemy in self.enemies.iter_mut() {
                enemy.update(curr_time);
                if let Some(sdf) = enemy.sdf(self.player.pos, curr_time) {
                    if sdf < self.player.size {
                        self.player.on_hit();
                    }
                }
            }
            if USE_MAP {
                self.scheduler
                    .update(self.time.get_beats(), &mut self.enemies);
            }

            // if self.last_beat < curr_time {
            //     self.last_beat = Beats(self.last_beat.0 + 1.0);
            //     let mut bullet = Box::new(Bullet::new(
            //         util::rand_circle_edge(WorldPos::origin(), 60.0),
            //         WorldPos::origin(),
            //         Beats(4.0),
            //     ));
            //     bullet.on_spawn(curr_time);
            //     self.enemies.push(bullet);

            //     let mut laser = Box::new(Laser::new_through_point(
            //         WorldPos::origin(),
            //         (self.last_beat.0) * std::f64::consts::PI / 12.0,
            //         Beats(0.25),
            //     ));

            //     laser.on_spawn(curr_time);
            //     // self.debug = Some(laser);
            //     self.enemies.push(laser);
            // }

            // Delete all dead enemies
            self.enemies
                .retain(|e| e.lifetime_state(curr_time) != EnemyLifetime::Dead);

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
                self.reset();
            } else {
                // Start the game. Also play the music.
                self.started = true;
                self.reset();
                self.time.reset();
                drop(self.music.play());
                self.music.set_volume(0.5);
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

        for enemy in self.enemies.iter() {
            enemy.draw(ctx)?;
        }
        let player_mesh = self.player.get_mesh(ctx)?;
        player_mesh.draw(
            ctx,
            DrawParam::default().dest(self.player.pos.as_screen_coords()),
        )?;

        self.draw_debug_time(ctx)?;
        self.draw_debug_world_lines(ctx)?;
        self.draw_debug_hitbox(ctx)?;
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
