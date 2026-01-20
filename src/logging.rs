use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use tracing::Subscriber;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer, EnvFilter};

const MAX_LOG_LINES: usize = 1500;

pub static LOG_FILE_PATH: Mutex<Option<String>> = Mutex::new(None);

pub fn log_to_file(msg: &str) {
    let log_path = LOG_FILE_PATH.lock().unwrap().clone().unwrap_or_else(|| {
        std::env::var("PROGRAMDATA").unwrap_or_else(|_| "C:\\ProgramData".to_string())
            + "\\BeeperAutomations\\service.log"
    });

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

pub fn init_logging(windows_service_mode: bool) {
    if windows_service_mode {
        // Set up log file path
        let log_path = std::env::var("PROGRAMDATA")
            .unwrap_or_else(|_| "C:\\ProgramData".to_string())
            + "\\BeeperAutomations\\service.log";

        *LOG_FILE_PATH.lock().unwrap() = Some(log_path.clone());

        // Create a directory if it doesn't exist
        if let Some(parent) = std::path::Path::new(&log_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // Create a custom layer that writes to file
        struct FileLayer;

        impl<S> Layer<S> for FileLayer
        where
            S: Subscriber,
        {
            fn on_event(
                &self,
                event: &tracing::Event<'_>,
                _ctx: tracing_subscriber::layer::Context<'_, S>,
            ) {
                let target = event.metadata().target();
                
                // Filter out notify crate logs to prevent feedback loop
                // (notify detects changes to service.log file itself)
                if target.starts_with("notify") {
                    return;
                }

                let mut message = String::new();
                let mut visitor = |field: &tracing::field::Field, value: &dyn std::fmt::Debug| {
                    use std::fmt::Write;
                    if message.is_empty() {
                        write!(&mut message, "{} = {:?}", field, value).ok();
                    } else {
                        write!(&mut message, ", {} = {:?}", field, value).ok();
                    }
                };

                event.record(&mut visitor);

                let level = event.metadata().level();
                let file = event.metadata().file();
                let line = event.metadata().line();

                let location = if let Some(f) = file {
                    if let Some(l) = line {
                        format!("{}:{}", f, l)
                    } else {
                        f.to_string()
                    }
                } else {
                    String::new()
                };

                if !location.is_empty() {
                    log_to_file(&format!(
                        "[{}] {} ({}) - {}",
                        level, target, location, message
                    ));
                } else {
                    log_to_file(&format!("[{}] {} - {}", level, target, message));
                }
            }
        }

        // Initialize tracing with file layer and filter to exclude notify traces
        let filter = EnvFilter::new("info")
            .add_directive("notify=warn".parse().unwrap())
            .add_directive("beeper_auotmations=trace".parse().unwrap());
        
        tracing_subscriber::registry()
            .with(filter)
            .with(FileLayer)
            .init();

        log_to_file("Tracing initialized for Windows Service mode");
    } else {
        // Initialize tracing with pretty output for console
        tracing_subscriber::fmt().pretty().init();
    }
}
