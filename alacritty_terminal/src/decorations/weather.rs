//! Rain forecast around a location from Open-Meteo, https://open-meteo.com, which needs no API
//! key. The precipitation of the next hours is fetched at the location and at points on rings
//! around it, to tell when rain comes and whether it is close, along with the hourly rain and
//! cloud cover of the location from 12 hours ago to 12 hours from now.

use super::sun::Location;
use log::*;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

/// Precipitation of a quarter of an hour, in millimeters, from which it rains.
const RAIN_MM: f32 = 0.05;

/// Distances of the rings of points around the location, in kilometers.
const RINGS_KM: [f64; 2] = [15., 35.];

/// Directions of the points of the rings, in degrees from the north.
const BEARINGS: [f64; 8] = [0., 45., 90., 135., 180., 225., 270., 315.];

/// Quarters of an hour of forecast, the rain coming later than this is not interesting.
const FORECAST_QUARTERS: usize = 12;

/// Hours of hourly forecast before and after now.
const PAST_HOURS: usize = 13;
const FORECAST_HOURS: usize = 14;

/// Rain around the rings within this many minutes is shown as nearby.
const NEARBY_MINUTES: i64 = 45;

/// Configuration of the corner of the window showing the sun and the rain.
#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(default)]
pub struct WeatherConfig {
    /// Whether the corner is shown, it needs `decorations.location`.
    pub enabled: bool,
    /// Whether the next sunrise or sunset is shown.
    pub sun: bool,
    /// Whether the rain forecast is fetched and shown.
    pub rain: bool,
    /// Size of the corner, at the bottom right of the grid, in cells.
    pub columns: usize,
    pub lines: usize,
    /// Minutes between forecasts.
    pub refresh_minutes: u64,
    /// Whether the timeline of the daylight and the weather is drawn along the top of the window.
    pub timeline: bool,
    /// Height of the timeline in pixels.
    pub timeline_height: f32,
}

impl Default for WeatherConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sun: true,
            rain: true,
            columns: 32,
            lines: 6,
            refresh_minutes: 10,
            timeline: true,
            timeline_height: 6.,
        }
    }
}

/// The forecast at a point around the location.
#[derive(Debug, Clone, PartialEq)]
pub struct PointForecast {
    /// Distance and direction from the location, in kilometers and degrees from the north.
    pub distance_km: f64,
    pub bearing: f64,
    /// End of each quarter of an hour, in seconds since the epoch, and its precipitation in mm.
    pub times: Vec<i64>,
    pub precipitation: Vec<f32>,
}

impl PointForecast {
    /// The first quarter of an hour, ending after `now`, in which it rains: when it starts and
    /// how much it rains.
    fn first_rain(&self, now: i64) -> Option<(i64, f32)> {
        self.times
            .iter()
            .zip(&self.precipitation)
            .filter(|&(&end, _)| end > now)
            .find(|&(_, &mm)| mm >= RAIN_MM)
            .map(|(&end, &mm)| ((end - 15 * 60).max(now), mm))
    }
}

/// The hourly forecast at the location.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HourlyForecast {
    /// The hours in seconds since the epoch.
    pub times: Vec<i64>,
    /// Precipitation of the hour before each time, in mm.
    pub precipitation: Vec<f32>,
    /// Cloud cover at each time, from 0 to 1.
    pub cloud_cover: Vec<f32>,
}

/// The forecast at the location, first, and around it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Forecast {
    pub points: Vec<PointForecast>,
    pub hourly: Option<HourlyForecast>,
}

impl Forecast {
    /// The periods between `from` and `to` in which it rains at the location, with the rain in
    /// mm per hour, from the quarters of an hour where they are known and from the hours
    /// elsewhere. Periods are `(start, end, mm per hour)` in seconds since the epoch.
    pub fn rain_periods(&self, from: i64, to: i64) -> Vec<(i64, i64, f32)> {
        let mut periods = vec![];
        let quarters = self.points.first().filter(|point| !point.times.is_empty());
        let quarters_span =
            quarters.map(|point| (point.times[0] - 15 * 60, point.times[point.times.len() - 1]));
        if let Some(point) = quarters {
            for (&end, &mm) in point.times.iter().zip(&point.precipitation) {
                periods.push((end - 15 * 60, end, mm * 4.));
            }
        }
        if let Some(hourly) = &self.hourly {
            for (&end, &mm) in hourly.times.iter().zip(&hourly.precipitation) {
                let start = end - 3600;
                // The quarters of an hour are more precise, only the parts of the hour before
                // and after them are kept.
                let parts = match quarters_span {
                    Some((first, last)) => [(start, end.min(first)), (start.max(last), end)],
                    None => [(start, end), (end, end)],
                };
                for (start, end) in parts {
                    if start < end {
                        periods.push((start, end, mm));
                    }
                }
            }
        }
        periods
            .into_iter()
            .filter(|&(start, end, mm)| mm >= RAIN_MM * 4. && end > from && start < to)
            .map(|(start, end, mm)| (start.max(from), end.min(to), mm))
            .collect()
    }

