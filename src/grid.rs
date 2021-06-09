use ggez::graphics::DrawParam;
use ggez::graphics::{Color, Mesh, MeshBuilder};
use ggez::*;

use ggez::nalgebra as na;

use util::*;

/// The grid that enemies and the player live on.
/// Also has a "glow" effect that is just decorative.
pub struct Grid {
    offset: na::Point2<f32>, // Offset in position from upper right corner
    glow_offset: na::Point2<f32>,
    grid_spacing: f32,
    pub grid_size: (usize, usize),
    line_width: f32,
    glow_line_width: f32,
    color: Color,
    glow_color: Color,
}

impl Default for Grid {
    fn default() -> Self {
        Grid {
            offset: na::Point2::new(15.0f32, 15.0f32),
            glow_offset: na::Point2::new(14.5f32, 15.5f32),
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
    /// Decorative, makes the glow grid pulse to the music
    pub fn update(&mut self, beat_percent: f64) {
        let color = 0.6 + 0.2 * smooth_step(1.0 - beat_percent) as f32;
        self.color = Color::new(color, color, color, 1.0);
        let opacity = 0.05 + 0.3 * smooth_step(1.0 - beat_percent) as f32;
        self.glow_color = Color::new(1.0, 1.0, 1.0, opacity);
        self.glow_line_width = 2.0 + 1.0 * smooth_step(1.0 - beat_percent) as f32;
    }

    pub fn draw(&self, ctx: &mut Context) -> GameResult<()> {
        let grid_mesh = self.mesh(ctx, self.line_width, self.color)?;
        let glow_mesh = self.mesh(ctx, self.glow_line_width, self.glow_color)?;

        graphics::draw(ctx, &grid_mesh, DrawParam::default().dest(self.offset))?;
        graphics::draw(ctx, &glow_mesh, DrawParam::default().dest(self.glow_offset))?;
        Ok(())
    }

    // Build the grid, returning a nice mesh.
    fn mesh(&self, ctx: &mut Context, line_width: f32, color: Color) -> GameResult<Mesh> {
        // Use a meshbuilder for speed and also ease of doing this.
        let mut mb = MeshBuilder::new();
        let max_x = self.grid_spacing * self.grid_size.0 as f32;
        let max_y = self.grid_spacing * self.grid_size.1 as f32;
        for i in 0..self.grid_size.0 {
            mb.line(
                &[
                    na::Point2::new(self.grid_spacing * i as f32, 0.0),
                    na::Point2::new(self.grid_spacing * i as f32, max_y),
                ],
                line_width,
                color,
            )?;
        }

        for i in 0..self.grid_size.1 {
            mb.line(
                &[
                    na::Point2::new(0.0, self.grid_spacing * i as f32),
                    na::Point2::new(max_x, self.grid_spacing * i as f32),
                ],
                line_width,
                color,
            )?;
        }

        mb.line(
            &[na::Point2::new(max_x, 0.0), na::Point2::new(max_x, max_y)],
            line_width,
            color,
        )?;

        mb.line(
            &[na::Point2::new(0.0, max_y), na::Point2::new(max_x, max_y)],
            line_width,
            color,
        )?;
        mb.build(ctx)
    }

    /// Transform a world-space coordinate into a screen-space coordinate (for drawing)
    pub fn to_screen_coord(&self, grid_point: GridPoint) -> na::Point2<f32> {
        (grid_point.as_point() * self.grid_spacing)
            + na::Vector2::new(self.offset[0], self.offset[1])
    }

    /// Transform a world-space length into a screen-space length (for drawing)
    pub fn to_screen_length(&self, length: f32) -> f32 {
        self.grid_spacing * length
    }
}
