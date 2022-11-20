//! The polar clock decoration

use chrono::prelude::*;
use chrono::NaiveDate;

use nannou::geom::path::Builder;
use nannou::glam::Vec2;
use super::nannou::NannouVertices;
use nannou::lyon;
use nannou::prelude::*;
use nannou::draw::Draw;
use serde::{Deserialize, Serialize};

// Create a Polar clock that has increasingly more and more opacity, so that the more granular time
// is more easily visible, these can become default and we can read them from the config yaml file
// for other hours, multipliers, etc.
const DAY_OF_YEAR_ALPHA_MULTIPLIER: f32 = 0.25;
const MONTH_OF_YEAR_ALPHA_MULTIPLIER: f32 = 0.35;
const DAY_OF_MONTH_ALPHA_MULTIPLIER: f32 = 0.45;
// For work hours, 9 to 5, show light line
const WORKHOUR_OF_DAY_ALPHA_MULTIPLIER: f32 = 0.65;
// For after-work-hours, show line more visible
const NONWORKHOUR_OF_DAY_ALPHA_MULTIPLIER: f32 = 1.25;
const MINUTE_OF_HOUR_ALPHA_MULTIPLIER: f32 = 0.75;
const SECOND_OF_MINUTE_ALPHA_MULTIPLIER: f32 = 0.85;

// The polar clock radius multipliers, similar to teh alpha multiplier, these make the arcs not
// collide. TODO: Right now they depend on the arc stroke_weight to avoid overlap.
const DAY_OF_YEAR_RADIUS_MULTIPLIER: f32 = 1.05;
const MONTH_OF_YEAR_RADIUS_MULTIPLIER: f32 = 0.95;
const DAY_OF_MONTH_RADIUS_MULTIPLIER: f32 = 0.85;
const HOUR_OF_DAY_RADIUS_MULTIPLIER: f32 = 0.75;
const MINUTE_OF_HOUR_RADIUS_MULTIPLIER: f32 = 0.65;
const SECOND_OF_MINUTE_RADIUS_MULTIPLIER: f32 = 0.55;

/// Set the default colors for the polar clock
const DAY_OF_YEAR_RGB: Rgb = GRAY.into_format::<f32>();
const MONTH_OF_YEAR_RGB: Rgb = LIGHTBLUE.into_format::<f32>();
const DAY_OF_MONTH_RGB: Rgb = GRAY.into_format::<f32>();
// For work hours, 9 to 5, show light line
const WORKHOUR_OF_DAY_RGB: Rgb = LIGHTBLUE.into_format::<f32>();
// For after-work-hours, show line more visible
const NONWORKHOUR_OF_DAY_RGB: Rgb = DARKRED.into_format::<f32>();
const MINUTE_OF_HOUR_RGB: Rgb = GRAY.into_format::<f32>();
const SECOND_OF_MINUTE_RGB: Rgb = AQUA.into_format::<f32>();

const DAY_OF_YEAR_STROKE_WEIGHT: f32 = 12.;
const MONTH_OF_YEAR_STROKE_WEIGHT: f32 = 0.95;
const DAY_OF_MONTH_STROKE_WEIGHT: f32 = 0.85;
const HOUR_OF_DAY_STROKE_WEIGHT: f32 = 0.75;
const MINUTE_OF_HOUR_STROKE_WEIGHT: f32 = 0.65;
const SECOND_OF_MINUTE_STROKE_WEIGHT: f32 = 0.55;

