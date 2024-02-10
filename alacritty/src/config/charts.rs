use serde::{de, Deserialize, Deserializer};
use toml::Value;

use alacritty_config_derive::{ConfigDeserialize, SerdeReplace};
use alacritty_terminal::charts::ChartsConfig;

use crate::config::ui_config::StringVisitor;

#[derive(ConfigDeserialize, Default, Clone, Debug, PartialEq)]
pub struct Charts {
    /// Chart configuration
    pub config: SerdeChartsConfig,
}

#[derive(SerdeReplace, Default, Clone, Debug, PartialEq)]
pub struct SerdeChartsConfig(pub ChartsConfig);

impl<'de> Deserialize<'de> for SerdeChartsConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = deserializer.deserialize_str(StringVisitor)?;
        ChartsConfig::deserialize(Value::String(value))
            .map(SerdeChartsConfig)
            .map_err(de::Error::custom)
    }
}
