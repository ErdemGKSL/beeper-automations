use crate::{app_state::SharedAppState, config::Config};
use anyhow::Result;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::io;

pub mod modules;

pub mod config_screen;
pub use config_screen::ConfigScreen;

pub mod main_screen;
pub use main_screen::{MainScreen, MenuOption};

pub mod loading_screen;
pub use loading_screen::show_loading_screen;

/// Initialize the terminal
pub fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore the terminal to its original state
pub fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

/// Show configuration validation screen and get user input
pub fn show_config_screen(config: Config) -> Result<Config> {
    let mut terminal = setup_terminal()?;
    let mut screen = ConfigScreen::new(config);

    let result = screen.run(&mut terminal);
    restore_terminal(&mut terminal)?;

    result
}

/// Show main menu screen and get user selection
pub fn show_main_screen(config: Config) -> Result<Option<MenuOption>> {
    let mut terminal = setup_terminal()?;
    let mut screen = MainScreen::new(config);

    let result = screen.run(&mut terminal);
    restore_terminal(&mut terminal)?;

    result
}

/// Show notification automations screen
pub fn show_notification_screen(app_state: SharedAppState) -> Result<()> {
    let mut terminal = setup_terminal()?;
    let mut screen = modules::NotificationScreen::new(app_state);

    let _ = screen.run(&mut terminal);
    restore_terminal(&mut terminal)?;

    Ok(())
}
