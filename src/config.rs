use crate::theme::Theme;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// ConfigFile represents the structure of the configuration file on disk.
/// This is what gets deserialized from TOML.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
struct ConfigFile {
    theme: String,
    tab_width: usize,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            theme: "gruvbox-dark".to_string(),
            tab_width: 4,
        }
    }
}

/// Config is the runtime configuration object that gets passed around.
/// It contains the resolved Theme object and other configuration values.
#[derive(Debug, Clone)]
pub struct Config {
    pub theme: Theme,
    pub tab_width: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Failed to determine config directory")]
    NoConfigDir,

    #[error("Failed to read config file {0}: {1}")]
    ReadError(PathBuf, std::io::Error),

    #[error("Failed to parse config file {0}: {1}")]
    ParseError(PathBuf, toml::de::Error),

    #[error("Unknown theme: {0}")]
    UnknownTheme(String),
}

fn get_config_path() -> Result<PathBuf, ConfigError> {
    // Try platform-specific config directory first (respects OS conventions)
    if let Some(config_dir) = dirs::config_dir() {
        let platform_path = config_dir.join("mahgit").join("config.toml");
        if platform_path.exists() {
            return Ok(platform_path);
        }
    }

    // Fall back to XDG-style ~/.config (more intuitive, cross-platform consistent)
    let home = dirs::home_dir().ok_or(ConfigError::NoConfigDir)?;
    let xdg_path = home.join(".config").join("mahgit").join("config.toml");

    // Return the XDG path even if it doesn't exist yet (for creation)
    Ok(xdg_path)
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = get_config_path()?;

        // Missing config file is OK - use defaults
        let config_file = if !config_path.exists() {
            ConfigFile::default()
        } else {
            // Read and parse config file
            let contents = std::fs::read_to_string(&config_path)
                .map_err(|e| ConfigError::ReadError(config_path.clone(), e))?;

            // Parse TOML - serde will merge with defaults automatically
            toml::from_str(&contents)
                .map_err(|e| ConfigError::ParseError(config_path.clone(), e))?
        };

        // Validate theme exists and resolve it
        let theme = Theme::from_name(&config_file.theme)
            .ok_or_else(|| ConfigError::UnknownTheme(config_file.theme.clone()))?;

        Ok(Config {
            theme,
            tab_width: config_file.tab_width,
        })
    }

    /// Expands tabs in a string to spaces based on the configured tab_width.
    /// Tracks column position to expand tabs to the next multiple of tab_width.
    pub fn expand_tabs(&self, line: &str) -> String {
        expand_tabs_with_width(line, self.tab_width)
    }
}

/// Expands tabs in a string to spaces based on the given tab_width.
/// Tracks column position to expand tabs to the next multiple of tab_width.
pub fn expand_tabs_with_width(line: &str, tab_width: usize) -> String {
    let mut result = String::with_capacity(line.len());
    let mut col = 0;

    for ch in line.chars() {
        if ch == '\t' {
            let spaces = tab_width - (col % tab_width);
            result.extend(std::iter::repeat_n(' ', spaces));
            col += spaces;
        } else {
            result.push(ch);
            col += 1;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tabs_no_tabs() {
        assert_eq!(expand_tabs_with_width("hello world", 4), "hello world");
        assert_eq!(expand_tabs_with_width("no tabs here", 8), "no tabs here");
    }

    #[test]
    fn test_expand_tabs_single_tab_at_start() {
        // Tab at position 0 should expand to 4 spaces (tab_width=4)
        assert_eq!(expand_tabs_with_width("\thello", 4), "    hello");
        // Tab at position 0 should expand to 8 spaces (tab_width=8)
        assert_eq!(expand_tabs_with_width("\thello", 8), "        hello");
    }

    #[test]
    fn test_expand_tabs_single_tab_at_middle() {
        // "a" is at col 0, tab at col 1 should expand to 3 spaces (next multiple of 4 is 4)
        assert_eq!(expand_tabs_with_width("a\tb", 4), "a   b");
        // "ab" ends at col 2, tab at col 2 should expand to 2 spaces (next multiple of 4 is 4)
        assert_eq!(expand_tabs_with_width("ab\tc", 4), "ab  c");
        // "abc" ends at col 3, tab at col 3 should expand to 1 space (next multiple of 4 is 4)
        assert_eq!(expand_tabs_with_width("abc\td", 4), "abc d");
        // "abcd" ends at col 4, tab at col 4 should expand to 4 spaces (next multiple of 4 is 8)
        assert_eq!(expand_tabs_with_width("abcd\te", 4), "abcd    e");
    }

    #[test]
    fn test_expand_tabs_multiple_tabs() {
        // Two tabs: first at col 0 -> 4 spaces, second at col 4 -> 4 spaces
        assert_eq!(expand_tabs_with_width("\t\thello", 4), "        hello");
        // "a" + tab + "b" + tab
        assert_eq!(expand_tabs_with_width("a\tb\tc", 4), "a   b   c");
    }

    #[test]
    fn test_expand_tabs_different_widths() {
        assert_eq!(expand_tabs_with_width("\thello", 2), "  hello");
        assert_eq!(expand_tabs_with_width("a\tb", 2), "a b");
        assert_eq!(expand_tabs_with_width("\thello", 8), "        hello");
    }

    #[test]
    fn test_expand_tabs_empty_string() {
        assert_eq!(expand_tabs_with_width("", 4), "");
    }

    #[test]
    fn test_expand_tabs_only_tabs() {
        assert_eq!(expand_tabs_with_width("\t", 4), "    ");
        assert_eq!(expand_tabs_with_width("\t\t", 4), "        ");
    }

    #[test]
    fn test_config_file_default() {
        let config_file = ConfigFile::default();
        assert_eq!(config_file.theme, "gruvbox-dark");
        assert_eq!(config_file.tab_width, 4);
    }

    #[test]
    fn test_config_expand_tabs() {
        let theme = Theme::from_name("gruvbox-dark").unwrap();
        let config = Config {
            theme,
            tab_width: 4,
        };
        assert_eq!(config.expand_tabs("\thello"), "    hello");

        let theme_8 = Theme::from_name("gruvbox-dark").unwrap();
        let config_width_8 = Config {
            theme: theme_8,
            tab_width: 8,
        };
        assert_eq!(config_width_8.expand_tabs("\thello"), "        hello");
    }

    #[test]
    fn test_config_load_missing_file() {
        // This test assumes no config file exists at the default location
        // or that the config directory doesn't exist. Since we can't control
        // that in a unit test easily, we just verify the function doesn't panic.
        let result = Config::load();
        assert!(result.is_ok());
    }
}
