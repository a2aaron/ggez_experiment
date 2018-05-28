extern crate ggez;
use ggez::event::{Keycode, Mod};
use ggez::graphics::{DrawMode, Point2, Rect, Color};
use ggez::*;

struct MainState {
    pos_x: f32,
    pos_y: f32,
    vel_x: f32,
    vel_y: f32,
    arrow: Arrow,
    keyboard: KeyboardState,
}

impl Default for MainState {
    fn default() -> Self {
        MainState {
            pos_x: 0.0,
            pos_y: 0.0,
            vel_x: 0.0,
            vel_y: 0.0,
            keyboard: Default::default(),
            arrow: Default::default(),
        }
    }
}

struct Arrow {
    direction: Keycode,
    opacity: f32,
    x: f32,
    y: f32
}

impl Arrow {
    fn update(&mut self) {
        self.opacity -= 0.05;
        if self.opacity < 0.0 {
            self.opacity = 0.0
        }
    }

    fn draw(&self, ctx: &mut Context) {
        use Keycode::*;
        use DrawMode::*;
        graphics::set_color(ctx, Color::new(1.0, 1.0, 1.0, self.opacity));

        match self.direction {
            Left => drop(graphics::rectangle(ctx, Fill, Rect::new(self.x, self.y, -100.0, 10.0))),
            Right => drop(graphics::rectangle(ctx, Fill, Rect::new(self.x, self.y, 100.0, 10.0))),
            Down => drop(graphics::rectangle(ctx, Fill, Rect::new(self.x, self.y, 10.0, 100.0))),
            Up => drop(graphics::rectangle(ctx, Fill, Rect::new(self.x, self.y, 10.0, -100.0))),
            _ => ()
        }
    }
}

impl Default for Arrow {
    fn default() -> Self {
        Arrow {
            direction: Keycode::Left,
            opacity: 0.0,
            x: 400.0,
            y: 400.0,
        }
    }
}

struct KeyboardState {
    left: bool,
    right: bool,
    up: bool,
    down: bool,
    space: bool,
}

impl KeyboardState {
    fn update(&mut self, keycode: Keycode, is_down: bool) {
        use Keycode::*;
        match keycode {
            Left => self.left = is_down,
            Right => self.right = is_down,
            Up => self.up = is_down,
            Down => self.down = is_down,
            Space => self.space = is_down,
            _ => (),
        }
    }
}

impl Default for KeyboardState {
    fn default() -> Self {
        KeyboardState {
            left: false,
            right: false,
            up: false,
            down: false,
            space: false,
        }
    }
}

impl MainState {
    fn new(_ctx: &mut Context) -> GameResult<MainState> {
        let s = MainState::default();
        Ok(s)
    }

    fn handle_boundaries(&mut self, height: f32, width: f32) {
        if self.pos_y > height {
            self.pos_y = height;
            self.vel_y = 0.0;
        } else if self.pos_y < 0.0 {
            self.pos_y = 0.0;
            self.vel_y = 0.0;
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
        self.pos_x += self.vel_x;
        self.pos_y -= self.vel_y;

        self.vel_y -= 2.0;
        self.vel_x /= 1.2;

        match (self.keyboard.left, self.keyboard.right) {
            (true, true) | (false, false) => (),
            (true, false) => self.vel_x = -10.0,
            (false, true) => self.vel_x = 10.0, 
        }

        self.handle_boundaries(ctx.conf.window_mode.height as f32, ctx.conf.window_mode.width as f32);

        self.arrow.update();
        Ok(())
    }

    fn key_down_event(&mut self, _ctx: &mut Context, keycode: Keycode, _keymod: Mod, _repeat: bool) {
        self.keyboard.update(keycode, true);
        if keycode == Keycode::Space {
            self.vel_y = 25.0;
        }

        self.arrow.direction = keycode;
        self.arrow.opacity = 1.0;
    }

    fn key_up_event(&mut self, _ctx: &mut Context, keycode: Keycode, _keymod: Mod, _repeat: bool) {
        self.keyboard.update(keycode, false);
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        graphics::clear(ctx);
        graphics::circle(
            ctx,
            DrawMode::Fill,
            Point2::new(self.pos_x, self.pos_y),
            10.0,
            2.0,
        )?;
        self.arrow.draw(ctx);
        graphics::present(ctx);
        Ok(())
    }
}

pub fn main() {
    let c = conf::Conf::new();
    let ctx = &mut Context::load_from_conf("super_simple", "ggez", c).unwrap();
    let state = &mut MainState::new(ctx).unwrap();
    event::run(ctx, state).unwrap();
}
