use std::time::Instant;

use ggez::event::KeyCode;

use crate::util::Direction8;

/// Remembers the press state of the key since the last frame.
/// Maybe should be hashmap?
#[derive(Default, Debug)]
pub struct KeyboardState {
    pub left: Key,
    pub right: Key,
    pub up: Key,
    pub down: Key,
    pub space: Key,
}

impl KeyboardState {
    pub fn update(&mut self, keycode: KeyCode, is_down: bool) {
        use KeyCode::*;
        match keycode {
            Left | A => self.left.update(is_down),
            Right | D => self.right.update(is_down),
            Up | W => self.up.update(is_down),
            Down | S => self.down.update(is_down),
            Space => self.space.update(is_down),
            _ => (),
        }
    }
    /// Return the direction based on the current state.
    /// Supports diagonal directions.
    pub fn direction(&self) -> Result<Direction8, &'static str> {
        let left = self.left.is_down;
        let right = self.right.is_down;
        let up = self.up.is_down;
        let down = self.down.is_down;
        match (left, right, up, down) {
            (true, false, false, false) => Ok(Direction8::Left),
            (false, true, false, false) => Ok(Direction8::Right),
            (false, false, true, false) => Ok(Direction8::Up),
            (false, false, false, true) => Ok(Direction8::Down),
            (true, false, true, false) => Ok(Direction8::LeftUp),
            (true, false, false, true) => Ok(Direction8::LeftDown),
            (false, true, true, false) => Ok(Direction8::RightUp),
            (false, true, false, true) => Ok(Direction8::RightDown),
            (true, false, true, true) => Ok(Direction8::Left),
            (false, true, true, true) => Ok(Direction8::Right),
            (true, true, true, false) => Ok(Direction8::Up),
            (true, true, false, true) => Ok(Direction8::Down),
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
}

impl Default for Key {
    fn default() -> Self {
        Key {
            is_down: false,
            last_pressed: Instant::now(),
        }
    }
}
