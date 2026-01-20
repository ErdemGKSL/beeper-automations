#![cfg(windows)]

// Windows User Service (Hidden Window)
// 
// This binary runs the Beeper Automations service in the user's session
// without showing a console window. It's designed to be used with Scheduled Tasks.

// Hide the console window at startup
#[cfg(windows)]
fn hide_console_window() {
    use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE};
    
    unsafe {
        let h_console = windows::Win32::System::Console::GetConsoleWindow();
        if !h_console.is_invalid() {
            let _ = ShowWindow(h_console, SW_HIDE);
        }
    }
}

async fn main_impl() -> anyhow::Result<()> {
    use beeper_automations::logging::log_to_file;
    
    log_to_file("Beeper Automations User Service started (hidden window)");
    
    // Set working directory to ProgramData
    let work_dir = std::env::var("PROGRAMDATA")
        .unwrap_or_else(|_| "C:\\ProgramData".to_string())
        + "\\BeeperAutomations";

    log_to_file(&format!("Working directory: {}", work_dir));
    
    if let Err(e) = std::fs::create_dir_all(&work_dir) {
        log_to_file(&format!("Failed to create work directory: {}", e));
    }
    
    if let Err(e) = std::env::set_current_dir(&work_dir) {
        log_to_file(&format!("Failed to set working directory: {}", e));
        return Err(e.into());
    }

    // Initialize file-based logging (no console output)
    beeper_automations::logging::init_logging(true);
    log_to_file("File logging initialized");

    // Create shutdown channel for clean exit
    let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);

    // Set up Ctrl+C handler for graceful shutdown
    #[cfg(windows)]
    {
        use tokio::signal::windows::ctrl_c;
        let mut ctrl_c = ctrl_c()?;
        
        tokio::spawn(async move {
            if ctrl_c.recv().await.is_some() {
                let _ = shutdown_tx.send(()).await;
            }
        });
    }

    // Run the service
    log_to_file("Starting service loop");
    let result = beeper_automations::run_service_with_shutdown(shutdown_rx).await;
    
    match &result {
        Ok(_) => log_to_file("Service stopped gracefully"),
        Err(e) => {
            log_to_file(&format!("Service error: {}", e));
            log_to_file(&format!("Error details: {:?}", e));
        }
    }
    
    result
}

fn main() -> anyhow::Result<()> {
    // Hide console window to avoid showing cmd popup
    hide_console_window();
    
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(main_impl())
}
