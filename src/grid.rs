use ggez::graphics::{Point2, MeshBuilder, Mesh, Color};
use ggez::*;

use util::*;

/// The grid that enemies and the player live on.
/// Also has a "glow" effect that is just decorative.
pub struct Grid {
    offset: Point2, // Offset in position from upper right corner
    glow_offset: Point2, 
    grid_spacing: f32,
    grid_size: (usize, usize),
    line_width: f32,
    glow_line_width: f32,
    color: Color,
    glow_color: Color,
}

impl Default for Grid {
    fn default() -> Self {
        Grid {
            offset: Point2::new(15.0f32, 15.0f32),
            glow_offset: Point2::new(14.5f32, 15.5f32),
            grid_spacing: 50.0,
            grid_size: (12, 9),
            line_width: 1.0,
            glow_line_width: 5.0,
            color: WHITE,
            glow_color: TRANSPARENT,
        }
    }
}

impl Grid {
    pub fn update(&mut self, beat_percent: f64) {
        let color = 0.6 + 0.4 * smooth_step(1.0 - beat_percent) as f32;
        self.color = Color::new(color, color, color, 1.0);
        let opacity = 0.05 + 0.6 * smooth_step(1.0 - beat_percent) as f32;
        self.glow_color = Color::new(1.0, 1.0, 1.0, opacity);
        self.glow_line_width = 2.0 + 3.0 * smooth_step(1.0 - beat_percent) as f32;
    }

    pub fn draw(&self, ctx: &mut Context) -> GameResult<()> {
        graphics::set_color(ctx, self.color)?;
        let grid_mesh = self.mesh(ctx, self.line_width)?;
        graphics::draw(ctx, &grid_mesh, self.offset, 0.0)?;
        graphics::set_color(ctx, self.glow_color)?;
        let glow_mesh = self.mesh(ctx, self.glow_line_width)?;
        graphics::draw(ctx, &glow_mesh, self.glow_offset, 0.0)?;
        Ok(())
    }

    fn mesh(&self, ctx: &mut Context, line_width: f32) -> GameResult<Mesh> {
        let mut mb = MeshBuilder::new();
        let max_x = self.grid_spacing * self.grid_size.0 as f32;
        let max_y = self.grid_spacing * self.grid_size.1 as f32;
        for i in 0..self.grid_size.0 {
            mb.line(&[
                Point2::new(self.grid_spacing * i as f32, 0.0),
                Point2::new(self.grid_spacing * i as f32, max_y),
            ], line_width);
        }

        for i in 0..self.grid_size.1 {
            mb.line(&[
                Point2::new(0.0, self.grid_spacing * i as f32),
                Point2::new(max_x, self.grid_spacing * i as f32),
            ], line_width);
        }

        mb.line(&[
            Point2::new(max_x, 0.0),
            Point2::new(max_x, max_y),
        ], line_width);

        mb.line(&[
                Point2::new(0.0, max_y),
                Point2::new(max_x, max_y),
            ], line_width);
        mb.build(ctx)
    }

    pub fn to_screen_coord(&self, grid_coord: (isize, isize)) -> Point2 {
        Point2::new(grid_coord.0 as f32 * self.grid_spacing + self.offset[0], -grid_coord.1 as f32 * self.grid_spacing + self.offset[1])
    }
}