pub mod config;
pub mod tui;
pub mod api_check;
pub mod notifications;
pub mod app_state;

use anyhow::Result;
use notify::{Watcher, RecursiveMode, Event};
use tokio::signal;

pub async fn run_service() -> Result<()> {
    println!("Starting Beeper Automations Service...");

    // Load configuration
    let config = config::Config::load()?;
    let config_path = config::Config::config_file_path()?;

    // Check if API is configured, if not wait for hot reload
    if !config.is_api_configured() {
        println!("⚠ API configuration not found. Waiting for configuration...");
        println!("  Config file: {:?}", config_path);
        println!("  Please run 'auto-beeper-configurator' to set up API configuration.");
        println!("  Service will automatically start once configuration is detected.\n");
    }

    // Initialize shared app state
    let app_state = app_state::SharedAppState::new(config.clone());
    
    // Create hot reload channel
    let (reload_tx, reload_rx) = tokio::sync::mpsc::channel::<config::Config>(10);
    
    // Always start the service with the reload receiver
    let _notification_service = notifications::service::NotificationService::new(app_state.clone(), reload_rx);
    
    // If API is configured, trigger initial load
    if config.is_api_configured() {
        print_config_status(&config);
        println!("\n🚀 Starting notification service...");
        
        // Send initial config to start automations
        if let Err(e) = reload_tx.send(config.clone()).await {
            eprintln!("✗ Error sending initial config: {}", e);
        } else {
            println!("✓ Service running. Press Ctrl+C to stop.\n");
        }
    }

    // Set up config file watcher
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Result<Event, notify::Error>>(100);
    
    let mut watcher = notify::recommended_watcher(move |res| {
        let _ = tx.blocking_send(res);
    })?;
    
    if let Some(parent) = config_path.parent() {
        watcher.watch(parent, RecursiveMode::NonRecursive)?;
    }
    
    // Spawn config reload task
    let config_path_clone = config_path.clone();
    
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            if let Ok(event) = event {
                // Check if config file was modified
                let config_modified = event.paths.iter().any(|p| p == &config_path_clone);
                
                if config_modified && (event.kind.is_modify() || event.kind.is_create()) {
                    println!("\n📝 Configuration file changed, reloading...");
                    
                    // Small delay to ensure file is fully written
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    
                    match config::Config::load() {
                        Ok(new_config) => {
                            if new_config.is_api_configured() {
                                print_config_status(&new_config);
                                
                                // Send reload signal to notification service
                                if let Err(e) = reload_tx.send(new_config).await {
                                    eprintln!("✗ Error sending reload signal: {}", e);
                                }
                            } else {
                                println!("⚠ Configuration loaded but API is not configured yet.");
                                println!("  Waiting for complete configuration...\n");
                            }
                        }
                        Err(e) => {
                            eprintln!("✗ Error reloading configuration: {}", e);
                        }
                    }
                }
            }
        }
    });

    // Wait for shutdown signal
    match signal::ctrl_c().await {
        Ok(()) => {
            println!("\n\n🛑 Received shutdown signal. Stopping service...");
        }
        Err(err) => {
            eprintln!("Error waiting for shutdown signal: {}", err);
        }
    }
    
    println!("✓ Service stopped.");
    
    Ok(())
}

fn print_config_status(config: &config::Config) {
    println!("✓ Configuration loaded successfully!");
    println!("  API URL: {}", config.api.url);
    println!("  Token: {}***", &config.api.token[..config.api.token.len().min(4)]);
    
    // Display enabled automations
    let enabled_count = config.notifications.automations.iter().filter(|a| a.enabled).count();
    println!("  Enabled automations: {}", enabled_count);
}
