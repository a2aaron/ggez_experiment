#![feature(drain_filter)]
#![feature(trait_alias)]
#![feature(float_interpolation)]
#![feature(try_blocks)]

use std::env;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use ggez::event::{KeyCode, KeyMods};
use ggez::graphics::mint::Point2;
use ggez::graphics::{
    Color, DrawMode, DrawParam, Drawable, Font, Mesh, PxScale, Rect, Text, TextFragment,
};
use ggez::{conf, event, graphics, timer, Context, ContextBuilder, GameError, GameResult};

use kira::instance::handle::InstanceHandle;
use kira::instance::{InstanceSettings, StopInstanceSettings};
use kira::manager::{AudioManager, AudioManagerSettings};
use kira::sound::handle::SoundHandle;
use kira::sound::{Sound, SoundSettings};

use cgmath as cg;

use chart::Scheduler;
use color::{RED, WHITE};
use ease::{BeatEasing, Lerp};
use enemy::{Enemy, EnemyDurations, EnemyLifetime, Laser};
use keyboard::KeyboardState;
use parse::SongMap;
use player::Player;
use time::{to_secs, Beats, Time};
use world::{WorldLen, WorldPos};

use crate::time::Seconds;

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
// const ARIAL_PATH: &str = "/Arial.ttf";
const FIRACODE_PATH: &str = "/FiraCode-Regular.ttf";

pub const WINDOW_WIDTH: f32 = 1.5 * 640.0;
pub const WINDOW_HEIGHT: f32 = 1.5 * 480.0;

/// Stores assets like fonts, music, sprite images, etc
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

pub struct EnemyGroup {
    pub enemies: Vec<Box<dyn Enemy>>,
    pub use_hitbox: bool,
    pub do_render: bool,
    pub render_warmup: bool,
    pub fadeout: Option<BeatEasing<Color>>,
    pub rotation: Option<(BeatEasing<f64>, WorldPos)>,
}

impl EnemyGroup {
    fn new() -> EnemyGroup {
        EnemyGroup {
            enemies: Vec::with_capacity(16),
            use_hitbox: true,
            do_render: true,
            render_warmup: true,
            fadeout: None,
            rotation: None,
        }
    }

    fn update(&mut self, player: &mut Player, curr_time: Beats) {
        let rotated_about = self.rotation_ease(curr_time);
        for enemy in self.enemies.iter_mut() {
            enemy.update(curr_time);
            if let Some(sdf) = enemy.sdf(player.pos, curr_time, rotated_about) {
                if sdf < player.size && self.use_hitbox {
                    player.on_hit();
                }
            }
        }

        // remove dead enemies
        self.enemies
            .retain(|e| e.lifetime_state(curr_time) != EnemyLifetime::Dead);
    }

    fn draw(&self, ctx: &mut Context, curr_time: Beats) -> GameResult<()> {
        if !self.do_render {
            return Ok(());
        }

        for enemy in self.enemies.iter() {
            if !self.render_warmup && enemy.lifetime_state(curr_time) == EnemyLifetime::Warmup {
                continue;
            }

            if let Some((mesh, param)) =
                enemy.draw(ctx, curr_time, self.rotation_ease(curr_time))?
            {
                let param = if let Some(fadeout) = &self.fadeout {
                    param.color(fadeout.ease(curr_time))
                } else {
                    param
                };

                mesh.draw(ctx, param)?;
            }
        }

        Ok(())
    }

    fn rotation_ease(&self, curr_time: Beats) -> Option<(WorldPos, f64)> {
        self.rotation
            .as_ref()
            .map(|(easing, rot_point)| (*rot_point, easing.ease(curr_time)))
    }
}

pub struct InnerWorldState {
    pub player: Player,
    pub groups: Vec<EnemyGroup>,
}

pub struct WorldState {
    inner: InnerWorldState,
    music: Option<SoundHandle>,
    audio_manager: AudioManager,
    scheduler: Scheduler,
    started: bool,
    debug: Option<Box<dyn Enemy>>,
    instance_handle: Option<InstanceHandle>,
}

impl WorldState {
    pub fn new<P: AsRef<Path>>(base_folder: P, map: &SongMap) -> WorldState {
        fn try_read(
            audio_manager: &mut AudioManager,
            path: impl AsRef<Path>,
        ) -> anyhow::Result<SoundHandle> {
            let music_file = std::fs::read(path)?;
            let sound = Sound::from_mp3_reader(music_file.as_slice(), SoundSettings::default())?;
            let song_handle = audio_manager.add_sound(sound)?;
            Ok(song_handle)
        }

        let mut audio_manager = AudioManager::new(AudioManagerSettings::default()).unwrap();
        let music = if let Some(path) = &map.music_path {
            let path = base_folder.as_ref().join(path);
            match try_read(&mut audio_manager, &path) {
                Ok(handle) => Some(handle),
                Err(err) => {
                    log::warn!("Couldn't read music file from path {:?}: {}", path, err);
                    None
                }
            }
        } else {
            None
        };

        WorldState {
            inner: InnerWorldState {
                player: map.player,
                groups: {
                    let mut vec = Vec::with_capacity(8);
                    vec.resize_with(8, EnemyGroup::new);
                    vec
                },
            },
            music,
            audio_manager,
            started: false,
            scheduler: Scheduler::new(map),
            debug: None,
            instance_handle: None,
        }
    }