fn build_time_arc(x: f32, y: f32, radius: f32, arc_angles: f32) -> nannou::geom::Path {
    let mut builder = Builder::new().with_svg();
    builder.move_to(lyon::math::point(x + radius, y));
    builder.arc(
        lyon::math::point(x, y),
        lyon::math::vector(radius, radius),
        lyon::math::Angle::degrees(arc_angles),
        lyon::math::Angle::radians(0.0),
    );
    builder.build()
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PolarClockUnit {
    /// The x coordinate center of the clock
    pub x: f32,
    /// The y coordinate center of the clock
    pub y: f32,
    /// The color that we want to use for our arc
    pub radius: f32,
    /// The multiplier of the above `radius` to avoid overlap
    pub radius_multiplier: f32,
    /// The last time this unit was drawn, only re-generate vertices if this unit progresses.
    pub color: Rgba<f32>,
    /// The stroke weight of the arc
    pub stroke_weight: f32,
    /// The radius of the clock.
    pub last_drawn_unit: u32,
    /// The vertices for the current state
    pub vecs: Vec<NannouVertices>,
    /// Whether we should force a vertice re-generation
    pub is_dirty: bool,
}

type DayOfYear = PolarClockUnit;
type MonthOfYear = PolarClockUnit;
type DayOfMonth = PolarClockUnit;
type HourOfDay = PolarClockUnit;
type MinuteOfHour = PolarClockUnit;
type SecondOfMinute = PolarClockUnit;

impl Default for DayOfYear {
    fn default() -> Self {
        Self {
            x: 100f32,
            y: 100f32,
            radius: 100f32,
            radius_multiplier: DAY_OF_MONTH_RADIUS_MULTIPLIER,
            color: rgba(DAY_OF_YEAR_RGB.red, DAY_OF_YEAR_RGB.green, DAY_OF_YEAR_RGB.blue, DAY_OF_YEAR_ALPHA_MULTIPLIER),
            stroke_weight: DAY_OF_YEAR_STROKE_WEIGHT,
            last_drawn_unit: 0,
            vecs: vec![],
            is_dirty: true,
        }
    }
}

impl DayOfYear {
    pub fn new(now: &DateTime<Local>) -> Self {
        let mut res = Self::default();
        res.tick(now);
        res
   }

    /// Updates the vertices for the
    pub fn tick(&mut self, tick_time: &DateTime<Local>) {
        let current_tick_unit = tick_time.ordinal();
        if self.is_dirty || self.last_drawn_unit != current_tick_unit {
            self.last_drawn_unit = tick_time.ordinal();
            self.vecs = self.gen_vertices();
            self.is_dirty = false;
        }
    }

    /// Creates vertices for the Polar Clock day of year.
    fn gen_vertices(&self) -> Vec<NannouVertices> {
        let draw = draw::Draw::default().triangle_mode();
        // Find the number of days in the current year by getting the first day of the current year
        // and the first day of the next year.
        let first_day_of_year = NaiveDate::from_ymd_opt(self.now.year(), 1, 1).unwrap();
        let first_day_of_next_year = NaiveDate::from_ymd_opt(self.now.year() + 1, 1, 1).unwrap();
        let days_in_year =
            first_day_of_year.signed_duration_since(first_day_of_next_year).num_days();
        let day_in_year_angle = 360f32 * self.now.ordinal() as f32 / days_in_year as f32;

        self.draw_arc_path(&draw,
            self.radius * DAY_OF_YEAR_RADIUS_MULTIPLIER,
            day_in_year_angle, year_arc_rgba, 12.);
        draw.path()
            .stroke()
            .stroke_weight(self.stroke_weight)
            .color(self.color)
            .caps_round()
            .events(build_time_arc(self.x, self.y, self.radius, day_in_year_angle).iter());
        super::NannouDecoration::gen_vertices_from_nannou_draw(draw)
    }
}


#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Default)]
pub struct PolarClockState {
    pub day_of_year: DayOfYear,
    pub month_of_year: MonthOfYear,
    pub day_of_month: DayOfMonth,
    pub hour_of_day: HourOfDay,
    pub minute_of_hour: MinuteOfHour,
    pub second_of_minute: SecondOfMinute,
}

impl PolarClockState {
    pub fn new(now: &DateTime<Local>) -> Self {
        Self {
            day_of_year: DayOfYear::new(now),
        }
    }

    pub tick(&mut self) {
        self.day_of_year.tick();
    }
}
