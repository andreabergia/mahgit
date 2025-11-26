use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub theme: String,
    pub tab_width: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: "gruvbox-dark".to_string(),
            tab_width: 4,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to determine config directory")]
    NoConfigDir,

    #[error("Failed to read config file {0}: {1}")]
    ReadError(PathBuf, std::io::Error),

    #[error("Failed to parse config file {0}: {1}")]
    ParseError(PathBuf, toml::de::Error),
}

fn get_config_path() -> Result<PathBuf, ConfigError> {
    let config_dir = dirs::config_dir().ok_or(ConfigError::NoConfigDir)?;
    Ok(config_dir.join("mahgit").join("config.toml"))
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = get_config_path()?;

        // Missing config file is OK - use defaults
        if !config_path.exists() {
            return Ok(Self::default());
        }

        // Read and parse config file
        let contents = std::fs::read_to_string(&config_path)
            .map_err(|e| ConfigError::ReadError(config_path.clone(), e))?;

        // Parse TOML - serde will merge with defaults automatically
        let config: Config = toml::from_str(&contents)
            .map_err(|e| ConfigError::ParseError(config_path.clone(), e))?;

        Ok(config)
    }
}
