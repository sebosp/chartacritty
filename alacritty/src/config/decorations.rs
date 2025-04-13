use serde::{de, Deserialize, Deserializer};
use toml::Value;

use alacritty_config_derive::{ConfigDeserialize, SerdeReplace};
use alacritty_terminal::decorations::DecorationsConfig;

use crate::config::ui_config::StringVisitor;

#[derive(ConfigDeserialize, Default, Clone, Debug, PartialEq)]
pub struct Decorations {
    /// Chart configuration
    pub config: SerdeDecorationsConfig,
}

#[derive(SerdeReplace, Default, Clone, Debug, PartialEq)]
pub struct SerdeDecorationsConfig(pub DecorationsConfig);

impl<'de> Deserialize<'de> for SerdeDecorationsConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = deserializer.deserialize_str(StringVisitor)?;
        DecorationsConfig::deserialize(Value::String(value))
            .map(SerdeDecorationsConfig)
            .map_err(de::Error::custom)
    }
}
