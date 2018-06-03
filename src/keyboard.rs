use std::time::{Duration, Instant};

use ggez::event::Keycode;

use util::Direction8;

// If a key was pressed since however many nanoseconds ago, cound it as having been pressed now
// This allows for
static NANOS_KEYPRESS_TOLERANCE: u32 = 3_000_000; // 3 milliseconds

#[derive(Default, Debug)]
pub struct KeyboardState {
    pub left: Key,
    pub right: Key,
    pub up: Key,
    pub down: Key,
    pub space: Key,
}

impl KeyboardState {
    pub fn update(&mut self, keycode: Keycode, is_down: bool) {
        use Keycode::*;
        match keycode {
            Left => self.left.update(is_down),
            Right => self.right.update(is_down),
            Up => self.up.update(is_down),
            Down => self.down.update(is_down),
            Space => self.space.update(is_down),
            _ => (),
        }
    }

    pub fn direction(&self) -> Result<Direction8, &'static str> {
        let left = self.left.pressed();
        let right = self.right.pressed();
        let up = self.up.pressed();
        let down = self.down.pressed();
        use Direction8::*;
        match (left, right, up, down) {
            (true, false, false, false) => Ok(Left),
            (false, true, false, false) => Ok(Right),
            (false, false, true, false) => Ok(Up),
            (false, false, false, true) => Ok(Down),
            (true, false, true, false) => Ok(LeftUp),
            (true, false, false, true) => Ok(LeftDown),
            (false, true, true, false) => Ok(RightUp),
            (false, true, false, true) => Ok(RightDown),
            _ => Err("Not a direction!"),
        }
    }
}

#[derive(Debug)]
pub struct Key {
    pub is_down: bool,
    last_pressed: Instant,
}

impl Key {
    fn update(&mut self, is_down: bool) {
        self.is_down = is_down;
        if is_down {
            self.last_pressed = Instant::now();
        }
    }

    fn last_pressed(&self) -> Duration {
        self.last_pressed.elapsed()
    }

    pub fn pressed(&self) -> bool {
        self.last_pressed() < Duration::new(0, NANOS_KEYPRESS_TOLERANCE)
    }
}

impl Default for Key {
    fn default() -> Self {
        Key {
            is_down: false,
            last_pressed: Instant::now(),
        }
    }
}
