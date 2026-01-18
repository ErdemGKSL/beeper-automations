use anyhow::Result;
use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use std::io;

pub struct LoadingScreen {
    message: String,
    spinner_frame: usize,
}

impl LoadingScreen {
    pub fn new(message: String) -> Self {
        Self {
            message,
            spinner_frame: 0,
        }
    }

    fn get_spinner(&self) -> &'static str {
        match self.spinner_frame % 4 {
            0 => "⠋",
            1 => "⠙",
            2 => "⠹",
            _ => "⠸",
        }
    }

    fn ui(&mut self, f: &mut Frame) {
        let size = f.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(40),
                Constraint::Min(5),
                Constraint::Percentage(40),
            ])
            .split(size);

        let text = vec![Line::from(vec![
            Span::styled(
                format!("{} ", self.get_spinner()),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(self.message.clone(), Style::default().fg(Color::White)),
        ])];

        let loading = Paragraph::new(text).alignment(Alignment::Center);

        f.render_widget(loading, chunks[1]);

        self.spinner_frame += 1;
    }
}

pub async fn show_loading_screen<F, T>(message: &str, future: F) -> Result<T>
where
    F: std::future::Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let mut stdout = io::stdout();
    enable_raw_mode()?;
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut loading = LoadingScreen::new(message.to_string());

    // Spawn the async task
    let task = tokio::spawn(future);

    // Animate loading screen while waiting
    loop {
        terminal.draw(|f| loading.ui(f))?;

        if task.is_finished() {
            break;
        }

        std::thread::sleep(std::time::Duration::from_millis(150));
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // Get the result
    Ok(task.await?)
}
