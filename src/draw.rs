use ggez::{
    graphics::{self, Color, DrawParam, Drawable, Mesh, Point2},
    Context, GameResult,
};
pub const SCREEN_WIDTH: f32 = 640.0;
pub const SCREEN_HEIGHT: f32 = 480.0;

pub fn draw_ex(ctx: &mut Context, drawable: &Drawable, param: DrawParam) -> GameResult<()> {
    let new_x = param.dest.x;
    let new_y = SCREEN_HEIGHT - param.dest.y;
    let param = DrawParam {
        dest: Point2::new(new_x, new_y),
        ..param
    };

    graphics::draw_ex(ctx, drawable, param)
}

pub fn draw(ctx: &mut Context, drawable: &Drawable, dest: Point2, rotation: f32) -> GameResult<()> {
    let param = DrawParam {
        dest,
        rotation,
        ..Default::default()
    };
    draw_ex(ctx, drawable, param)
}

pub const WHITE: Color = Color {
    r: 1.0,
    g: 1.0,
    b: 1.0,
    a: 1.0,
};

pub const RED: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

pub const GREEN: Color = Color {
    r: 0.0,
    g: 1.0,
    b: 0.0,
    a: 1.0,
};

pub const TRANSPARENT: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 0.0,
};

pub const DEBUG_RED: Color = Color {
    r: 1.0,
    g: 0.1,
    b: 0.1,
    a: 1.0,
};