    /// The cloud cover between `from` and `to`, as `(start, end, cover)` periods around every
    /// hour, with the cover from 0 to 1.
    pub fn cloud_periods(&self, from: i64, to: i64) -> Vec<(i64, i64, f32)> {
        let Some(hourly) = &self.hourly else { return vec![] };
        hourly
            .times
            .iter()
            .zip(&hourly.cloud_cover)
            .map(|(&time, &cover)| (time - 1800, time + 1800, cover))
            .filter(|&(start, end, cover)| cover > 0. && end > from && start < to)
            .map(|(start, end, cover)| (start.max(from), end.min(to), cover))
            .collect()
    }
}

/// Where the forecast is requested: the location, then the rings around it, with their
/// distance and direction.
fn probe_points(location: Location) -> Vec<(Location, f64, f64)> {
    let mut points = vec![(location, 0., 0.)];
    for distance in RINGS_KM {
        for bearing in BEARINGS {
            let (sin, cos) = bearing.to_radians().sin_cos();
            let latitude = location.latitude + distance * cos / 111.32;
            let longitude = location.longitude
                + distance * sin / (111.32 * location.latitude.to_radians().cos().max(0.01));
            points.push((Location { latitude, longitude }, distance, bearing));
        }
    }
    points
}

/// The Open-Meteo request of the precipitation at the `points`.
fn forecast_url(points: &[(Location, f64, f64)]) -> String {
    let join = |coordinate: fn(&Location) -> f64| {
        points
            .iter()
            .map(|(location, ..)| format!("{:.3}", coordinate(location)))
            .collect::<Vec<_>>()
            .join(",")
    };
    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&minutely_15=precipitation&\
         forecast_minutely_15={FORECAST_QUARTERS}&past_minutely_15=1&hourly=precipitation,\
         cloud_cover&past_hours={PAST_HOURS}&forecast_hours={FORECAST_HOURS}&timeformat=unixtime",
        join(|location| location.latitude),
        join(|location| location.longitude),
    )
}

/// Parses the Open-Meteo response for the `points`, a list of forecasts in their order.
fn parse_forecast(json: &serde_json::Value, points: &[(Location, f64, f64)]) -> Option<Forecast> {
    let forecasts = match json {
        serde_json::Value::Array(forecasts) => forecasts.iter().collect::<Vec<_>>(),
        forecast => vec![forecast],
    };
    if forecasts.len() != points.len() {
        return None;
    }
    let hourly = forecasts.first().and_then(|forecast| {
        let hourly = forecast.get("hourly")?;
        let values = |name: &str, scale: f64| -> Option<Vec<f32>> {
            let values = hourly.get(name)?.as_array()?;
            Some(values.iter().map(|value| (value.as_f64().unwrap_or(0.) * scale) as f32).collect())
        };
        let times: Option<Vec<i64>> =
            hourly.get("time")?.as_array()?.iter().map(|t| t.as_i64()).collect();
        Some(HourlyForecast {
            times: times?,
            precipitation: values("precipitation", 1.)?,
            cloud_cover: values("cloud_cover", 0.01)?,
        })
    });
    let points = forecasts
        .into_iter()
        .zip(points)
        .map(|(forecast, &(_, distance_km, bearing))| {
            let minutely = forecast.get("minutely_15")?;
            let times: Option<Vec<i64>> =
                minutely.get("time")?.as_array()?.iter().map(|t| t.as_i64()).collect();
            // Missing values are dry.
            let precipitation = minutely
                .get("precipitation")?
                .as_array()?
                .iter()
                .map(|mm| mm.as_f64().unwrap_or(0.) as f32)
                .collect();
            Some(PointForecast { distance_km, bearing, times: times?, precipitation })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Forecast { points, hourly })
}

/// What the rain is doing around the location.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RainOutlook {
    /// It rains at the location, in mm per quarter of an hour.
    Raining {
        mm: f32,
    },
    /// It rains at the location in some minutes.
    Coming {
        minutes: i64,
        mm: f32,
    },
    /// It rains, or is about to, close to the location.
    Nearby {
        distance_km: f64,
        bearing: f64,
    },
    Dry,
}

