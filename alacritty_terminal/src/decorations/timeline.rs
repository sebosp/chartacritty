//! A faint timeline along the top of the window, from 12 hours ago on the left to 12 hours from
//! now on the right, with a small triangle pointing down at the current time in the middle.
//!
//! It shows the daylight, brighter for the sunlight left today, the cloud cover and the rain
//! at the location.

use super::nature::Triangles;
use super::sun::{Location, SunDay, sun_day};
use super::weather::Forecast;
use crate::term::SizeInfo;
use bevy::math::Vec2;
use chrono::{DateTime, Duration, Timelike, Utc};

/// Seconds shown on each side of the current time.
const HALF_SPAN: i64 = 12 * 3600;

/// Twilight drawn before the sunrise and after the sunset, in seconds.
const TWILIGHT: i64 = 30 * 60;

const NIGHT: [f32; 3] = [0.45, 0.5, 0.65];
const NIGHT_ALPHA: f32 = 0.06;
const SUN: [f32; 3] = [1., 0.78, 0.35];
const DAYLIGHT_ALPHA: f32 = 0.13;
const SUNLIGHT_LEFT_ALPHA: f32 = 0.32;
const CLOUD: [f32; 3] = [0.9, 0.92, 0.96];
const RAIN: [f32; 3] = [0.35, 0.6, 1.];
const MARKER: [f32; 3] = [1., 1., 1.];

/// The daylight periods, `(sunrise, sunset)`, around `now`.
fn daylight(now: DateTime<Utc>, location: Location) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    let local = now + Duration::seconds((location.longitude * 240.) as i64);
    (-1..=1)
        .filter_map(|offset| {
            let date = local.date_naive() + Duration::days(offset);
            match sun_day(date, location) {
                SunDay::RiseAndSet { sunrise, sunset } => Some((sunrise, sunset)),
                SunDay::AlwaysUp => {
                    let midnight = date.and_hms_opt(0, 0, 0)?.and_utc();
                    Some((midnight, midnight + Duration::days(1)))
                },
                SunDay::AlwaysDown => None,
            }
        })
        .collect()
}

