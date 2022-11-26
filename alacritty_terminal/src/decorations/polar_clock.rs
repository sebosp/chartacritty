//! The polar clock decoration

use super::nannou::NannouVertices;
use crate::term::SizeInfo;
use chrono::prelude::*;
use chrono::NaiveDate;
use nannou::draw;
use nannou::geom::path::Builder;
use nannou::lyon;
use nannou::prelude::*;
use serde::{Deserialize, Serialize};

// Create a Polar clock that has increasingly more and more opacity, so that the more granular time
// is more easily visible, these can become default and we can read them from the config yaml file
// for other hours, multipliers, etc.
const DAY_OF_YEAR_ALPHA_MULTIPLIER: f32 = 0.30;
const MONTH_OF_YEAR_ALPHA_MULTIPLIER: f32 = 0.05;
const DAY_OF_MONTH_ALPHA_MULTIPLIER: f32 = 0.30;
// For work hours, 9 to 5, show light line
const WORKHOUR_OF_DAY_ALPHA_MULTIPLIER: f32 = 0.20;
// For after-work-hours, show line more visible
const NONWORKHOUR_OF_DAY_ALPHA_MULTIPLIER: f32 = 0.25;
const MINUTE_OF_HOUR_ALPHA_MULTIPLIER: f32 = 0.25;
const SECONDS_WITH_MILLIS_OF_MINUTE_ALPHA_MULTIPLIER: f32 = 0.15;

// The polar clock radius multipliers, similar to teh alpha multiplier, these make the arcs not
// collide. TODO: Right now they depend on the arc stroke_weight to avoid overlap.
const DAY_OF_YEAR_RADIUS_MULTIPLIER: f32 = 1.05;
const MONTH_OF_YEAR_RADIUS_MULTIPLIER: f32 = 0.95;
const DAY_OF_MONTH_RADIUS_MULTIPLIER: f32 = 0.85;
const HOUR_OF_DAY_RADIUS_MULTIPLIER: f32 = 0.75;
const MINUTE_OF_HOUR_RADIUS_MULTIPLIER: f32 = 0.65;
const SECONDS_WITH_MILLIS_OF_MINUTE_RADIUS_MULTIPLIER: f32 = 0.55;

/// Set the default colors for the polar clock
const DAY_OF_YEAR_RGB: Srgb<u8> = PALETURQUOISE;
const MONTH_OF_YEAR_RGB: Srgb<u8> = PALEGOLDENROD;
const DAY_OF_MONTH_RGB: Srgb<u8> = PALETURQUOISE;
// For work hours, 9 to 5, show light line
const WORKHOUR_OF_DAY_RGB: Srgb<u8> = PALEGOLDENROD;
// For after-work-hours, show line more visible
const NONWORKHOUR_OF_DAY_RGB: Srgb<u8> = DARKRED;
const MINUTE_OF_HOUR_RGB: Srgb<u8> = PALETURQUOISE;
const SECONDS_WITH_MILLIS_OF_MINUTE_RGB: Srgb<u8> = PALEGOLDENROD;

const DAY_OF_YEAR_STROKE_WEIGHT: f32 = 8.;
const MONTH_OF_YEAR_STROKE_WEIGHT: f32 = 6.;
const DAY_OF_MONTH_STROKE_WEIGHT: f32 = 6.;
const HOUR_OF_DAY_STROKE_WEIGHT: f32 = 6.;
const MINUTE_OF_HOUR_STROKE_WEIGHT: f32 = 6.;
const SECONDS_WITH_MILLIS_OF_MINUTE_STROKE_WEIGHT: f32 = 6.;

/// Draws the progression arc for a time unit along its domain.
fn build_time_arc_progress(x: f32, y: f32, radius: f32, arc_angles: f32) -> nannou::geom::Path {
    let mut builder = Builder::new().with_svg();
    builder.move_to(lyon::math::point(x, y + radius));
    builder.arc(
        lyon::math::point(x, y),
        lyon::math::vector(radius, radius),
        lyon::math::Angle::degrees(arc_angles),
        lyon::math::Angle::degrees(90.)
    );
    builder.build()
}

