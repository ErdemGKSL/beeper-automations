#![cfg(windows)]

use std::ffi::OsString;
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

// Use the shared logging function
use beeper_auotmations::logging::log_to_file;

define_windows_service!(ffi_service_main, service_main);

fn write_crash_log(msg: &str) {
    let log_path = std::env::var("PROGRAMDATA")
        .unwrap_or_else(|_| "C:\\ProgramData".to_string())
        + "\\BeeperAutomations\\service_crash.log";

    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");

    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(&log_path)
    {
        use std::io::Write;
        let _ = writeln!(f, "[{}] {}", timestamp, msg);
    }
}

fn service_main(_arguments: Vec<OsString>) {
    write_crash_log("service_main() called");

    // Initialize tracing for Windows service mode BEFORE any other logging
    beeper_auotmations::logging::init_logging(true);
    log_to_file("Windows service wrapper started");

    write_crash_log("Logging initialized, about to call run_service()");

    if let Err(e) = run_service() {
        let error_msg = format!("Service error: {}", e);
        log_to_file(&error_msg);
        log_to_file(&format!("Error details: {:?}", e));
        write_crash_log(&error_msg);
    }

    log_to_file("Windows service wrapper exiting");
    write_crash_log("Windows service wrapper exiting");
}

fn run_service() -> windows_service::Result<()> {
    log_to_file("run_service() called");

    // Set working directory to ProgramData
    let work_dir = std::env::var("PROGRAMDATA").unwrap_or_else(|_| "C:\\ProgramData".to_string())
        + "\\BeeperAutomations";

    log_to_file(&format!("Creating work directory: {}", work_dir));
    if let Err(e) = std::fs::create_dir_all(&work_dir) {
        let error_msg = format!("Failed to create work directory: {}", e);
        log_to_file(&error_msg);
        log_to_file(&format!("Error details: {:?}", e));
        return Err(windows_service::Error::Winapi(std::io::Error::new(
            std::io::ErrorKind::Other,
            error_msg,
        )));
    }

    log_to_file(&format!("Setting working directory to: {}", work_dir));
    if let Err(e) = std::env::set_current_dir(&work_dir) {
        let error_msg = format!("Failed to set working directory: {}", e);
        log_to_file(&error_msg);
        log_to_file(&format!("Error details: {:?}", e));
        return Err(windows_service::Error::Winapi(std::io::Error::new(
            std::io::ErrorKind::Other,
            error_msg,
        )));
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
    let result = runtime.block_on(async {
        tokio::select! {
            result = beeper_auotmations::run_service_with_shutdown(shutdown_rx) => {
                if let Err(e) = result {
                    log_to_file(&format!("Service error: {}", e));
                    log_to_file(&format!("Error details: {:?}", e));
                }
                log_to_file("run_service_with_shutdown() RETURNED");
            }
        }
    });

    log_to_file(&format!("Service block_on completed with result: {:?}", result));

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