/// Generates the vertices of the timeline at `now`, `height` pixels tall at the top of the
/// window.
pub fn gen_vertices(
    now: DateTime<Utc>,
    location: Location,
    forecast: Option<&Forecast>,
    size_info: SizeInfo,
    height: f32,
) -> Vec<f32> {
    let mut triangles = Triangles::new(&size_info, 1.);

    // Window pixels from the bottom left corner, the timeline spans the width of the grid.
    let (left, right) = (size_info.padding_x, size_info.width - size_info.padding_x);
    if right <= left || height <= 0. {
        return vec![];
    }
    let (top, bottom) = (size_info.height, size_info.height - height);
    let middle = bottom + height / 2.;
    let (from, to) = (now.timestamp() - HALF_SPAN, now.timestamp() + HALF_SPAN);
    let x = |time: i64| left + (time - from) as f32 / (to - from) as f32 * (right - left);
    let mut span = |start: i64, end: i64, low: f32, high: f32, color, alpha| {
        let (start, end) = (start.max(from), end.min(to));
        if start < end {
            triangles.rect(Vec2::new(x(start), low), Vec2::new(x(end), high), color, alpha);
        }
    };

    // The night everywhere, the daylight over it.
    span(from, to, bottom, top, NIGHT, NIGHT_ALPHA);
    for (sunrise, sunset) in daylight(now, location) {
        let (sunrise, sunset) = (sunrise.timestamp(), sunset.timestamp());
        span(sunrise - TWILIGHT, sunrise, bottom, top, SUN, DAYLIGHT_ALPHA / 2.);
        span(sunset, sunset + TWILIGHT, bottom, top, SUN, DAYLIGHT_ALPHA / 2.);
        span(sunrise, sunset, bottom, top, SUN, DAYLIGHT_ALPHA);
        // The sunlight left until the sunset.
        span(sunrise.max(now.timestamp()), sunset, bottom, top, SUN, SUNLIGHT_LEFT_ALPHA);
    }

    // The weather on the lower half.
    if let Some(forecast) = forecast {
        for (start, end, cover) in forecast.cloud_periods(from, to) {
            span(start, end, bottom, middle, CLOUD, 0.04 + 0.16 * cover);
        }
        for (start, end, mm) in forecast.rain_periods(from, to) {
            span(start, end, bottom, middle, RAIN, 0.3 + 0.35 * (mm / 4.).min(1.));
        }
    }

    // Ticks at every hour, taller at midnight and noon.
    let first_hour = from - from.rem_euclid(3600) + 3600;
    for hour in (first_hour..to).step_by(3600) {
        let local = DateTime::from_timestamp(hour, 0)
            .map(|time| time.with_timezone(&chrono::Local).hour())
            .unwrap_or_default();
        let (tick, alpha) = if local % 12 == 0 { (height, 0.25) } else { (height * 0.4, 0.12) };
        triangles.line(
            Vec2::new(x(hour), bottom),
            Vec2::new(x(hour), bottom + tick),
            1.,
            MARKER,
            alpha,
        );
    }

    // A small triangle pointing down at the current time.
    let center = x(now.timestamp());
    let size = height.max(4.) * 0.8;
    triangles.push(
        [
            Vec2::new(center - size, top),
            Vec2::new(center, bottom - size * 0.6),
            Vec2::new(center + size, top),
        ],
        MARKER,
        0.55,
    );
    triangles.vertices
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decorations::weather::HourlyForecast;
    use chrono::TimeZone;

    fn size_info() -> SizeInfo {
        SizeInfo {
            width: 1240.,
            height: 600.,
            cell_width: 10.,
            cell_height: 20.,
            padding_x: 20.,
            padding_y: 0.,
            screen_lines: 30,
            columns: 120,
        }
    }

    /// The colors and alphas of the triangles whose x range covers the window position `x`.
    fn colors_at(vertices: &[f32], x: f32) -> Vec<([f32; 3], f32)> {
        let size_info = size_info();
        let scaled = size_info.scale_x(x - size_info.padding_x);
        vertices
            .chunks_exact(21)
            .filter(|triangle| {
                let xs = [triangle[0], triangle[7], triangle[14]];
                let min = xs.iter().copied().fold(f32::MAX, f32::min);
                let max = xs.iter().copied().fold(f32::MIN, f32::max);
                min <= scaled && scaled <= max && max - min > 1e-3
            })
            .map(|triangle| ([triangle[3], triangle[4], triangle[5]], triangle[6]))
            .collect()
    }

    #[test]
    fn daylight_rain_and_clouds() {
        let amsterdam = Location { latitude: 52.37, longitude: 4.9 };
        // 06:00 UTC on the 2026-12-21, the sun rises at 07:48 UTC.
        let now = Utc.with_ymd_and_hms(2026, 12, 21, 6, 0, 0).unwrap();
        let hour = 3600;
        let start = now.timestamp() + 3 * hour;
        let forecast = Forecast {
            points: vec![],
            hourly: Some(HourlyForecast {
                times: vec![start, start + hour],
                precipitation: vec![0., 2.],
                cloud_cover: vec![1., 0.],
            }),
        };
        let vertices = gen_vertices(now, amsterdam, Some(&forecast), size_info(), 6.);
        assert_eq!(vertices.len() % 21, 0);

        // One pixel is 24 hours over 1200 pixels, 72 seconds, now is at 620.
        let x = |offset: i64| 620. + offset as f32 / 72.;
        let at = |offset| colors_at(&vertices, x(offset));

        // Night 1 hour from now, the sunlight left after the sunrise, but not 12 hours later.
        assert!(!at(hour).iter().any(|&(color, _)| color == SUN));
        assert!(at(4 * hour).contains(&(SUN, SUNLIGHT_LEFT_ALPHA)));
        assert!(!at(11 * hour).iter().any(|&(color, _)| color == SUN));

        // Clouds until 3h30, rain from 3h to 4h.
        assert!(at(3 * hour - 600).iter().any(|&(color, _)| color == CLOUD));
        assert!(at(3 * hour + 1800).iter().any(|&(color, _)| color == RAIN));
        assert!(!at(5 * hour).iter().any(|&(color, _)| color == RAIN || color == CLOUD));

        // The marker at the current time.
        assert!(at(0).iter().any(|&(color, alpha)| color == MARKER && alpha > 0.5));
    }
}
