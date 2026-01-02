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

define_windows_service!(ffi_service_main, service_main);

fn service_main(_arguments: Vec<OsString>) {
    if let Err(e) = run_service() {
        // Log error to Windows Event Log or a file
        eprintln!("Service error: {}", e);
    }
}

fn run_service() -> windows_service::Result<()> {
    // Create a channel to handle service stop events
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);

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

    // Run the service and wait for shutdown signal
    runtime.block_on(async {
        tokio::select! {
            result = beeper_auotmations::run_service() => {
                if let Err(e) = result {
                    eprintln!("Service error: {}", e);
                }
            }
            _ = shutdown_rx.recv() => {
                println!("Received shutdown signal from Windows Service Manager");
            }
        }
    });

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