impl RainOutlook {
    pub fn new(forecast: &Forecast, now: i64) -> Self {
        let Some((location, around)) = forecast.points.split_first() else {
            return Self::Dry;
        };
        if let Some((start, mm)) = location.first_rain(now) {
            let minutes = (start - now) / 60;
            return if minutes <= 0 { Self::Raining { mm } } else { Self::Coming { minutes, mm } };
        }
        around
            .iter()
            .filter(|point| {
                point.first_rain(now).is_some_and(|(start, _)| start - now <= NEARBY_MINUTES * 60)
            })
            .min_by(|a, b| a.distance_km.total_cmp(&b.distance_km))
            .map_or(Self::Dry, |point| Self::Nearby {
                distance_km: point.distance_km,
                bearing: point.bearing,
            })
    }

    /// How hard the rain is drawn, from 0 for none to 1.
    pub fn intensity(&self) -> f32 {
        match *self {
            Self::Raining { mm } => (0.5 + mm).min(1.),
            Self::Coming { minutes, .. } => 0.15 + 0.35 * (1. - minutes as f32 / 180.).max(0.),
            Self::Nearby { .. } => 0.1,
            Self::Dry => 0.,
        }
    }

    pub fn describe(&self) -> String {
        match *self {
            Self::Raining { mm } => format!("rain now {:.1}mm/h", mm * 4.),
            Self::Coming { minutes, mm } => {
                format!("rain in {} {:.1}mm/h", format_minutes(minutes), mm * 4.)
            },
            Self::Nearby { distance_km, bearing } => {
                format!("rain {distance_km:.0}km {}", compass(bearing))
            },
            Self::Dry => String::from("no rain for 3h"),
        }
    }
}

/// Minutes as `1h05m` or `35m`.
pub fn format_minutes(minutes: i64) -> String {
    if minutes >= 60 {
        format!("{}h{:02}m", minutes / 60, minutes % 60)
    } else {
        format!("{minutes}m")
    }
}

/// The compass point of a direction in degrees from the north.
fn compass(bearing: f64) -> &'static str {
    const POINTS: [&str; 8] = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];
    POINTS[((bearing.rem_euclid(360.) + 22.5) / 45.) as usize % 8]
}

/// Fetches the forecast regularly in a background thread.
#[derive(Debug)]
pub struct WeatherWatcher {
    forecast: Arc<Mutex<Option<Forecast>>>,
    stop: Arc<AtomicBool>,
}

impl WeatherWatcher {
    pub fn spawn(location: Location, refresh_minutes: u64) -> Self {
        let forecast = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let (shared, stopped) = (forecast.clone(), stop.clone());
        let refresh = Duration::from_secs(refresh_minutes.max(1) * 60);
        let spawned = thread::Builder::new().name("weather".to_owned()).spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(err) => return error!("Unable to start the weather runtime: {err}"),
            };
            let points = probe_points(location);
            let url = forecast_url(&points);
            let client = reqwest::Client::builder().timeout(Duration::from_secs(30)).build();
            let Ok(client) = client else { return error!("Unable to create the weather client") };
            while !stopped.load(Ordering::Relaxed) {
                let response = runtime.block_on(async {
                    client
                        .get(&url)
                        .send()
                        .await?
                        .error_for_status()?
                        .json::<serde_json::Value>()
                        .await
                });
                match response.map(|json| parse_forecast(&json, &points)) {
                    Ok(Some(new)) => *shared.lock().unwrap() = Some(new),
                    Ok(None) => warn!("Unexpected weather forecast from {url}"),
                    Err(err) => info!("Unable to fetch the weather forecast: {err}"),
                }

                // Sleep in small steps to stop soon after the watcher is dropped.
                let mut slept = Duration::ZERO;
                while slept < refresh && !stopped.load(Ordering::Relaxed) {
                    thread::sleep(Duration::from_millis(500));
                    slept += Duration::from_millis(500);
                }
            }
        });
        if let Err(err) = spawned {
            error!("Unable to start the weather thread: {err}");
        }
        Self { forecast, stop }
    }

    /// The last forecast, if any was fetched yet.
    pub fn forecast(&self) -> Option<Forecast> {
        self.forecast.lock().ok()?.clone()
    }
}

impl Drop for WeatherWatcher {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn point(distance_km: f64, bearing: f64, precipitation: &[f32]) -> PointForecast {
        let times = (0..precipitation.len()).map(|i| 1000 + i as i64 * 900).collect();
        PointForecast { distance_km, bearing, times, precipitation: precipitation.to_vec() }
    }

