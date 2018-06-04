extern crate ggez;
extern crate rand;

mod keyboard;
mod player;
mod util;
mod enemy;

use std::env;
use std::path::PathBuf;
use std::time::Duration;

use ggez::audio::Source;
use ggez::event::{Keycode, Mod};
use ggez::graphics::{Color, DrawMode, Mesh, Point2};
use ggez::*;

use keyboard::KeyboardState;
use player::Ball;
use util::*;
use enemy::Enemy;

const BPM: f64 = 170.0;
const MUSIC_PATH: &str = "/bbkkbkk.ogg";

struct MainState {
    ball: Ball,
    arrow: Arrow,
    keyboard: KeyboardState,
    time: Duration,
    background: Color,
    bpm: Duration,
    music: Source,
    enemies: Vec<Enemy>,
}

impl MainState {
    fn new(ctx: &mut Context) -> GameResult<MainState> {
        let s = MainState {
            ball: Default::default(),
            keyboard: Default::default(),
            arrow: Default::default(),
            time: Duration::new(0, 0),
            background: Color::new(0.0, 0.0, 0.0, 1.0),
            bpm: bpm_to_duration(BPM),
            music: audio::Source::new(ctx, MUSIC_PATH)?,
            enemies: Default::default(),
        };
        // s.music.play()?;
        Ok(s)
    }

    fn beat(&mut self, ctx: &mut Context) {
        println!("Beat!");
        if self.enemies.len() < 100 {
            self.enemies.push(Enemy::spawn(
                ctx.conf.window_mode.width as f32,
                ctx.conf.window_mode.height as f32,
                Direction4::rand(),
            ));
        }
    }
}

impl event::EventHandler for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        let time_in_beat = timer::get_time_since_start(ctx) - self.time;
        if time_in_beat > self.bpm {
            self.beat(ctx);
            self.time = timer::get_time_since_start(ctx);
        }
        let beat_percent = timer::duration_to_f64(time_in_beat) / timer::duration_to_f64(self.bpm);
        let color = (rev_quad(beat_percent) / 10.0) as f32;
        self.background = Color::new(color, color, color, 1.0);

        self.ball.update(ctx, beat_percent);
        self.arrow.update();
        for enemy in self.enemies.iter_mut() {
            enemy.update(ctx);
        }

        self.enemies.retain(|e| e.alive);

        self.ball.speed = if self.keyboard.space.is_down {
            0.01
        } else {
            0.2
        };

        if let Ok(direction) = self.keyboard.direction() {
            self.ball.key_down_event(direction);
            self.arrow.key_down_event(direction);
            println!("{:?}", direction);
        }

        Ok(())
    }

    fn key_down_event(
        &mut self,
        _ctx: &mut Context,
        keycode: Keycode,
        _keymod: Mod,
        _repeat: bool,
    ) {
        use Keycode::*;
        match keycode {
            P => drop(self.music.resume()),
            S => drop(self.music.pause()),
            _ => (),
        }

        self.keyboard.update(keycode, true);
    }

    fn key_up_event(&mut self, _ctx: &mut Context, keycode: Keycode, _keymod: Mod, _repeat: bool) {
        self.keyboard.update(keycode, false);
        if let Ok(direction) = self.keyboard.direction() {
            self.arrow.direction = Some(direction);
            println!("{:?}", self.arrow.direction);
        }
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::clear(ctx);
        graphics::set_background_color(ctx, self.background);
        self.ball.draw(ctx)?;
        self.arrow.draw(ctx)?;
        for enemy in self.enemies.iter() {
            enemy.draw(ctx)?;
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