    fn update(
        &mut self,
        _ctx: &mut Context,
        keyboard: &KeyboardState,
        physics_delta_time: f64,
        curr_time: Beats,
    ) -> GameResult<()> {
        if !self.started {
            return Ok(());
        }

        if let Some(debug) = &mut self.debug {
            debug.update(curr_time);

            if debug.lifetime_state(curr_time) == EnemyLifetime::Dead {
                self.debug = None;
            }
        }

        self.inner.player.update(physics_delta_time, keyboard);

        for group in self.inner.groups.iter_mut() {
            group.update(&mut self.inner.player, curr_time);
        }

        self.update_scheduler(curr_time);

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context, curr_time: Beats) -> GameResult<()> {
        for group in self.inner.groups.iter() {
            group.draw(ctx, curr_time)?;
        }

        let player_mesh = self.inner.player.get_mesh(ctx)?;
        player_mesh.draw(
            ctx,
            DrawParam::default().dest(self.inner.player.pos.as_screen_coords()),
        )?;

        Ok(())
    }

    fn update_scheduler(&mut self, time: Beats) {
        self.scheduler.update(time, &mut self.inner);
    }

    fn stop_world(&mut self) {
        // Stop the game, pausing the music, fetching a new Source instance, and
        // rebuild the scheduler work queue.
        self.started = false;
        if let Some(handle) = &mut self.instance_handle {
            match handle.stop(StopInstanceSettings::new()) {
                Ok(()) => self.instance_handle = None,
                Err(err) => log::error!("Error stopping music: {}", err),
            }
        }
    }

    fn start_world(&mut self, map: &SongMap, time: &mut Time) {
        // Reset the player and groups
        self.inner.player = map.player;
        self.inner.groups = {
            let mut vec = Vec::with_capacity(8);
            vec.resize_with(8, EnemyGroup::new);
            vec
        };

        // Simulate all events up to this point. We do this before the level
        // starts in order to reduce the amount of BeatActions the scheduler needs
        // to perform immediately, which could be a lot if there were many events.
        self.scheduler = Scheduler::new(map);
        self.update_scheduler(map.skip_amount);

        let skip_amount = to_secs(map.skip_amount, map.bpm);

        // Play the music
        if let Some(music) = &mut self.music {
            match music.play(
                InstanceSettings::new()
                    .volume(0.5)
                    .start_position(skip_amount.0),
            ) {
                Ok(handle) => self.instance_handle = Some(handle),
                Err(err) => log::error!("Error starting music: {}", err),
            }
        } else {
            log::warn!("No music loaded!")
        }

        // Reset the timer
        *time = Time::new(map.bpm, skip_amount);

        self.started = true;
    }

