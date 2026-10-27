//! Sunrise and sunset times for a location, from the NOAA solar equations, accurate to a couple
//! of minutes which is plenty to plan a walk.

use chrono::{DateTime, Datelike, Duration, NaiveDate, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// A location on earth, in degrees, it does not need to be precise.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Clone, Copy)]
pub struct Location {
    /// Positive to the north.
    pub latitude: f64,
    /// Positive to the east.
    pub longitude: f64,
}

/// The sun rises and sets, or it stays up or down the whole day near the poles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SunDay {
    RiseAndSet { sunrise: DateTime<Utc>, sunset: DateTime<Utc> },
    AlwaysUp,
    AlwaysDown,
}

/// Sunrise and sunset in UTC at `location` on `date`, as a date at the location.
pub fn sun_day(date: NaiveDate, location: Location) -> SunDay {
    // Fractional year in radians, at noon.
    let days = if date.leap_year() { 366. } else { 365. };
    let gamma = 2. * PI / days * (date.ordinal0() as f64);

    // Equation of time in minutes and solar declination in radians.
    let eqtime = 229.18
        * (0.000075 + 0.001868 * gamma.cos()
            - 0.032077 * gamma.sin()
            - 0.014615 * (2. * gamma).cos()
            - 0.040849 * (2. * gamma).sin());
    let decl = 0.006918 - 0.399912 * gamma.cos() + 0.070257 * gamma.sin()
        - 0.006758 * (2. * gamma).cos()
        + 0.000907 * (2. * gamma).sin()
        - 0.002697 * (3. * gamma).cos()
        + 0.00148 * (3. * gamma).sin();

    // Hour angle of the sun when its upper edge touches the horizon, with refraction.
    let latitude = location.latitude.to_radians();
    let cos_hour_angle =
        90.833f64.to_radians().cos() / (latitude.cos() * decl.cos()) - latitude.tan() * decl.tan();
    if cos_hour_angle > 1. {
        return SunDay::AlwaysDown;
    } else if cos_hour_angle < -1. {
        return SunDay::AlwaysUp;
    }
    let hour_angle = cos_hour_angle.acos().to_degrees();

    // Minutes after midnight UTC.
    let at = |minutes: f64| {
        let midnight = Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap());
        midnight + Duration::seconds((minutes * 60.).round() as i64)
    };
    SunDay::RiseAndSet {
        sunrise: at(720. - 4. * (location.longitude + hour_angle) - eqtime),
        sunset: at(720. - 4. * (location.longitude - hour_angle) - eqtime),
    }
}

/// The next sunrise or sunset after `now`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NextSunEvent {
    Sunrise(DateTime<Utc>),
    Sunset(DateTime<Utc>),
}

/// What the sun is doing at `now`: the next sunrise or sunset, if any in the coming days, and how
/// far along the day or the night we are, from 0 to 1.
pub fn sun_status(now: DateTime<Utc>, location: Location) -> Option<(NextSunEvent, f32)> {
    // The date at the location, roughly from its longitude.
    let local = now + Duration::seconds((location.longitude * 240.) as i64);
    let mut events = vec![];
    for offset in -1..=2 {
        let date = local.date_naive() + Duration::days(offset);
        if let SunDay::RiseAndSet { sunrise, sunset } = sun_day(date, location) {
            events.push(NextSunEvent::Sunrise(sunrise));
            events.push(NextSunEvent::Sunset(sunset));
        }
    }
    let time = |event: &NextSunEvent| match *event {
        NextSunEvent::Sunrise(time) | NextSunEvent::Sunset(time) => time,
    };
    events.sort_by_key(time);
    let next = events.iter().position(|event| time(event) > now)?;
    let progress = next
        .checked_sub(1)
        .map(|previous| {
            let (start, end) = (time(&events[previous]), time(&events[next]));
            (now - start).num_seconds() as f32 / (end - start).num_seconds().max(1) as f32
        })
        .unwrap_or(0.);
    Some((events[next], progress.clamp(0., 1.)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const AMSTERDAM: Location = Location { latitude: 52.37, longitude: 4.9 };

    fn minutes(time: DateTime<Utc>) -> i64 {
        (time.timestamp() - time.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp())
            / 60
    }

    #[test]
    fn amsterdam_sunrise_and_sunset() {
        // On the 2026-06-21 the sun rises at 05:18 and sets at 22:06 local time, UTC+2.
        let date = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
        let SunDay::RiseAndSet { sunrise, sunset } = sun_day(date, AMSTERDAM) else {
            panic!("the sun rises in Amsterdam");
        };
        assert!((minutes(sunrise) - (3 * 60 + 18)).abs() <= 4, "{sunrise}");
        assert!((minutes(sunset) - (20 * 60 + 6)).abs() <= 4, "{sunset}");

        // On the 2026-12-21 it rises at 08:48 and sets at 16:29 local time, UTC+1.
        let date = NaiveDate::from_ymd_opt(2026, 12, 21).unwrap();
        let SunDay::RiseAndSet { sunrise, sunset } = sun_day(date, AMSTERDAM) else {
            panic!("the sun rises in Amsterdam");
        };
        assert!((minutes(sunrise) - (7 * 60 + 48)).abs() <= 4, "{sunrise}");
        assert!((minutes(sunset) - (15 * 60 + 29)).abs() <= 4, "{sunset}");
    }

    #[test]
    fn polar_days_and_nights() {
        let svalbard = Location { latitude: 78.2, longitude: 15.6 };
        let summer = NaiveDate::from_ymd_opt(2026, 6, 21).unwrap();
        let winter = NaiveDate::from_ymd_opt(2026, 12, 21).unwrap();
        assert_eq!(sun_day(summer, svalbard), SunDay::AlwaysUp);
        assert_eq!(sun_day(winter, svalbard), SunDay::AlwaysDown);
    }

    #[test]
    fn next_sun_event() {
        // At 02:00 UTC in the summer, the night is almost over.
        let now = Utc.with_ymd_and_hms(2026, 6, 21, 2, 0, 0).unwrap();
        let (event, progress) = sun_status(now, AMSTERDAM).unwrap();
        let NextSunEvent::Sunrise(sunrise) = event else { panic!("{event:?}") };
        assert_eq!(sunrise.date_naive(), now.date_naive());
        assert!(progress > 0.7 && progress < 1., "{progress}");

        // At noon the sun sets next, about half way through the day.
        let now = Utc.with_ymd_and_hms(2026, 6, 21, 11, 40, 0).unwrap();
        let (event, progress) = sun_status(now, AMSTERDAM).unwrap();
        assert!(matches!(event, NextSunEvent::Sunset(_)), "{event:?}");
        assert!((progress - 0.5).abs() < 0.05, "{progress}");
    }
}
