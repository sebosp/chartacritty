//! Moon Phase Nannou decoration

use crate::term::SizeInfo;
use lyon::path::Path;
use lyon::tessellation::*;
use moon_phase::MoonPhase;
use palette::named::*;
use palette::rgb::Rgba;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MoonPhaseState {
    /// The MoonPhase state.
    #[serde(skip, default = "current_moon_state")]
    moon_phase: MoonPhase,
    /// The radius of the moon shown on the screen
    radius: f32,
    /// The vertices for the current state
    pub vecs: Vec<f32>,
    /// Keep track of the last time the vertices needed to be calculated.
    /// This should only happen once a day.
    #[serde(skip, default = "current_system_time")]
    pub last_drawn_time: SystemTime,
    /// If redrawing is required
    is_dirty: bool,
}

impl Default for MoonPhaseState {
    fn default() -> Self {
        Self {
            moon_phase: current_moon_state(),
            radius: 0.,
            vecs: vec![],
            last_drawn_time: SystemTime::now(),
            is_dirty: true,
        }
    }
}

fn current_moon_state() -> MoonPhase {
    get_moon_phase_for_date(SystemTime::now())
}

fn current_system_time() -> SystemTime {
    SystemTime::now()
}

fn get_moon_phase_for_date(time: SystemTime) -> MoonPhase {
    MoonPhase::new(time)
}

impl PartialEq for MoonPhaseState {
    fn eq(&self, other: &Self) -> bool {
        self.radius == other.radius && self.vecs == other.vecs
    }
}

impl MoonPhaseState {
    /// Creates a new MoonPhaseState.
    /// After `new()`, the caller must call `tick()` to populate the vertices
    pub fn new(radius: f32) -> Self {
        let time = SystemTime::now();
        Self {
            moon_phase: get_moon_phase_for_date(time),
            radius,
            vecs: vec![],
            last_drawn_time: time,
            is_dirty: true,
        }
    }

    /// Updates the vertices of the moon if needed.
    pub fn tick(&mut self, x: f32, y: f32, radius: f32, size_info: SizeInfo) {
        // Update the MoonPhase
        self.moon_phase = current_moon_state();
        self.radius = radius;
        if let Ok(elapsed) = self.last_drawn_time.elapsed() {
            // Recalculate the moon phase once a day
            if elapsed > std::time::Duration::from_secs(24 * 60 * 60) {
                self.is_dirty = true;
            }
        }
        if self.is_dirty {
            self.vecs = self.gen_vertices(x, y, size_info);
            self.is_dirty = false;
        }
    }

    /// Creates vertices for the Polar Clock Arc
    fn gen_vertices(&self, x: f32, y: f32, size_info: SizeInfo) -> Vec<f32> {
        log::info!("MoonPhase::gen_vertices, phase: {:?}", self.moon_phase);
        let ellipse_color = LIGHTSKYBLUE.into_format::<f32>();
        let ellipse_color =
            Rgba::new(ellipse_color.red, ellipse_color.green, ellipse_color.blue, 0.002f32);
        let x_60_degrees_offset = super::COS_60 * self.radius;
        let y_60_degrees_offset = super::SIN_60 * self.radius;
        let mut builder = Path::builder().with_svg();
        // TODO: Add the ellipse stroke.

        // phase 0.5 is full
        let illuminated_percent = 1. - ((self.moon_phase.phase as f32 - 0.5).abs() * 2.);
        let moon_fraction_x = x + x_60_degrees_offset;
        let moon_fraction_y = y + y_60_degrees_offset;
        let moon_fraction_radius = self.radius * 0.4;
        // Start from the top
        builder.move_to(lyon::math::point(moon_fraction_x, moon_fraction_y + moon_fraction_radius));
        // For some reason I have to multiply the control point's x for 1.33 to get a shape similar to
        // a circle... I'm kindof trying to build half a circle with bezier curves... Maybe not the
        // right way.
        builder.cubic_bezier_to(
            lyon::math::point(
                moon_fraction_x + moon_fraction_radius * 1.33,
                moon_fraction_y + moon_fraction_radius,
            ),
            lyon::math::point(
                moon_fraction_x + moon_fraction_radius * 1.33,
                moon_fraction_y - moon_fraction_radius,
            ),
            lyon::math::point(moon_fraction_x, moon_fraction_y - moon_fraction_radius),
        );
        builder.cubic_bezier_to(
            lyon::math::point(
                moon_fraction_x + moon_fraction_radius * 1.33
                    - moon_fraction_radius * (illuminated_percent * 2.) * 1.33,
                moon_fraction_y - moon_fraction_radius,
            ),
            lyon::math::point(
                moon_fraction_x + moon_fraction_radius * 1.33
                    - moon_fraction_radius * (illuminated_percent * 2.) * 1.33,
                moon_fraction_y + moon_fraction_radius,
            ),
            lyon::math::point(moon_fraction_x, moon_fraction_y + moon_fraction_radius),
        );
        builder.close();
        let path = builder.build();
        super::LyonDecoration::gen_vertices_from_lyon_path(&path, size_info, ellipse_color)
    }

    pub fn mark_as_dirty(&mut self) {
        self.is_dirty = true;
    }
}
