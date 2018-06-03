extern crate ggez;
extern crate rand;

mod keyboard;

use ggez::event::{Keycode, Mod};
use ggez::graphics::{Color, DrawMode, Mesh, Point2, Vector2};
use ggez::audio::{Source};
use ggez::*;
use std::collections::VecDeque;
use std::time::Duration;
use std::path::PathBuf;
use std::env;
use rand::{Rng, thread_rng};
use keyboard::{Direction, KeyboardState};

const WHITE: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};
const RED: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

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

struct Ball {
    pos: Point2,
    goal: Point2,
    speed: f32,
    keyframes: VecDeque<Point2>,
}

impl Ball {
    fn handle_boundaries(&mut self, width: f32, height: f32) {
        if self.pos[1] > height {
            self.pos[1] = height;
        } else if self.pos[1] < 0.0 {
            self.pos[1] = 0.0;
        }

        if self.pos[0] < 0.0 {
            self.pos[0] = width;
        } else if self.pos[0] > width {
            self.pos[0] = 0.0;
        }
    }

    fn update(&mut self, ctx: &mut Context) {
        if let Some(goal) = self.keyframes.pop_front() {
            let speed = (self.speed * (self.keyframes.len() + 1) as f32).min(1.0);
            self.pos = lerp(self.pos, goal, speed);
            if distance(self.pos, goal) > 0.01 {
                self.keyframes.push_front(goal);
            }
        }

        self.handle_boundaries(
            ctx.conf.window_mode.width as f32,
            ctx.conf.window_mode.height as f32,
        );
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::set_color(ctx, WHITE)?;
        graphics::circle(ctx, DrawMode::Fill, self.pos, 10.0, 2.0)?;
        graphics::set_color(ctx, RED)?;
        graphics::circle(ctx, DrawMode::Fill, self.goal, 3.0, 2.0)?;
        Ok(())
    }

    fn key_down_event(&mut self, direction: Direction) {
        use Direction::*;
        match direction {
            Left | LeftDown | LeftUp => self.goal[0] += -40.0,
            Right | RightDown | RightUp => self.goal[0] += 40.0,
            Up | Down | None => (),
        }
        match direction {
            Up | LeftUp | RightUp => self.goal[1] += -40.0,
            Down | LeftDown | RightDown => self.goal[1] += 40.0,
            Left | Right | None => (),
        }
        self.keyframes.push_back(self.goal.clone());
    }
}

impl Default for Ball {
    fn default() -> Self {
        Ball {
            pos: Point2::new(0.0, 0.0),
            goal: Point2::new(0.0, 0.0),
            speed: 0.0,
            keyframes: VecDeque::new(),
        }
    }
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
        s.music.play()?;
        Ok(s)
    }

    fn beat(&mut self, ctx: &mut Context) {
        println!("Beat!");
        if self.enemies.len() < 100 {
            self.enemies.push(Enemy::spawn(ctx.conf.window_mode.width as f32, ctx.conf.window_mode.height as f32, Wall::rand()));
        }
    }

    fn interbeat_update(&mut self, time_in_beat: Duration) {
        fn rev_quad(n: f64) -> f64 {
            (1.0 - n) * (1.0 - n)
        }
        let beat_percent = timer::duration_to_f64(time_in_beat)/timer::duration_to_f64(self.bpm);
        let color = (rev_quad(beat_percent)/2.0) as f32;
        self.background = Color::new(color, color, color, 1.0);
    }
}

