extern crate ggez;

mod keyboard;

use ggez::event::{Keycode, Mod};
use ggez::graphics::{Color, DrawMode, Mesh, Point2};
use ggez::audio::{Source};
use ggez::*;
use std::collections::VecDeque;
use std::time::Duration;
use std::path::PathBuf;
use std::env;

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
    pos: Point2,
    goal: Point2,
    keyframes: VecDeque<Point2>,
    speed: f32,
    arrow: Arrow,
    keyboard: KeyboardState,
    time: Duration,
    background: Color,
    bpm: Duration,
    music: Source
}

impl MainState {
    fn new(ctx: &mut Context) -> GameResult<MainState> {
        let s = MainState {
            pos: Point2::new(0.0, 0.0),
            goal: Point2::new(0.0, 0.0),
            keyframes: VecDeque::new(),
            speed: 0.0,
            keyboard: Default::default(),
            arrow: Default::default(),
            time: Duration::new(0, 0),
            background: Color::new(0.0, 0.0, 0.0, 1.0),
            bpm: bpm_to_duration(BPM),
            music: audio::Source::new(ctx, MUSIC_PATH)?,
        };
        s.music.play();
        Ok(s)
    }

    fn beat(&mut self, _ctx: &mut Context) {
        print!("Beat!");
    }

    fn interbeat_update(&mut self, time_in_beat: Duration) {
        fn rev_quad(n: f64) -> f64 {
            (1.0 - n) * (1.0 - n)
        }
        let beat_percent = timer::duration_to_f64(time_in_beat)/timer::duration_to_f64(self.bpm);
        let color = rev_quad(beat_percent) as f32;
        self.background = Color::new(color, color, color, 1.0);
    }

    fn handle_boundaries(&mut self, height: f32, width: f32) {
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
}

impl event::EventHandler for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        if let Some(goal) = self.keyframes.pop_front() {
            let speed = (self.speed * (self.keyframes.len() + 1) as f32).min(1.0);
            self.pos = lerp(self.pos, goal, speed);
            if distance(self.pos, goal) > 0.01 {
                self.keyframes.push_front(goal);
            }
        }

        self.handle_boundaries(
            ctx.conf.window_mode.height as f32,
            ctx.conf.window_mode.width as f32,
        );

        self.arrow.update();
        self.speed = if self.keyboard.space { 0.01 } else { 0.2 };

        let time_in_beat = timer::get_time_since_start(ctx) - self.time;
        self.interbeat_update(time_in_beat);
        if time_in_beat > self.bpm {
            self.beat(ctx);
            self.time = timer::get_time_since_start(ctx);
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
            Left => self.goal[0] += -40.0,
            Right => self.goal[0] += 40.0,
            Up => self.goal[1] += -40.0,
            Down => self.goal[1] += 40.0,
            P => drop(self.music.resume()),
            S => drop(self.music.pause()),
            _ => (),
        }

        self.keyboard.update(keycode, true);
        if let Ok(direction) = Direction::from_keyboard(&self.keyboard) {
            self.arrow.direction = direction;
            self.arrow.opacity = 1.0;
            println!("{:?}", self.arrow.direction);
            self.keyframes.push_back(self.goal.clone());
        }
    }

    fn key_up_event(&mut self, _ctx: &mut Context, keycode: Keycode, _keymod: Mod, _repeat: bool) {
        self.keyboard.update(keycode, false);
        if let Ok(direction) = Direction::from_keyboard(&self.keyboard) {
            self.arrow.direction = direction;
            println!("{:?}", self.arrow.direction);
        }
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::clear(ctx);
        graphics::set_background_color(ctx, self.background);
        graphics::set_color(ctx, WHITE).expect("Couldn't set color");
        graphics::circle(ctx, DrawMode::Fill, self.pos, 10.0, 2.0)?;
        graphics::set_color(ctx, RED).expect("Couldn't set color");
        graphics::circle(ctx, DrawMode::Fill, self.goal, 3.0, 2.0)?;

        self.arrow.draw(ctx);
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

    fn draw(&self, ctx: &mut Context) {
        use Direction::*;
        use DrawMode::*;

        if self.direction == None {
            return;
        }

        let prev_color = graphics::get_color(ctx);

        graphics::set_color(ctx, Color::new(1.0, 1.0, 1.0, self.opacity))
            .expect("Couldn't set color");
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
        let rect = Mesh::new_polygon(ctx, Fill, &points).expect("Couldn't male rectangle");
        graphics::draw(ctx, &rect, Point2::new(self.x, self.y), angle).expect("Couldn't draw");
        graphics::set_color(ctx, prev_color).expect("Couldn't set color");
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
