use ggez::mint;

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Direction8 {
    Left,
    Right,
    Up,
    Down,
    LeftUp,
    LeftDown,
    RightUp,
    RightDown,
}

pub fn into_mint<T>(point: cgmath::Point2<T>) -> mint::Point2<T> {
    mint::Point2 {
        x: point.x,
        y: point.y,
    }
}

pub fn mint<T>(x: T, y: T) -> mint::Point2<T> {
    mint::Point2 { x, y }
}

pub fn into_cg<T>(point: mint::Point2<T>) -> cgmath::Point2<T> {
    cgmath::Point2::new(point.x, point.y)
}

pub fn quartic(n: f64) -> f64 {
    n * n * n * n
}

pub fn rev_quartic(n: f64) -> f64 {
    1.0 - quartic(1.0 - n)
}
