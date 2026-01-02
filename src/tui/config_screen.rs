use crate::config::Config;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    backend::Backend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};

/// Application state for the TUI
#[derive(Debug, Clone, Copy)]
enum InputField {
    Url,
    Token,
}

pub struct ConfigScreen {
    config: Config,
    active_field: InputField,
    url_input: String,
    token_input: String,
    message: String,
}

impl ConfigScreen {
    pub fn new(config: Config) -> Self {
        let url_input = config.api.url.clone();
        let token_input = config.api.token.clone();

        Self {
            config,
            active_field: InputField::Url,
            url_input,
            token_input,
            message: String::new(),
        }
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<Config> {
        use crossterm::event::{self, Event};

        loop {
            terminal.draw(|f| self.ui(f))?;

            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if self.handle_key(key) {
                        break;
                    }
                }
            }
        }

        // Update config with new values
        self.config.api.url = self.url_input.clone();
        self.config.api.token = self.token_input.clone();

        // Save configuration
        self.config.save()?;
        self.message = "Configuration saved!".to_string();

        // Display save message for a moment
        terminal.draw(|f| self.ui(f))?;

        Ok(self.config.clone())
    }

    fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Tab => {
                self.active_field = match self.active_field {
                    InputField::Url => InputField::Token,
                    InputField::Token => InputField::Url,
                };
                false
            }
            KeyCode::Backspace => {
                match self.active_field {
                    InputField::Url => {
                        self.url_input.pop();
                    }
                    InputField::Token => {
                        self.token_input.pop();
                    }
                }
                self.message.clear();
                false
            }
            KeyCode::Char(c) => {
                match self.active_field {
                    InputField::Url => {
                        self.url_input.push(c);
                    }
                    InputField::Token => {
                        self.token_input.push(c);
                    }
                }
                self.message.clear();
                false
            }
            KeyCode::Enter => {
                if !self.url_input.is_empty() && !self.token_input.is_empty() {
                    true
                } else {
                    self.message = "Please fill in both URL and token".to_string();
                    false
                }
            }
            KeyCode::Esc => {
                self.message = "Configuration cancelled".to_string();
                true
            }
            _ => false,
        }
    }

    fn ui(&self, f: &mut Frame) {
        let size = f.area();

        // Main vertical layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(3),
                    Constraint::Min(10),
                    Constraint::Length(3),
                ]
                .as_ref(),
            )
            .split(size);

        // Header
        let header = Paragraph::new(vec![
            Line::from(vec![Span::styled(
                "Beeper Automations Configuration",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
        ]);
        f.render_widget(header, chunks[0]);

        // Form area
        let form_area = chunks[1];
        let form_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Length(6),
                    Constraint::Length(6),
                    Constraint::Min(0),
                ]
                .as_ref(),
            )
            .split(form_area);

        // URL Input
        self.render_input_field(
            f,
            form_chunks[0],
            "Beeper Desktop API URL",
            &self.url_input,
            matches!(self.active_field, InputField::Url),
        );

        // Token Input
        self.render_input_field(
            f,
            form_chunks[1],
            "API Token",
            &self.token_input,
            matches!(self.active_field, InputField::Token),
        );

        // Message or help text area
        let message_text = if !self.message.is_empty() {
            self.message.clone()
        } else {
            "Tab: Switch field | Enter: Save | Esc: Cancel".to_string()
        };

        let message_style = if self.message.contains("saved") {
            Style::default().fg(Color::Green)
        } else if self.message.contains("cancelled") || self.message.contains("fill") {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let help = Paragraph::new(message_text).style(message_style);
        f.render_widget(help, chunks[2]);
    }

    fn render_input_field(&self, f: &mut Frame, area: Rect, label: &str, value: &str, active: bool) {
        let border_color = if active { Color::Cyan } else { Color::White };
        let style = if active {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        let block = Block::default()
            .title(label)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let display_value = if active && value.is_empty() {
            "_".to_string()
        } else {
            value.to_string()
        };

        let content = Paragraph::new(display_value)
            .block(block)
            .style(style)
            .alignment(Alignment::Left);

        f.render_widget(content, area);
    }
}
