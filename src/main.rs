extern crate ggez;

mod keyboard;

use ggez::event::{Keycode, Mod};
use ggez::graphics::{Color, DrawMode, Point2, Mesh};
use ggez::*;
use keyboard::{KeyboardState, Direction};
const WHITE: Color = Color { r: 1.0, g: 1.0, b: 1.0, a: 1.0 };
const RED: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };

struct MainState {
    pos_x: f32,
    pos_y: f32,
    goal_x: f32,
    goal_y: f32,
    speed: f32,
    arrow: Arrow,
    keyboard: KeyboardState,
}

impl MainState {
    fn new(_ctx: &mut Context) -> GameResult<MainState> {
        let s = MainState::default();
        Ok(s)
    }

    fn handle_boundaries(&mut self, height: f32, width: f32) {
        if self.pos_y > height {
            self.pos_y = height;
        } else if self.pos_y < 0.0 {
            self.pos_y = 0.0;
        }

        if self.pos_x < 0.0 {
            self.pos_x = width;
        } else if self.pos_x > width {
            self.pos_x = 0.0;
        }
    }
}

impl event::EventHandler for MainState {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        self.pos_x = interpolate(self.pos_x, self.goal_x, self.speed);
        self.pos_y = interpolate(self.pos_y, self.goal_y, self.speed);

        self.handle_boundaries(
            ctx.conf.window_mode.height as f32,
            ctx.conf.window_mode.width as f32,
        );

        self.arrow.update();
        self.speed = if self.keyboard.space { 0.01 } else { 0.2 };
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
            Left => self.goal_x += -40.0,
            Right => self.goal_x += 40.0,
            Up => self.goal_y += -40.0,
            Down => self.goal_y += 40.0,
            _ => (),
        }

        self.keyboard.update(keycode, true);
        if let Ok(direction) = Direction::to_direction(&self.keyboard) {
            self.arrow.direction = direction;
            self.arrow.opacity = 1.0;
            println!("{:?}", self.arrow.direction);
        }
    }

    fn key_up_event(&mut self, _ctx: &mut Context, keycode: Keycode, _keymod: Mod, _repeat: bool) {
        self.keyboard.update(keycode, false);
        if let Ok(direction) = Direction::to_direction(&self.keyboard) {
            self.arrow.direction = direction;
            println!("{:?}", self.arrow.direction);
        }
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::clear(ctx);
        graphics::set_color(ctx, WHITE).expect("Couldn't set color");
        graphics::circle(
            ctx,
            DrawMode::Fill,
            Point2::new(self.pos_x, self.pos_y),
            10.0,
            2.0,
        )?;
        graphics::set_color(ctx, RED).expect("Couldn't set color");
        graphics::circle(
            ctx,
            DrawMode::Fill,
            Point2::new(self.goal_x, self.goal_y),
            3.0,
            2.0,
        )?;

        self.arrow.draw(ctx);
        graphics::present(ctx);
        Ok(())
    }
}

impl Default for MainState {
    fn default() -> Self {
        MainState {
            pos_x: 0.0,
            pos_y: 0.0,
            goal_x: 0.0,
            goal_y: 0.0,
            speed: 0.0,
            keyboard: Default::default(),
            arrow: Default::default(),
        }
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
        use DrawMode::*;
        use Direction::*;

        if self.direction == None {
            return
        }

        let prev_color = graphics::get_color(ctx);

        graphics::set_color(ctx, Color::new(1.0, 1.0, 1.0, self.opacity)).expect("Couldn't set color");
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

        let points = [Point2::new(0.0, 0.0), Point2::new(100.0, 0.0), Point2::new(100.0, 10.0), Point2::new(0.0, 10.0)];
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

pub fn interpolate(current: f32, goal: f32, time: f32) -> f32 {
    current + (goal - current) * time
}

pub fn main() {
    let c = conf::Conf::new();
    let ctx = &mut Context::load_from_conf("super_simple", "ggez", c).unwrap();
    let state = &mut MainState::new(ctx).unwrap();
    event::run(ctx, state).unwrap();
}
