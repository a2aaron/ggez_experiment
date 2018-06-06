extern crate ggez;
extern crate rand;

mod enemy;
mod grid;
mod keyboard;
mod player;
mod util;

use std::env;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use ggez::audio::Source;
use ggez::event::{Keycode, Mod};
use ggez::graphics::{Color, DrawMode, Mesh, Point2};
use ggez::*;

use enemy::Enemy;
use grid::Grid;
use keyboard::KeyboardState;
use player::Ball;
use util::*;

const BPM: f64 = 170.0;
const MUSIC_PATH: &str = "/bbkkbkk.ogg";

struct MainState {
    ball: Ball,
    enemies: Vec<Enemy>,
    grid: Grid,
    arrow: Arrow,
    keyboard: KeyboardState,
    background: Color,
    time: Instant,
    bpm: Duration,
    music: Source,
    started: bool,
    beat_num: usize,
    measure_num: usize,
}

impl MainState {
    fn new(ctx: &mut Context) -> GameResult<MainState> {
        let s = MainState {
            ball: Default::default(),
            enemies: Default::default(),
            grid: Grid::default(),
            arrow: Default::default(),
            keyboard: Default::default(),
            background: Color::new(0.0, 0.0, 0.0, 1.0),
            time: Instant::now(),
            bpm: bpm_to_duration(BPM),
            music: audio::Source::new(ctx, MUSIC_PATH)?,
            started: false,
            beat_num: 0,
            measure_num: 0,
        };
        Ok(s)
    }

    fn beat(&mut self, _ctx: &mut Context) {
        self.beat_num += 1;
        if self.beat_num % 4 == 0 {
            self.measure_num += 1
        }
        {
            fn spawn(state: &mut MainState, num: usize, spread: isize) {
                for _ in 0..num {
                    let start_pos = rand_edge(state.grid.grid_size);
                    let end_pos = rand_around(state.grid.grid_size, state.ball.goal, spread);
                    state.enemies.push(Enemy::new(start_pos, end_pos));
                }
            };
            // 0 4 8 (12) 16 (20) 24 (32) 40! 48 56!! 64 72! 80 88(end)
            match self.measure_num {
                0...3 => (),
                4...7 => spawn(self, 1, 4),
                8...15 => spawn(self, 1, 2),
                16...23 => spawn(self, 2, 4),
                24...39 => spawn(self, 3, 4),
                40...47 => if self.beat_num % 4 == 0 {
                    spawn(self, 10, 0);
                },
                48...55 => spawn(self, 1, 0),
                _ => spawn(self, 3, 5),
            }
        }
        // println!("{}", self.measure_num);
    }
}

impl event::EventHandler for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        if !self.started {
            return Ok(());
        }

        let time_in_beat = Instant::now().duration_since(self.time);
        if time_in_beat > self.bpm {
            self.beat(ctx);
            self.time += self.bpm;
        }
        let beat_percent = timer::duration_to_f64(time_in_beat) / timer::duration_to_f64(self.bpm);
        let color = (rev_quad(beat_percent) / 10.0) as f32;
        self.background = Color::new(color, color, color, 1.0);

        self.grid.update(beat_percent);
        self.ball.update(ctx);
        self.arrow.update();

        let mut was_hit = false;
        for enemy in self.enemies.iter_mut() {
            enemy.update(beat_percent);
            if self.ball.hit(enemy) {
                was_hit = true
            }
        }

        if was_hit {
            self.ball.on_hit();
        }

        self.enemies.retain(|e| e.alive);

        if let Ok(direction) = self.keyboard.direction() {
            self.ball.key_down_event(direction);
            self.arrow.key_down_event(direction);
        }

        Ok(())
    }

    fn key_down_event(&mut self, ctx: &mut Context, keycode: Keycode, _keymod: Mod, _repeat: bool) {
        use Keycode::*;
        match keycode {
            P => {
                self.started = true;
                self.time = Instant::now();
                drop(self.music.play());
            }
            S => {
                self.started = false;
                self.music.stop();
                drop(self.music = audio::Source::new(ctx, MUSIC_PATH).unwrap());
                self.beat_num = 0;
            }
            _ => (),
        }

        self.keyboard.update(keycode, true);
    }

    fn key_up_event(&mut self, _ctx: &mut Context, keycode: Keycode, _keymod: Mod, _repeat: bool) {
        self.keyboard.update(keycode, false);
        if let Ok(direction) = self.keyboard.direction() {
            self.arrow.direction = Some(direction);
        }
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::clear(ctx);
        graphics::set_background_color(ctx, self.background);
        self.grid.draw(ctx)?;
        self.ball.draw(ctx, &self.grid)?;
        self.arrow.draw(ctx)?;
        for enemy in self.enemies.iter() {
            enemy.draw(ctx, &self.grid)?;
        }
        graphics::present(ctx);
        Ok(())
    }
}

struct Arrow {
    direction: Option<Direction8>,
    opacity: f32,
    x: f32,
    y: f32,
}

impl Arrow {
    fn update(&mut self) {
        self.opacity -= 0.03;
        if self.opacity < 0.0 {
            self.opacity = 0.0
        }
    }

    fn key_down_event(&mut self, direction: Direction8) {
        self.direction = Some(direction);
        self.opacity = 1.0;
    }

    fn draw(&self, ctx: &mut Context) -> GameResult<()> {
        use Direction8::*;
        use DrawMode::*;

        if self.direction == None {
            return Ok(());
        }

        let prev_color = graphics::get_color(ctx);

        graphics::set_color(ctx, Color::new(1.0, 1.0, 1.0, self.opacity))?;
        let angle: f32 = match self.direction.unwrap() {
            Right => 0.0f32,
            RightDown => 45.0f32,
            Down => 90.0f32,
            LeftDown => 135.0f32,
            Left => 180.0f32,
            LeftUp => 225.0f32,
            Up => 270.0f32,
            RightUp => 315.0f32,
        }.to_radians();

        let points = [
            Point2::new(0.0, 0.0),
            Point2::new(100.0, 0.0),
            Point2::new(100.0, 10.0),
            Point2::new(0.0, 10.0),
        ];
        let rect = Mesh::new_polygon(ctx, Fill, &points)?;
        graphics::draw(ctx, &rect, Point2::new(self.x, self.y), angle)?;
        graphics::set_color(ctx, prev_color)?;
        Ok(())
    }
}

impl Default for Arrow {
    fn default() -> Self {
        Arrow {
            direction: None,
            opacity: 0.0,
            x: 400.0,
            y: 400.0,
        }
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
