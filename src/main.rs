#![feature(drain_filter)]
#![feature(trait_alias)]
#![feature(float_interpolation)]

use std::env;
use std::path::PathBuf;

use ease::Lerp;
use ggez::audio::{SoundSource, Source};
use ggez::event::{KeyCode, KeyMods};
use ggez::graphics::mint::Point2;
use ggez::graphics::{
    Color, DrawMode, DrawParam, Drawable, Font, Mesh, PxScale, Text, TextFragment,
};
use ggez::{audio, conf, event, graphics, timer, Context, ContextBuilder, GameError, GameResult};

use cgmath as cg;

use chart::Scheduler;
use enemy::{Enemy, EnemyLifetime};
use keyboard::KeyboardState;
use player::Player;
use time::{to_secs, Beats, Time};
use world::{WorldLen, WorldPos};

use crate::parse::SongMap;

mod chart;
mod color;
mod ease;
mod enemy;
mod keyboard;
mod parse;
mod player;
mod time;
mod util;
mod world;

const TARGET_FPS: u32 = 60;

// Files read via ggez (usually music/font/images)
const MUSIC_PATH: &str = "/supersquare.mp3"; //"/metronome120.ogg"; // "/bbkkbkk.ogg";
                                             // const ARIAL_PATH: &str = "/Arial.ttf";
const FIRACODE_PATH: &str = "/FiraCode-Regular.ttf";
// Files manually read by me (usually maps)
const MAP_PATH: &str = "/square.rhai";

pub const WINDOW_WIDTH: f32 = 640.0;
pub const WINDOW_HEIGHT: f32 = 480.0;

/// Stores assets like fonts, music, sprite images, etc
/// TODO: Add music stuff here.
struct Assets {
    debug_font: Font,
    music: Source,
}

impl Assets {
    fn new(ctx: &mut Context) -> GameResult<Assets> {
        Ok(Assets {
            debug_font: Font::new(ctx, FIRACODE_PATH)?,
            music: audio::Source::new(ctx, MUSIC_PATH)?,
        })
    }
}

pub struct WorldState {
    pub player: Player,
    pub enemies: Vec<Box<dyn Enemy>>,
    pub fade_out: Vec<Box<dyn Enemy>>,
    pub show_warmup: bool,
}

impl WorldState {
    fn new() -> WorldState {
        WorldState {
            player: Player::new(),
            // Usually there are less than 128 enemies on screen, right??
            enemies: Vec::with_capacity(128),
            fade_out: Vec::with_capacity(128),
            show_warmup: true,
        }
    }
    fn update(
        &mut self,
        _ctx: &mut Context,
        keyboard: &KeyboardState,
        physics_delta_time: f64,
        curr_time: Beats,
    ) -> GameResult<()> {
        self.player.update(physics_delta_time, keyboard);

        for enemy in self.enemies.iter_mut() {
            enemy.update(curr_time);
            if let Some(sdf) = enemy.sdf(self.player.pos, curr_time) {
                if sdf < self.player.size {
                    self.player.on_hit();
                }
            }
        }

        // Update enemies in the fade_out vector, but don't do any hit detection
        // on them.
        for enemy in self.fade_out.iter_mut() {
            enemy.update(curr_time);
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context, curr_time: Beats) -> GameResult<()> {
        for enemy in self.enemies.iter() {
            if !self.show_warmup && enemy.lifetime_state(curr_time) == EnemyLifetime::Warmup {
                continue;
            }

            if let Some((mesh, param)) = enemy.draw(ctx, curr_time)? {
                mesh.draw(ctx, param)?;
            }
        }

        for enemy in self.fade_out.iter() {
            if let Some((mesh, param)) = enemy.draw(ctx, curr_time)? {
                mesh.draw(ctx, param.color(Color::new(0.0, 1.0, 0.0, 1.0)))?;
            }
        }

        let player_mesh = self.player.get_mesh(ctx)?;
        player_mesh.draw(
            ctx,
            DrawParam::default().dest(self.player.pos.as_screen_coords()),
        )?;

        Ok(())
    }
}

struct MainState {
    scheduler: Scheduler,
    time: Time,
    keyboard: KeyboardState,
    assets: Assets,
    started: bool,
    world: WorldState,
    debug: Option<Box<dyn Enemy>>,
    map: SongMap,
}

impl MainState {
    fn new(ctx: &mut Context) -> GameResult<MainState> {
        let map = SongMap::read_map(ctx, MAP_PATH).unwrap_or_default();
        let s = MainState {
            keyboard: KeyboardState::default(),
            time: Time::new(map.bpm, time::Seconds(0.0)),
            started: false,
            assets: Assets::new(ctx)?,
            world: WorldState::new(),
            scheduler: Scheduler::new(ctx, &map),
            debug: None,
            map,
        };
        Ok(s)
    }

