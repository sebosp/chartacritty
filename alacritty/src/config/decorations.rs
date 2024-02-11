use serde::{self, Deserialize, Serialize};

use alacritty_terminal::decorations::DecorationsConfig;

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct Decorations {
    /// Decorations configuration
    #[serde(flatten)]
    pub config: DecorationsConfig,
}

impl alacritty_config::SerdeReplace for Decorations {
    fn replace(&mut self, value: toml::Value) -> Result<(), Box<dyn std::error::Error>> {
        *self = serde::Deserialize::deserialize(value)?;

        Ok(())
    }
}
