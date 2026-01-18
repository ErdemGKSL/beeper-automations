#![cfg(windows)]

use std::ffi::OsString;
use std::fs::OpenOptions;
use std::io::Write;
use std::time::Duration;
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};

const SERVICE_NAME: &str = "BeeperAutomations";
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;
const MAX_LOG_LINES: usize = 500;

fn log_to_file(msg: &str) {
    let log_path = std::env::var("PROGRAMDATA").unwrap_or_else(|_| "C:\\ProgramData".to_string())
        + "\\BeeperAutomations\\service.log";

    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let new_line = format!("[{}] {}", timestamp, msg);

    // Read existing lines if file exists
    let mut lines = if let Ok(content) = std::fs::read_to_string(&log_path) {
        content.lines().map(String::from).collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    // Add new line
    lines.push(new_line);

    // Keep only last MAX_LOG_LINES
    if lines.len() > MAX_LOG_LINES {
        let skip_count = lines.len() - MAX_LOG_LINES;
        lines = lines.into_iter().skip(skip_count).collect();
    }

    // Write back to file
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&log_path)
    {
        for line in lines {
            let _ = writeln!(f, "{}", line);
        }
    }
}

define_windows_service!(ffi_service_main, service_main);

fn service_main(_arguments: Vec<OsString>) {
    log_to_file("Windows service wrapper started");
    if let Err(e) = run_service() {
        log_to_file(&format!("Service error: {}", e));
    }
    log_to_file("Windows service wrapper exiting");
}

fn run_service() -> windows_service::Result<()> {
    log_to_file("run_service() called");

    // Set working directory to ProgramData
    let work_dir = std::env::var("PROGRAMDATA").unwrap_or_else(|_| "C:\\ProgramData".to_string())
        + "\\BeeperAutomations";

    if let Err(e) = std::fs::create_dir_all(&work_dir) {
        log_to_file(&format!("Failed to create work directory: {}", e));
    }

    if let Err(e) = std::env::set_current_dir(&work_dir) {
        log_to_file(&format!("Failed to set working directory: {}", e));
    } else {
        log_to_file(&format!("Working directory set to: {}", work_dir));
    }

    // Create a channel to handle service stop events
    let (shutdown_tx, shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);

    // Define the service control handler
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                // Signal the service to stop
                let _ = shutdown_tx.blocking_send(());
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    // Register the service control handler
    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    log_to_file("Service control handler registered");

    // Tell Windows that the service is starting
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::StartPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(0),
        process_id: None,
    })?;

    // Create a Tokio runtime for the async service
    let runtime = tokio::runtime::Runtime::new().map_err(|e| {
        windows_service::Error::Winapi(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("Failed to create Tokio runtime: {}", e),
        ))
    })?;

    // Tell Windows that the service is running
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(0),
        process_id: None,
    })?;

    log_to_file("Service status set to Running");
    log_to_file("About to call beeper_auotmations::run_service_with_shutdown()");

    // Run the service and wait for shutdown signal
    runtime.block_on(async {
        tokio::select! {
            result = beeper_auotmations::run_service_with_shutdown(shutdown_rx) => {
                if let Err(e) = result {
                    log_to_file(&format!("Service error: {}", e));
                }
                log_to_file("run_service_with_shutdown() RETURNED");
            }
        }
    });

    log_to_file("Service loop exited, initiating shutdown");

    // Tell Windows that the service is stopping
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::StopPending,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(0),
        process_id: None,
    })?;

    // Tell Windows that the service has stopped
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::from_secs(0),
        process_id: None,
    })?;

    Ok(())
}

fn main() -> windows_service::Result<()> {
    // Register the service with Windows Service Control Manager
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}
