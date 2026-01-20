use crate::notifications::NotificationAutomation;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parsing error: {0}")]
    TomlError(#[from] toml::de::Error),
    #[error("TOML serialization error: {0}")]
    TomlSerError(#[from] toml::ser::Error),
    #[error("Missing configuration directory")]
    NoConfigDir,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub notifications: NotificationsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsConfig {
    #[serde(default)]
    pub automations: Vec<NotificationAutomation>,
}

impl Default for NotificationsConfig {
    fn default() -> Self {
        Self {
            automations: Vec::new(),
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:23373".to_string(),
            token: String::new(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            api: ApiConfig::default(),
            notifications: NotificationsConfig::default(),
        }
    }
}

impl Config {
    /// Get the configuration file path
    pub fn config_file_path() -> Result<PathBuf, ConfigError> {
        let config_dir = dirs::config_dir().ok_or(ConfigError::NoConfigDir)?;
        Ok(config_dir.join("beeper-automations").join("config.toml"))
    }

    /// Load configuration from file, creating default if it doesn't exist
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = Self::config_file_path()?;

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            Ok(toml::from_str(&content)?)
        } else {
            // Create default config
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<(), ConfigError> {
        let config_path = Self::config_file_path()?;

        // Create parent directories if they don't exist
        if let Some(parent) = config_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;

        Ok(())
    }

    /// Check if API credentials are configured
    pub fn is_api_configured(&self) -> bool {
        !self.api.token.is_empty() && !self.api.url.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.api.url, "http://localhost:23373");
        assert!(config.api.token.is_empty());
    }

    #[test]
    fn test_is_api_configured() {
        let mut config = Config::default();
        assert!(!config.is_api_configured());

        config.api.token = "test-token".to_string();
        assert!(config.is_api_configured());
    }
}