impl event::EventHandler for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        self.ball.update(ctx);
        self.arrow.update();
        for enemy in self.enemies.iter_mut() {
            enemy.update(ctx);
        }

        self.enemies.retain(|e| e.alive);

        self.ball.speed = if self.keyboard.space.is_down { 0.01 } else { 0.2 };

        let time_in_beat = timer::get_time_since_start(ctx) - self.time;
        self.interbeat_update(time_in_beat);
        if time_in_beat > self.bpm {
            self.beat(ctx);
            self.time = timer::get_time_since_start(ctx);
        }
        if let Ok(direction) = self.keyboard.direction() {
            self.ball.key_down_event(direction);

            self.arrow.direction = direction;
            self.arrow.opacity = 1.0;
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
            self.arrow.direction = direction;
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
    direction: Direction,
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

    fn draw(&self, ctx: &mut Context) -> GameResult<()> {
        use Direction::*;
        use DrawMode::*;

        if self.direction == None {
            return Ok(());
        }

        let prev_color = graphics::get_color(ctx);

        graphics::set_color(ctx, Color::new(1.0, 1.0, 1.0, self.opacity))?;
        let angle: f32 = match self.direction {
            Right => 0.0f32,
            RightDown => 45.0f32,
            Down => 90.0f32,
            LeftDown => 135.0f32,
            Left => 180.0f32,
            LeftUp => 225.0f32,
            Up => 270.0f32,
            RightUp => 315.0f32,
            None => unreachable!(),
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
            direction: Direction::None,
            opacity: 0.0,
            x: 400.0,
            y: 400.0,
        }
    }
}

struct Enemy {
    pos: Point2,
    start_pos: Point2,
    end_pos: Point2,
    alive: bool,
    time: f32,
}

impl Enemy {
    fn handle_boundaries(&mut self, width: f32, height: f32) {
        if self.pos[0] < 0.0 || self.pos[0] > width
          || self.pos[1] > height || self.pos[1] < 0.0 {
            self.alive = self.time > 1.0;
        }
        
    }

    fn update(&mut self, ctx: &mut Context) {
        self.handle_boundaries(
            ctx.conf.window_mode.width as f32,
            ctx.conf.window_mode.height as f32,
        );
        self.pos = lerp(self.start_pos, self.end_pos, self.time);
        self.time += 0.01;
    }

    fn spawn(width: f32, height: f32, wall: Wall) -> Enemy {
        use Wall::*;
        use rand::thread_rng;
        let (pos_x, end_pos_x) = match wall {
            Left => (0.0, width),
            Right => (width, 0.0),
            Up => (thread_rng().gen_range(0.0, width), thread_rng().gen_range(0.0, width)),
            Down => (thread_rng().gen_range(0.0, width), thread_rng().gen_range(0.0, width)),
        };
        
        let (pos_y, end_pos_y) = match wall {
            Left => (thread_rng().gen_range(0.0, height), thread_rng().gen_range(0.0, height)),
            Right => (thread_rng().gen_range(0.0, height), thread_rng().gen_range(0.0, height)),
            Up => (0.0, height),
            Down => (height, 0.0),
        };

        Enemy {
            pos: Point2::new(pos_x, pos_y),
            start_pos: Point2::new(pos_x, pos_y),
            end_pos: Point2::new(end_pos_x, end_pos_y),
            alive: true,
            time: 0.0,
        }
    }

    fn draw(&self, ctx: &mut Context)-> GameResult<()> {
        graphics::set_color(ctx, RED)?;
        graphics::circle(ctx, DrawMode::Fill, self.pos, 5.0, 2.0)?;
        Ok(())
    }
}

enum Wall {
    Left,
    Right,
    Up,
    Down,
}

impl Wall {
    fn rand() -> Wall {
        use Wall::*;
        match thread_rng().gen_range(0, 4) {
            0 => Left,
            1 => Right,
            2 => Up,
            3 => Down,
            _ => unreachable!()
        }
    }
}

pub fn vector2(a: Point2, b: Point2) -> Vector2 {
    Vector2::new(b[0] - a[0], b[1] - a[1])
}

pub fn lerp(current: Point2, goal: Point2, time: f32) -> Point2 {
    current + (goal - current) * time
}

pub fn distance(a: Point2, b: Point2) -> f32 {
    ((a[0] - b[0]).powf(2.0) + (a[1] - b[1]).powf(2.0)).sqrt()
}

pub fn bpm_to_duration(bpm: f64) -> Duration {
    timer::f64_to_duration(60.0/bpm)
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
