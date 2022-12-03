//! Moon Phase Nannou decoration

use std::time::SystemTime;
use moon_phase::MoonPhase;
use nannou::draw;
use nannou::geom::path::Builder;
use nannou::prelude::*;
use serde::{Deserialize, Serialize};
use crate::term::SizeInfo;

use super::nannou::NannouVertices;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MoonPhaseState{
    /// The MoonPhase state.
    #[serde(skip, default = "current_moon_state")]
    moon_phase: MoonPhase,
    /// The radius of the moon shown on the screen
    radius: f32,
    /// The vertices for the current state
    pub vecs: Vec<NannouVertices>,
}

impl Default for MoonPhaseState {
    fn default() -> Self {
        Self {
            moon_phase: current_moon_state(),
            radius: 0.,
            vecs: vec![],
        }
    }
}

fn current_moon_state() -> MoonPhase {
    MoonPhase::new(SystemTime::now())
}

impl PartialEq for MoonPhaseState {
    fn eq(&self, other: &Self) -> bool {
        self.radius == other.radius && self.vecs == other.vecs
    }

}

fn build_moon_arc_fraction(x: f32, y: f32, radius: f32, fraction: f32) -> nannou::geom::Path {
    // It seems the fraction is inverted?
    let fraction = 1. - fraction;
    let mut builder = Builder::new().with_svg();
    // Start from the top
    builder.move_to(lyon::math::point(x, y + radius));
    // For some reason I have to multiply the control point's x for 1.33 to get a shape similar to
    // a circle... I'm kindof trying to build half a circle with bezier curves... Maybe not the
    // right way.
    builder.cubic_bezier_to(
        lyon::math::point(x + radius * 1.33, y + radius),
        lyon::math::point(x + radius * 1.33, y - radius),
        lyon::math::point(x, y - radius),
    );
    builder.cubic_bezier_to(
        lyon::math::point(x + radius * 1.33 - radius * (fraction * 2.) * 1.33, y - radius),
        lyon::math::point(x + radius * 1.33 - radius * (fraction * 2.) * 1.33, y + radius),
        lyon::math::point(x, y + radius),
    );
    builder.build()
}
impl MoonPhaseState {
    /// Creates a new MoonPhaseState.
    /// After `new()`, the caller must call `tick()` to populate the vertices
    pub fn new(radius: f32) -> Self {
        Self {
            moon_phase: current_moon_state(),
            radius,
            vecs: vec![],
        }
    }

    /// Updates the vertices of the moon if needed.
    pub fn tick(
        &mut self,
        x: f32,
        y: f32,
        radius: f32,
        size_info: SizeInfo,
    ) {
        // Update the MoonPhase
        self.moon_phase = current_moon_state();
        self.radius = radius;
        self.vecs = self.gen_vertices(x, y, size_info);
    }

    /// Creates vertices for the Polar Clock Arc
    fn gen_vertices(
        &self,
        x: f32,
        y: f32,
        size_info: SizeInfo,
    ) -> Vec<NannouVertices> {
        //log::info!("MoonPhase::gen_vertices_from_nannou_draw radius {}", self.radius);
        let draw = draw::Draw::default().triangle_mode();
        let ellipse_color = LIGHTSKYBLUE.into_format::<f32>();
        let ellipse_stroke_color =
            rgba(ellipse_color.red, ellipse_color.green, ellipse_color.blue, 0.01f32);
        let x_60_degrees_offset = super::COS_60 * self.radius;
        let y_60_degrees_offset = super::SIN_60 * self.radius;
        let alpha = 0.07;
        draw.ellipse()
            .x_y(x + x_60_degrees_offset, y + y_60_degrees_offset)
            .radius(self.radius * 0.4)
            .stroke(ellipse_stroke_color)
            .rgba(ellipse_color.red, ellipse_color.green, ellipse_color.blue, alpha);
        draw.path()
            .fill()
            .rgba(ellipse_color.red, ellipse_color.green, ellipse_color.blue, alpha * 2.)
            .events(
                build_moon_arc_fraction(x + x_60_degrees_offset, y + y_60_degrees_offset, self.radius * 0.4, self.moon_phase.fraction as f32).iter(),
            );
        super::NannouDecoration::gen_vertices_from_nannou_draw(draw, size_info)
    }


}