/// Draws the whiskers showing time unit significant separators
fn build_time_arc_whisker(x: f32, y: f32, radius: f32, arc_angles: f32) -> nannou::geom::Path {
    let mut builder = Builder::new().with_svg();
    builder.move_to(lyon::math::point(arc_angles.to_radians().cos() * radius + x, arc_angles.to_radians().sin() * radius + y));
    builder.arc(
        lyon::math::point(x, y),
        lyon::math::vector(radius, radius * 1.1),
        lyon::math::Angle::degrees(2f32), // Draw a 2 degrees arc.
        lyon::math::Angle::degrees(arc_angles),
    );
    builder.build()
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PolarClockUnitProperties {
    /// The multiplier of the above `radius` to avoid overlap
    /// This value changes only during config file changes. We can generate a new
    /// `effective_radius` that is the original radius times the multiplier and cache the value.
    radius_multiplier: f32,
    /// The color that we want to use for our arc, the alpha is actually an alpha multiplier, not
    /// the real alpha, this is so that there is a global PolarClockState alpha and different
    /// components have slightly less alpha.
    color: Rgba<f32>,
    /// The stroke weight of the arc
    stroke_weight: f32,
}

impl PolarClockUnitProperties {
    /// Creates the default properties for the day of year arc.
    /// This is the outermost arc.
    fn with_default_day_of_year_props() -> Self {
        let color: Rgb = DAY_OF_YEAR_RGB.into_format::<f32>();
        Self {
            radius_multiplier: DAY_OF_YEAR_RADIUS_MULTIPLIER,
            color: rgba(color.red, color.green, color.blue, DAY_OF_YEAR_ALPHA_MULTIPLIER),
            stroke_weight: DAY_OF_YEAR_STROKE_WEIGHT,
        }
    }

    /// Creates the default properties for the month of year arc.
    /// This is the second to outermost arc.
    fn with_default_month_of_year_props() -> Self {
        let color: Rgb = MONTH_OF_YEAR_RGB.into_format::<f32>();
        Self {
            radius_multiplier: MONTH_OF_YEAR_RADIUS_MULTIPLIER,
            color: rgba(color.red, color.green, color.blue, MONTH_OF_YEAR_ALPHA_MULTIPLIER),
            stroke_weight: MONTH_OF_YEAR_STROKE_WEIGHT,
        }
    }

    /// Creates the default properties for the day of month arc.
    /// This is the third to outermost arc.
    fn with_default_day_of_month_props() -> Self {
        let color: Rgb = DAY_OF_MONTH_RGB.into_format::<f32>();
        Self {
            radius_multiplier: DAY_OF_MONTH_RADIUS_MULTIPLIER,
            color: rgba(color.red, color.green, color.blue, DAY_OF_MONTH_ALPHA_MULTIPLIER),
            stroke_weight: DAY_OF_MONTH_STROKE_WEIGHT,
        }
    }

    /// Creates the default properties for the hour of day arc.
    /// This is the third arc from the center.
    fn with_default_hour_of_day_props() -> Self {
        let color: Rgb = WORKHOUR_OF_DAY_RGB.into_format::<f32>();
        Self {
            radius_multiplier: HOUR_OF_DAY_RADIUS_MULTIPLIER,
            color: rgba(color.red, color.green, color.blue, WORKHOUR_OF_DAY_ALPHA_MULTIPLIER),
            stroke_weight: HOUR_OF_DAY_STROKE_WEIGHT,
        }
    }

    /// Creates the default properties for the minute of hour arc.
    /// This is the second arc from the center.
    fn with_default_minute_of_hour_props() -> Self {
        let color: Rgb = MINUTE_OF_HOUR_RGB.into_format::<f32>();
        Self {
            radius_multiplier: MINUTE_OF_HOUR_RADIUS_MULTIPLIER,
            color: rgba(color.red, color.green, color.blue, MINUTE_OF_HOUR_ALPHA_MULTIPLIER),
            stroke_weight: MINUTE_OF_HOUR_STROKE_WEIGHT,
        }
    }

    /// Creates the default properties for the seconds with millis of minute
    /// This is the second arc from the center.
    fn with_default_seconds_with_millis_of_minute_props() -> Self {
        let color: Rgb = SECONDS_WITH_MILLIS_OF_MINUTE_RGB.into_format::<f32>();
        Self {
            radius_multiplier: SECONDS_WITH_MILLIS_OF_MINUTE_RADIUS_MULTIPLIER,
            color: rgba(
                color.red,
                color.green,
                color.blue,
                SECONDS_WITH_MILLIS_OF_MINUTE_ALPHA_MULTIPLIER,
            ),
            stroke_weight: SECONDS_WITH_MILLIS_OF_MINUTE_STROKE_WEIGHT,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum PolarClockUnit {
    DayOfYear,
    MonthOfYear,
    DayOfMonth,
    HourOfDay,
    MinuteOfHour,
    SecondsWithMillisOfMinute,
}

impl PolarClockUnit {
    /// Returns the default PolarClockUnitProperties for a given PolarClockUnit
    /// This is so that we do not have to configure every single property for every single
    /// arc/dial in the config file.
    pub fn default_props(&self) -> PolarClockUnitProperties {
        match self {
            Self::DayOfYear => PolarClockUnitProperties::with_default_day_of_year_props(),
            Self::MonthOfYear => PolarClockUnitProperties::with_default_month_of_year_props(),
            Self::DayOfMonth => PolarClockUnitProperties::with_default_day_of_month_props(),
            Self::HourOfDay => PolarClockUnitProperties::with_default_hour_of_day_props(),
            Self::MinuteOfHour => PolarClockUnitProperties::with_default_minute_of_hour_props(),
            Self::SecondsWithMillisOfMinute => {
                PolarClockUnitProperties::with_default_seconds_with_millis_of_minute_props()
            },
        }
    }

    /// Returns the whiskers that are useful to understand clearly the time progression across
    /// its domain, i.e. for the month, one tick per month, for hour, one tick every 3 hours, etc.
    pub fn get_unit_whiskers(&self, _input_time: &DateTime<Local>) -> Vec<u32> {
        // TODO: use input_time to return special days of like weekends
        match self {
            Self::DayOfYear => vec![0],
            Self::MonthOfYear => (0..12).collect(),
            Self::DayOfMonth => vec![0, 15],
            Self::HourOfDay => (0..8).map(|x| x * 3).collect(),
            Self::MinuteOfHour => (0..4).map(|x| x * 15).collect(),
            Self::SecondsWithMillisOfMinute => vec![0, 30_000],
        }
    }

    /// Gets the current time unit value
    pub fn get_time_unit_value(&self, input_time: &DateTime<Local>) -> u32 {
        match self {
            Self::DayOfYear => input_time.ordinal(),
            Self::MonthOfYear => input_time.month(),
            Self::DayOfMonth => input_time.day(),
            Self::HourOfDay => input_time.hour(),
            Self::MinuteOfHour => input_time.minute(),
            Self::SecondsWithMillisOfMinute => {
                input_time.second() * 1000u32
                    + (input_time.nanosecond() as f32 / 1_000_000f32).floor() as u32
            },
        }
    }

    /// Returns the max value for the current time unit in a time frame, for example, for a year it
    /// would return the number of days in the current year, or month, or etc.
    pub fn get_time_unit_max_value(&self, input_time: &DateTime<Local>) -> u32 {
        match self {
            Self::DayOfYear => Self::day_of_year_max_value(input_time),
            Self::MonthOfYear => 12,
            Self::DayOfMonth => Self::day_of_month_max_value(input_time),
            Self::HourOfDay => 24,
            Self::MinuteOfHour => 60,
            Self::SecondsWithMillisOfMinute => 60_000u32,
        }
    }

    /// Find the number of days in the current year by getting the first day of the current year
    /// and the first day of the next year and substracting them
    pub fn day_of_year_max_value(input_time: &DateTime<Local>) -> u32 {
        let first_day_of_year = NaiveDate::from_ymd_opt(input_time.year(), 1, 1).unwrap();
        let first_day_of_next_year = NaiveDate::from_ymd_opt(input_time.year() + 1, 1, 1).unwrap();
        first_day_of_next_year.signed_duration_since(first_day_of_year).num_days() as u32
    }

    /// Find the number of days in the current month by getting the first day of the current month
    /// and the first day of the next month and substracting them
    pub fn day_of_month_max_value(input_time: &DateTime<Local>) -> u32 {
        let first_day_of_next_year = NaiveDate::from_ymd_opt(input_time.year() + 1, 1, 1).unwrap();
        let first_day_of_next_month = if input_time.month() == 12 {
            first_day_of_next_year
        } else {
            NaiveDate::from_ymd_opt(input_time.year(), input_time.month() + 1, 1).unwrap()
        };
        let first_day_of_month =
            NaiveDate::from_ymd_opt(input_time.year(), input_time.month(), 1).unwrap();
        first_day_of_next_month.signed_duration_since(first_day_of_month).num_days() as u32
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PolarClockUnitState {
    /// The unit of time
    unit: PolarClockUnit,
    /// The unit of time drawing properties
    props: PolarClockUnitProperties,
    /// The last time this unit was drawn, only re-generate vertices if this unit progresses.
    last_drawn_unit: u32,
    /// The vertices for the current state
    pub vecs: Vec<NannouVertices>,
    /// Whether we should force a vertice re-generation
    is_dirty: bool,
}

impl Default for PolarClockUnitState {
    /// The default function is there only for allowing easy parsing of the config yaml. This
    /// shouldn't be used at all.
    /// The config file parsing needs to be adjusted to receive and parse somehow the
    /// angle/color/stroke_weight/radius_multiplier/etc
    fn default() -> Self {
        let unit = PolarClockUnit::DayOfYear;
        let props = unit.default_props();
        Self { unit, props, last_drawn_unit: 0, vecs: vec![], is_dirty: true }
    }
}

impl PolarClockUnitState {
    /// Creates a new PolarClockUnitState with optionally some properties.
    /// After `new()`, the caller must call `tick()` to populate the vertices
    pub fn new(unit: PolarClockUnit, props: Option<PolarClockUnitProperties>) -> Self {
        let props = match props {
            Some(props) => props,
            None => unit.default_props(),
        };
        Self {
            unit,
            props,
            // This is not important because is_dirty is true and it will
            // overwrite this value the first time we call `tick()`
            last_drawn_unit: 0,
            vecs: vec![],
            is_dirty: true,
        }
    }

    /// Updates the vertices of the arc if needed.
    pub fn tick(
        &mut self,
        tick_time: &DateTime<Local>,
        x: f32,
        y: f32,
        radius: f32,
        size_info: SizeInfo,
        alpha: f32,
    ) {
        let current_tick_unit = self.unit.get_time_unit_value(tick_time);
        if let PolarClockUnit::HourOfDay = &self.unit {
            let (hour_color, hour_alpha) = if (9..17).contains(&current_tick_unit) {
                (WORKHOUR_OF_DAY_RGB.into_format::<f32>(), WORKHOUR_OF_DAY_ALPHA_MULTIPLIER)
            } else {
                (NONWORKHOUR_OF_DAY_RGB.into_format::<f32>(), NONWORKHOUR_OF_DAY_ALPHA_MULTIPLIER)
            };
            self.props.color = rgba(hour_color.red, hour_color.green, hour_color.blue, hour_alpha);
        }
        if self.is_dirty || self.last_drawn_unit != current_tick_unit {
            self.last_drawn_unit = current_tick_unit;
            self.vecs = self.gen_vertices(tick_time, x, y, radius, size_info, alpha);
            self.is_dirty = false;
        }
    }

    /// Creates vertices for the Polar Clock Arc
    fn gen_vertices(
        &self,
        tick_time: &DateTime<Local>,
        x: f32,
        y: f32,
        radius: f32,
        size_info: SizeInfo,
        alpha: f32,
    ) -> Vec<NannouVertices> {
        let draw = draw::Draw::default().triangle_mode();
        let progress_angle = 360f32 * self.unit.get_time_unit_value(tick_time) as f32
            / self.unit.get_time_unit_max_value(tick_time) as f32;
        let mut color = self.props.color;
        color.alpha *= alpha;
        draw.path()
            .stroke()
            .stroke_weight(self.props.stroke_weight)
            .color(color)
            .caps_round()
            .events(
                build_time_arc_progress(x, y, radius * self.props.radius_multiplier, progress_angle).iter(),
            );
        let color = rgba(GOLD.into_format::<f32>().red, GOLD.into_format::<f32>().green, GOLD.into_format::<f32>().blue, alpha * 0.4);
        for whisker in self.unit.get_unit_whiskers(tick_time) {
            let mut whisker_angle = 360f32 * (whisker % self.unit.get_time_unit_max_value(tick_time)) as f32
                / self.unit.get_time_unit_max_value(tick_time) as f32;
            whisker_angle = (whisker_angle + 90f32)% 360f32;
            draw.path()
                .stroke()
                .stroke_weight(2.)
                .color(color)
                .events(
                    build_time_arc_whisker(x, y, radius * self.props.radius_multiplier, whisker_angle).iter(),
                );
        }
        /*draw.tri()
            .points(
                Point2::new(
                    (radius * self.props.radius_multiplier * 0.95) * 90_f32.to_radians().cos() + x,
                    (radius * self.props.radius_multiplier * 0.95) * 90_f32.to_radians().sin() + y
                ),
                Point2::new(
                    ((radius + self.props.stroke_weight) * self.props.radius_multiplier) * 85_f32.to_radians().cos() + x,
                    ((radius + self.props.stroke_weight) * self.props.radius_multiplier) * 85_f32.to_radians().sin() + y
                ),
                Point2::new(
                    ((radius + self.props.stroke_weight) * self.props.radius_multiplier) * 95_f32.to_radians().cos() + x,
                    ((radius + self.props.stroke_weight) * self.props.radius_multiplier) * 95_f32.to_radians().sin() + y
                )
            )
            .color(color);*/
        super::NannouDecoration::gen_vertices_from_nannou_draw(draw, size_info)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PolarClockState {
    pub day_of_year: PolarClockUnitState,
    pub month_of_year: PolarClockUnitState,
    pub day_of_month: PolarClockUnitState,
    pub hour_of_day: PolarClockUnitState,
    pub minute_of_hour: PolarClockUnitState,
    pub seconds_with_millis_of_minute: PolarClockUnitState,
}

impl Default for PolarClockState {
    fn default() -> Self {
        Self {
            day_of_year: PolarClockUnitState::new(PolarClockUnit::DayOfYear, None),
            month_of_year: PolarClockUnitState::new(PolarClockUnit::MonthOfYear, None),
            day_of_month: PolarClockUnitState::new(PolarClockUnit::DayOfMonth, None),
            hour_of_day: PolarClockUnitState::new(PolarClockUnit::HourOfDay, None),
            minute_of_hour: PolarClockUnitState::new(PolarClockUnit::MinuteOfHour, None),
            seconds_with_millis_of_minute: PolarClockUnitState::new(
                PolarClockUnit::SecondsWithMillisOfMinute,
                None,
            ),
        }
    }
}

impl PolarClockState {
    /// Creates a new Polar  Clock State for the given time
    /// After `new()`, the caller must call `tick()` to populate the vertices
    pub fn new(props: Option<PolarClockUnitProperties>) -> Self {
        Self {
            day_of_year: PolarClockUnitState::new(PolarClockUnit::DayOfYear, props.clone()),
            month_of_year: PolarClockUnitState::new(PolarClockUnit::MonthOfYear, props.clone()),
            day_of_month: PolarClockUnitState::new(PolarClockUnit::DayOfMonth, props.clone()),
            hour_of_day: PolarClockUnitState::new(PolarClockUnit::HourOfDay, props.clone()),
            minute_of_hour: PolarClockUnitState::new(PolarClockUnit::MinuteOfHour, props.clone()),
            seconds_with_millis_of_minute: PolarClockUnitState::new(
                PolarClockUnit::SecondsWithMillisOfMinute,
                props,
            ),
        }
    }

    /// Sets the dials as needing a redraw. This should be called after we resize/etc.
    pub fn mark_as_dirty(&mut self) {
        self.day_of_year.is_dirty = true;
        self.month_of_year.is_dirty = true;
        self.day_of_month.is_dirty = true;
        self.hour_of_day.is_dirty = true;
        self.minute_of_hour.is_dirty = true;
        self.seconds_with_millis_of_minute.is_dirty = true;
    }

    /// Calculates the vertices of the polar clock if needed.
    pub fn tick(
        &mut self,
        tick_time: &DateTime<Local>,
        x: f32,
        y: f32,
        radius: f32,
        size_info: SizeInfo,
        alpha: f32,
    ) {
        self.day_of_year.tick(tick_time, x, y, radius, size_info, alpha);
        self.month_of_year.tick(tick_time, x, y, radius, size_info, alpha);
        self.day_of_month.tick(tick_time, x, y, radius, size_info, alpha);
        self.hour_of_day.tick(tick_time, x, y, radius, size_info, alpha);
        self.minute_of_hour.tick(tick_time, x, y, radius, size_info, alpha);
        self.seconds_with_millis_of_minute.tick(tick_time, x, y, radius, size_info, alpha);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn returns_seconds_with_millis() {
        let seconds_with_millis = PolarClockUnit::SecondsWithMillisOfMinute;
        let timezone_west = FixedOffset::west_opt(7 * 60 * 60).unwrap();
        let naivedatetime_west = NaiveDate::from_ymd_opt(2000, 1, 11).unwrap()
            .and_hms_milli_opt(12, 34, 56, 789).unwrap();
        let dt = DateTime::<Local>::from_local(naivedatetime_west, timezone_west);
        assert_eq!(seconds_with_millis.get_time_unit_value(&dt), 56_789);
        let day_of_year = PolarClockUnit::DayOfYear;
        assert_eq!(day_of_year.get_time_unit_value(&dt), 11);
        assert_eq!(day_of_year.get_time_unit_max_value(&dt), 366);
    }
}