    #[allow(dead_code)]
    fn draw_debug_hitbox(&self, ctx: &mut Context, time: &Time) -> Result<(), GameError> {
        let curr_time = time.get_beats();
        let rotated_about = if ggez::input::keyboard::is_key_pressed(ctx, KeyCode::V) {
            Some((self.inner.player.pos, curr_time.0 * 400.0))
        } else {
            None
        };

        if let Some(enemy) = &self.debug {
            if ggez::input::keyboard::is_key_pressed(ctx, KeyCode::C) {
                if let Some((mesh, param)) = enemy.draw(ctx, curr_time, rotated_about)? {
                    mesh.draw(ctx, param)?;
                }
            }
            if ggez::input::keyboard::is_key_pressed(ctx, KeyCode::Z) {
                for x in -20..20 {
                    for y in -20..20 {
                        let pos = WorldPos {
                            x: x as f64,
                            y: y as f64,
                        };
                        let sdf = enemy.sdf(pos, curr_time, rotated_about);
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
        }

        Ok(())
    }
}

pub struct LevelSelect {
    levels: Vec<Level>,
    current_selection: usize,
}

impl LevelSelect {
    fn new(levels_folder: impl AsRef<Path>) -> anyhow::Result<LevelSelect> {
        let mut levels = vec![];
        let levels_folder = std::fs::read_dir(levels_folder.as_ref())?;
        for level in levels_folder {
            let result: anyhow::Result<(Level, PathBuf)> = try {
                let level = level?;
                let path = level.path();
                let level = Level::new(&path)?;
                (level, path)
            };
            match result {
                Ok((level, path)) => {
                    levels.push(level);
                    log::info!("Loaded level from path {:?}", path)
                }
                Err(err) => log::warn!("Couldn't load level: {}", err),
            }
        }

        Ok(LevelSelect {
            levels,
            current_selection: 0,
        })
    }

    fn update(&mut self) {
        // Nothing...?
    }

    fn draw(&self, ctx: &mut Context, font: Font) -> GameResult<()> {
        if let Some(level) = self.current_level() {
            let fragment = TextFragment {
                text: level.name,
                color: Some(color::DEBUG_RED),
                font: Some(font),
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
        }
        Ok(())
        // for (i, level) in self.levels.iter().enumerate() {
        //     let pos = WorldPos {
        //         x: -20.0,
        //         y: i as f64 * 10.0 - 25.0,
        //     };
        // }
    }

    fn change_song(&mut self, delta: i32) {
        let new_selection = delta + self.current_selection as i32;
        self.current_selection = i32::rem_euclid(new_selection, self.levels.len() as i32) as usize;
    }

    fn current_level(&self) -> Option<Level> {
        if self.levels.is_empty() {
            None
        } else {
            Some(self.levels[self.current_selection].clone())
        }
    }
}

impl Default for LevelSelect {
    fn default() -> Self {
        LevelSelect {
            levels: vec![],
            current_selection: 0,
        }
    }
}

#[derive(Clone)]
pub struct Level {
    name: String,
    map_folder: PathBuf,
}

impl Level {
    fn new(base_folder: impl AsRef<Path>) -> anyhow::Result<Level> {
        let base_folder = base_folder.as_ref();
        if base_folder.is_dir() {
            Ok(Level {
                name: base_folder
                    .file_name()
                    .unwrap_or_else(|| OsStr::new("No Name"))
                    .to_string_lossy()
                    .to_string(),
                map_folder: base_folder.to_path_buf(),
            })
        } else {
            Err(anyhow::anyhow!("path {:?} is not a folder!", base_folder))
        }
    }

    fn load_level<P: AsRef<Path>>(&self, resource_path: P) -> anyhow::Result<SongMap> {
        let base_folder = resource_path.as_ref().join(&self.map_folder);
        let source = std::fs::read(base_folder.join("main.lua"))?;
        let song_map = SongMap::run_lua(base_folder, &source)?;
        Ok(song_map)
    }
}

pub enum Scene {
    LevelSelect(LevelSelect),
    MainGame(WorldState, Time, PathBuf),
}

struct MainState {
    current_scene: Scene,
    keyboard: KeyboardState,
    assets: Assets,
    resource_path: PathBuf,
}

impl MainState {
    fn new(ctx: &mut Context) -> MainState {
        // TODO: this is a stupid way to do this, use an actual virtual file system
        let resource_path = match env::var("CARGO_MANIFEST_DIR") {
            Ok(manifest_dir) => {
                let mut path = PathBuf::from(manifest_dir);
                path.push("resources");
                path
            }
            Err(err) => panic!("{}", err),
        };
        MainState {
            current_scene: Scene::LevelSelect(LevelSelect::new(&resource_path).unwrap_or_default()),
            keyboard: KeyboardState::default(),
            assets: Assets::new(ctx),
            resource_path,
        }
    }
}

impl event::EventHandler<GameError> for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        // Lock the framerate at 60 FPS
        while timer::check_update_time(ctx, TARGET_FPS) {
            let physics_delta_time = 1.0 / f64::from(TARGET_FPS);

            match &mut self.current_scene {
                Scene::LevelSelect(level_select) => level_select.update(),
                Scene::MainGame(world, time, _) => {
                    time.update();
                    let curr_time = time.get_beats();
                    world.update(ctx, &self.keyboard, physics_delta_time, curr_time)?
                }
            }

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
        match &mut self.current_scene {
            Scene::LevelSelect(level_select) => match keycode {
                KeyCode::Up | KeyCode::W => {
                    level_select.change_song(1);
                }
                KeyCode::Down | KeyCode::S => {
                    level_select.change_song(-1);
                }
                KeyCode::Space => {
                    let level = level_select.current_level();
                    if let Some(level) = level {
                        match level.load_level(&self.resource_path) {
                            Ok(map) => {
                                let world = WorldState::new(&level.map_folder, &map);
                                let time = Time::new(map.bpm, Seconds(0.0));
                                self.current_scene = Scene::MainGame(world, time, level.map_folder);
                            }
                            Err(err) => log::error!("Couldn't load map: {}", err),
                        }
                    }
                }
                _ => (),
            },
            Scene::MainGame(world, time, base_folder) => {
                if keycode == KeyCode::P {
                    if world.started {
                        log::info!("-- Stopped Game --");
                        world.stop_world();
                    } else {
                        log::info!("++ Started Game ++");

                        match try_read_map(&base_folder) {
                            Ok(map) => {
                                if ggez::input::keyboard::is_key_pressed(ctx, KeyCode::R) {
                                    log::info!("Reloaded music files!");
                                    *world = WorldState::new(&base_folder, &map);
                                }
                                world.start_world(&map, time);
                            }
                            Err(err) => {
                                log::warn!(
                                    "Couldn't load map from path {:?}! {:?}",
                                    base_folder,
                                    err
                                )
                            }
                        }
                    }
                }

                // Debug spawn
                if keycode == KeyCode::X {
                    world.debug = Some(Box::new(crate::enemy::Laser::new_through_points(
                        world.inner.player.pos,
                        WorldPos::origin(),
                        time.get_beats(),
                        EnemyDurations::default_laser(Beats(16.0)),
                        &Laser::default_outline_color(),
                        &Laser::default_outline_keyframes(),
                    )));
                }
            }
        }

        self.keyboard.update(keycode, true);
    }

    fn key_up_event(&mut self, _ctx: &mut Context, keycode: KeyCode, _keymod: KeyMods) {
        self.keyboard.update(keycode, false);
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::clear(ctx, ggez::graphics::Color::BLACK);

        match &mut self.current_scene {
            Scene::LevelSelect(level_select) => level_select.draw(ctx, self.assets.debug_font)?,
            Scene::MainGame(world, time, _) => {
                let curr_time = time.get_beats();
                world.draw(ctx, curr_time)?;
                draw_debug_world_lines(ctx)?;
                draw_debug_time(ctx, self.assets.debug_font, world, time)?;
                draw_debug_metronome(ctx, time)?;
            }
        }

        graphics::present(ctx)?;

        // if timer::ticks(ctx) % 1000 == 0 {
        //     log::warn!("remaining update: {:?}", timer::remaining_update_time(ctx));
        // }

        // let delta = ggez::timer::delta(ctx);
        // if delta > std::time::Duration::from_millis(16) {
        //     log::warn!("Slow frame! {:?}", delta);
        // }

        let sleep_duration = ggez::timer::remaining_update_time(ctx);
        spin_sleep::sleep(sleep_duration);
        Ok(())
    }
}

fn try_read_map(base_folder: impl AsRef<Path>) -> anyhow::Result<SongMap> {
    let source = std::fs::read(base_folder.as_ref().join("main.lua"))?;
    let songmap = SongMap::run_lua(base_folder, &source)?;
    Ok(songmap)
}

/// Draw debug text at the bottom of the screen showing the time in the song, in beats.
fn draw_debug_time(
    ctx: &mut Context,
    font: Font,
    world: &mut WorldState,
    time: &Time,
) -> GameResult<()> {
    let beat_time = time.get_beats();
    let delta = ggez::timer::delta(ctx);
    let text = format!(
        "Measure: {}, Beat: {:.2?}\nPlayer position: {:.2?} ({:.2?}, {:.2?})\nDelta: {:.2?}",
        (beat_time.0 / 4.0) as i32,
        beat_time.0,
        world.inner.player.pos,
        world.inner.player.pos.as_screen_coords().x,
        world.inner.player.pos.as_screen_coords().y,
        delta
    );

    let fragment = TextFragment {
        text,
        color: Some(color::DEBUG_RED),
        font: Some(font),
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

fn draw_debug_world_lines(ctx: &mut Context) -> Result<(), GameError> {
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

fn draw_debug_metronome(ctx: &mut Context, time: &Time) -> Result<(), GameError> {
    if ggez::input::keyboard::is_key_pressed(ctx, KeyCode::C) {
        let curr_time = time.get_beats();
        let percent = curr_time.0 % 1.0;
        let beat = (curr_time.0 as usize) % 4;

        let point = [
            (100.0, 100.0),
            (200.0, 100.0),
            (200.0, 200.0),
            (100.0, 200.0),
        ][beat];

        let rect = Rect::new(point.0, point.1, 100.0, 100.0);
        let color = Color::lerp(RED, WHITE, percent);
        Mesh::new_rectangle(ctx, DrawMode::fill(), rect, color)?.draw(ctx, DrawParam::default())?;
    }
    Ok(())
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
        log::info!("Adding path {:?}", path);
        // We need this re-assignment alas, see
        // https://aturon.github.io/ownership/builders.html
        // under "Consuming builders"
        cb = cb.add_resource_path(path);
    } else {
        log::warn!("Not building from cargo");
    }

    // gfx_device_gl ends up spamming the log with Info messages.
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .with_module_level("gfx_device_gl", log::LevelFilter::Warn)
        .init()
        .unwrap();

    let (mut ctx, events_loop) = cb.build().unwrap();
    let state = MainState::new(&mut ctx);
    ggez::event::run(ctx, events_loop, state);
}
