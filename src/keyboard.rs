use ggez::event::Keycode;

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
    LeftUp,
    LeftDown,
    RightUp,
    RightDown,
    None,
}

impl Direction {
    pub fn from_keyboard(keyboard: &KeyboardState) -> Result<Direction, &'static str> {
        match (keyboard.left, keyboard.right, keyboard.up, keyboard.down, ) {
            (true, false, false, false) => Ok(Direction::Left),
            (false, true, false, false) => Ok(Direction::Right),
            (false, false, true, false) => Ok(Direction::Up),
            (false, false, false, true) => Ok(Direction::Down),
            (true, false, true, false) => Ok(Direction::LeftUp),
            (true, false, false, true) => Ok(Direction::LeftDown),
            (false, true, true, false) => Ok(Direction::RightUp),
            (false, true, false, true) => Ok(Direction::RightDown),
            _ => Err("Not a direction!")
        }
    }
}

pub struct KeyboardState {
    pub left: bool,
    pub right: bool,
    pub up: bool,
    pub down: bool,
    pub space: bool,
}

impl KeyboardState {
    pub fn update(&mut self, keycode: Keycode, is_down: bool) {
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