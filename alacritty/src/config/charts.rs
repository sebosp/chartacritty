use serde::{self, Deserialize, Serialize};

use alacritty_terminal::charts::ChartsConfig;

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
pub struct Charts {
    /// Chart configuration
    #[serde(flatten)]
    pub config: ChartsConfig,
}

impl alacritty_config::SerdeReplace for Charts {
    fn replace(&mut self, value: toml::Value) -> Result<(), Box<dyn std::error::Error>> {
        *self = serde::Deserialize::deserialize(value)?;

        Ok(())
    }
}
