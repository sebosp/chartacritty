/// Utilities moved from alacritty_terminal/src/config/mod.rs
use log::error;
use serde::{Deserialize, Deserializer};
use serde_yaml::Value;
use std::fmt::Display;

pub const LOG_TARGET_CONFIG: &str = "alacritty_config";

fn fallback_default<T, E>(err: E) -> T
where
    T: Default,
    E: Display,
{
    error!(target: LOG_TARGET_CONFIG, "Problem with config: {}; using default value", err);
    T::default()
}

pub fn failure_default<'a, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'a>,
    T: Deserialize<'a> + Default,
{
    Ok(T::deserialize(Value::deserialize(deserializer)?).unwrap_or_else(fallback_default))
}
