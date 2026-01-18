use crate::config::Config;
use beeper_desktop_api::BeeperClient;
use std::sync::{Arc, RwLock};

/// Application state shared across the entire app
pub struct AppState {
    pub config: RwLock<Config>,
    pub client: RwLock<BeeperClient>,
}

impl AppState {
    /// Create a new AppState with a configured client
    pub fn new(config: Config) -> Self {
        let client = BeeperClient::new(&config.api.token, &config.api.url);
        Self {
            config: RwLock::new(config),
            client: RwLock::new(client),
        }
    }
}

/// Wrapper for shared AppState with RwLock for thread-safe mutable access
pub struct SharedAppState(Arc<RwLock<AppState>>);

impl SharedAppState {
    /// Create a new SharedAppState
    pub fn new(config: Config) -> Self {
        SharedAppState(Arc::new(RwLock::new(AppState::new(config))))
    }

    /// Clone the Arc for sharing across threads/tasks
    pub fn clone_arc(&self) -> Arc<RwLock<AppState>> {
        Arc::clone(&self.0)
    }

    /// Update the API configuration and recreate the client
    pub fn update_api(&self, url: String, token: String) -> Result<(), String> {
        let state = self
            .0
            .write()
            .map_err(|e| format!("Failed to acquire write lock: {}", e))?;
        let mut config = state
            .config
            .write()
            .map_err(|e| format!("Failed to acquire config write lock: {}", e))?;
        config.api.url = url.clone();
        config.api.token = token.clone();
        drop(config); // Release the config lock before acquiring client lock

        let mut client = state
            .client
            .write()
            .map_err(|e| format!("Failed to acquire client write lock: {}", e))?;
        *client = BeeperClient::new(&token, &url);
        Ok(())
    }

    /// Get a cloned config
    pub fn get_config(&self) -> Result<Config, String> {
        let state = self
            .0
            .read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        let config = state
            .config
            .read()
            .map_err(|e| format!("Failed to acquire config read lock: {}", e))?;
        Ok(config.clone())
    }

    /// Execute a function with read-only access to the client
    pub fn with_client<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(&BeeperClient) -> T,
    {
        let state = self
            .0
            .read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        let client = state
            .client
            .read()
            .map_err(|e| format!("Failed to acquire client read lock: {}", e))?;
        Ok(f(&client))
    }

    /// Execute a function with mutable access to the config
    pub fn with_config_mut<F>(&self, f: F) -> Result<(), String>
    where
        F: FnOnce(&mut Config) -> (),
    {
        let state = self
            .0
            .read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        let mut config = state
            .config
            .write()
            .map_err(|e| format!("Failed to acquire config write lock: {}", e))?;
        f(&mut config);
        Ok(())
    }

    /// Execute a function with read-only access to the config
    pub fn with_config<F, T>(&self, f: F) -> Result<T, String>
    where
        F: FnOnce(&Config) -> T,
    {
        let state = self
            .0
            .read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;
        let config = state
            .config
            .read()
            .map_err(|e| format!("Failed to acquire config read lock: {}", e))?;
        Ok(f(&config))
    }

    /// Update the entire config and recreate the client if API config changed
    pub fn update_config(&self, new_config: Config) -> Result<(), String> {
        let state = self
            .0
            .read()
            .map_err(|e| format!("Failed to acquire read lock: {}", e))?;

        // Update config
        let mut config = state
            .config
            .write()
            .map_err(|e| format!("Failed to acquire config write lock: {}", e))?;
        let api_changed =
            config.api.url != new_config.api.url || config.api.token != new_config.api.token;
        *config = new_config.clone();
        drop(config); // Release config lock before acquiring client lock

        // Recreate client if API config changed
        if api_changed {
            let mut client = state
                .client
                .write()
                .map_err(|e| format!("Failed to acquire client write lock: {}", e))?;
            *client = BeeperClient::new(&new_config.api.token, &new_config.api.url);
        }

        Ok(())
    }
}

impl Clone for SharedAppState {
    fn clone(&self) -> Self {
        SharedAppState(Arc::clone(&self.0))
    }
}

/// Helper function for creating SharedAppState
pub fn create_shared_app_state(config: Config) -> SharedAppState {
    SharedAppState::new(config)
}