    fn reset(&mut self, ctx: &mut Context) {
        match SongMap::read_map(ctx, MAP_PATH) {
            Ok(map) => self.map = map,
            Err(err) => println!("{:?}", err),
        }
        let skip_amount = to_secs(self.map.skip_amount, self.map.bpm);
        self.assets.music = audio::Source::new(ctx, MUSIC_PATH).unwrap();
        self.world = WorldState::new();
        self.scheduler = Scheduler::new(ctx, &self.map);
        self.assets.music.set_skip_amount(skip_amount.as_duration());
        self.assets.music.set_volume(0.5);

        // Simulate all events up to this point. We do this before the level
        // starts in order to reduce the amount of BeatActions the scheduler needs
        // to perform immediately, which could be a lot if there were many events.
        // self.update_with_time(0.0, self.map.skip_amount);

        self.time = Time::new(self.map.bpm, skip_amount);
    }

    fn update_scheduler(&mut self, time: Beats) {
        self.scheduler.update(time, &mut self.world);

        // Delete all dead enemies
        self.world
            .enemies
            .retain(|e| e.lifetime_state(time) != EnemyLifetime::Dead);

        self.world
            .fade_out
            .retain(|e| e.lifetime_state(time) != EnemyLifetime::Dead);
    }

    /// Draw debug text at the bottom of the screen showing the time in the song, in beats.
    fn draw_debug_time(&mut self, ctx: &mut Context) -> GameResult<()> {
        let beat_time = self.time.get_beats();
        let delta = ggez::timer::delta(ctx);
        let text = format!(
            "Measure: {}, Beat: {:.2?}\nPlayer position: {:.2?} ({:.2?}, {:.2?})\nDelta: {:.2?}",
            (beat_time.0 / 4.0) as i32,
            beat_time.0,
            self.world.player.pos,
            self.world.player.pos.as_screen_coords().x,
            self.world.player.pos.as_screen_coords().y,
            delta
        );
        if delta > std::time::Duration::from_millis(16) {
            log::warn!("Slow frame! {:?}", delta);
        }
        let fragment = TextFragment {
            text,
            color: Some(color::DEBUG_RED),
            font: Some(self.assets.debug_font),
            scale: Some(PxScale::from(18.0)),
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
        let origin = WorldPos::origin().as_screen_coords_cg();
        Mesh::new_line(
            ctx,
            &[
                util::into_mint(origin + cg::Vector2::new(-5.0, 0.0)),
                util::into_mint(origin + cg::Vector2::new(5.0, 0.0)),
            ],
            2.0,
            crate::color::DEBUG_RED,
        )?
        .draw(ctx, DrawParam::default())?;
        Mesh::new_line(
            ctx,
            &[
                util::into_mint(origin + cg::Vector2::new(0.0, 5.0)),
                util::into_mint(origin + cg::Vector2::new(0.0, -5.0)),
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
        let curr_time = self.time.get_beats();

        if let Some(enemy) = &self.debug {
            if ggez::input::keyboard::is_key_pressed(ctx, KeyCode::C) {
                enemy.draw(ctx, curr_time)?;
            }

            for x in -20..20 {
                for y in -20..20 {
                    let pos = WorldPos {
                        x: x as f64,
                        y: y as f64,
                    };
                    let sdf = enemy.sdf(pos, self.time.get_beats());
                    let color = match sdf {
                        None => crate::color::GUIDE_GREY,
                        Some(sdf) => Color::lerp(
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

                if debug.lifetime_state(curr_time) == EnemyLifetime::Dead {
                    self.debug = None;
                }
            }

            self.world
                .update(ctx, &self.keyboard, physics_delta_time, curr_time)?;

            self.update_scheduler(curr_time);

            ggez::graphics::window(ctx).set_title(&format!("{}", ggez::timer::fps(ctx)));
        }
        // ggez::timer::sleep(ggez::timer::remaining_update_time(ctx));

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
                drop(self.assets.music.stop(ctx));
                println!("Stopped");
            } else {
                // Start the game. Also play the music.
                println!("Started");
                self.started = true;
                self.reset(ctx);
                drop(self.assets.music.play(ctx));
            }
        }

        if keycode == KeyCode::X {
            self.debug = Some(Box::new(crate::enemy::Laser::new_through_points(
                WorldPos::origin(),
                self.world.player.pos,
                self.time.get_beats(),
                Beats(16.0),
            )));
        }

        self.keyboard.update(keycode, true);
    }

    fn key_up_event(&mut self, _ctx: &mut Context, keycode: KeyCode, _keymod: KeyMods) {
        self.keyboard.update(keycode, false);
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::clear(ctx, ggez::graphics::Color::BLACK);
        let curr_time = self.time.get_beats();

        self.world.draw(ctx, curr_time)?;

        self.draw_debug_time(ctx)?;
        self.draw_debug_world_lines(ctx)?;
        self.draw_debug_hitbox(ctx)?;
        graphics::present(ctx)?;
        // We are done with this frame, sleep till the next frame
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

    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Warn)
        .init()
        .unwrap();

    let (mut ctx, events_loop) = cb.build().unwrap();
    let state = MainState::new(&mut ctx).unwrap();
    ggez::event::run(ctx, events_loop, state);
}