    #[test]
    fn rain_outlooks() {
        let dry = [0.; 6];
        let now = 1000;
        let forecast = |points| Forecast { points, hourly: None };
        assert_eq!(RainOutlook::new(&forecast(vec![point(0., 0., &dry)]), now), RainOutlook::Dry);

        // The quarter of an hour ending at 1900 covers now.
        let raining = forecast(vec![point(0., 0., &[0., 0.3, 0., 0., 0., 0.])]);
        assert_eq!(RainOutlook::new(&raining, now), RainOutlook::Raining { mm: 0.3 });

        // Rain from 2800, 30 minutes from now.
        let coming = forecast(vec![point(0., 0., &[0., 0., 0., 0.2, 0., 0.])]);
        assert_eq!(RainOutlook::new(&coming, now), RainOutlook::Coming { minutes: 30, mm: 0.2 });

        // The closest rain around, the rain in more than 45 minutes is ignored.
        let around = forecast(vec![
            point(0., 0., &dry),
            point(35., 90., &[0., 0.5, 0., 0., 0., 0.]),
            point(15., 315., &[0., 0., 0.1, 0., 0., 0.]),
            point(15., 180., &[0., 0., 0., 0., 0., 2.]),
        ]);
        assert_eq!(RainOutlook::new(&around, now), RainOutlook::Nearby {
            distance_km: 15.,
            bearing: 315.
        });
        assert_eq!(RainOutlook::new(&around, now).describe(), "rain 15km NW");
    }

    #[test]
    fn parse_open_meteo() {
        let points = probe_points(Location { latitude: 52.37, longitude: 4.9 });
        assert_eq!(points.len(), 17);
        let url = forecast_url(&points);
        assert!(url.contains("latitude=52.370,52.505,"), "{url}");

        let forecast = serde_json::json!({
            "minutely_15": { "time": [1000, 1900], "precipitation": [0.0, 0.25] },
            "hourly": { "time": [3600, 7200], "precipitation": [0.0, 1.5], "cloud_cover": [20, 100] }
        });
        let json = serde_json::Value::Array(vec![forecast; 17]);
        let parsed = parse_forecast(&json, &points).unwrap();
        assert_eq!(parsed.points.len(), 17);
        assert_eq!(parsed.points[0].precipitation, [0., 0.25]);
        assert_eq!(parsed.points[16].bearing, 315.);
        assert!(parse_forecast(&json, &points[..3]).is_none());
        let hourly = parsed.hourly.unwrap();
        assert_eq!(hourly.precipitation, [0., 1.5]);
        assert_eq!(hourly.cloud_cover, [0.2, 1.]);
    }

    #[test]
    fn rain_and_cloud_periods() {
        let hour = 3600;
        let forecast = Forecast {
            // Quarters of an hour from 10:00 to 11:00, raining from 10:30.
            points: vec![PointForecast {
                distance_km: 0.,
                bearing: 0.,
                times: (1..=4).map(|i| 10 * hour + i * 900).collect(),
                precipitation: vec![0., 0., 0.5, 0.5],
            }],
            // Hours ending at 9:00 to 13:00, raining from 8:00 to 9:00 and 11:00 to 13:00.
            hourly: Some(HourlyForecast {
                times: (9..=13).map(|h| h * hour).collect(),
                precipitation: vec![1., 0., 3., 2., 1.],
                cloud_cover: vec![0., 0.5, 1., 1., 0.],
            }),
        };
        assert_eq!(forecast.rain_periods(0, 24 * hour), [
            (10 * hour + 1800, 10 * hour + 2700, 2.),
            (10 * hour + 2700, 11 * hour, 2.),
            (8 * hour, 9 * hour, 1.),
            // The hour ending at 11:00 is known from the quarters of an hour.
            (11 * hour, 12 * hour, 2.),
            (12 * hour, 13 * hour, 1.),
        ]);
        // Clipped to the requested time.
        assert_eq!(forecast.rain_periods(12 * hour + 1800, 24 * hour), [(
            12 * hour + 1800,
            13 * hour,
            1.
        )]);
        assert_eq!(forecast.cloud_periods(0, 24 * hour), [
            (10 * hour - 1800, 10 * hour + 1800, 0.5),
            (11 * hour - 1800, 11 * hour + 1800, 1.),
            (12 * hour - 1800, 12 * hour + 1800, 1.),
        ]);
    }

    #[test]
    fn minutes_and_compass() {
        assert_eq!(format_minutes(35), "35m");
        assert_eq!(format_minutes(65), "1h05m");
        assert_eq!(compass(350.), "N");
        assert_eq!(compass(100.), "E");
    }
}
