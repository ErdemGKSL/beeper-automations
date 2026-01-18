use crate::config::Config;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    Frame, Terminal,
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuOption {
    Module(usize),
    ChangeConfiguration,
    Exit,
}

pub struct MainScreen {
    _config: Config,
    selected_index: usize,
    modules: Vec<String>,
    message: String,
}

impl MainScreen {
    pub fn new(config: Config) -> Self {
        let modules = vec![
            "Notification Manager".to_string(),
            "Auto Response".to_string(),
        ];

        Self {
            _config: config,
            selected_index: 0,
            modules,
            message: String::new(),
        }
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<Option<MenuOption>> {
        use crossterm::event::{self, Event};

        loop {
            terminal.draw(|f| self.ui(f))?;

            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if let Some(choice) = self.handle_key(key) {
                        return Ok(Some(choice));
                    }
                }
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<MenuOption> {
        match key.code {
            KeyCode::Up => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                } else {
                    self.selected_index = self.total_items() - 1;
                }
                self.message.clear();
                None
            }
            KeyCode::Down => {
                self.selected_index = (self.selected_index + 1) % self.total_items();
                self.message.clear();
                None
            }
            KeyCode::Enter => {
                let choice = self.get_selected_option();
                self.message = match choice {
                    MenuOption::Module(idx) => format!("Selected: {}", self.modules[idx]),
                    MenuOption::ChangeConfiguration => "Opening configuration...".to_string(),
                    MenuOption::Exit => "Exiting...".to_string(),
                };
                Some(choice)
            }
            KeyCode::Esc | KeyCode::Char('q') => Some(MenuOption::Exit),
            _ => None,
        }
    }

    fn total_items(&self) -> usize {
        self.modules.len() + 2 // modules + "Change Configuration" + "Exit"
    }

    fn get_selected_option(&self) -> MenuOption {
        if self.selected_index < self.modules.len() {
            MenuOption::Module(self.selected_index)
        } else if self.selected_index == self.modules.len() {
            MenuOption::ChangeConfiguration
        } else {
            MenuOption::Exit
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
                "Beeper Automations",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::styled(
                "Main Menu",
                Style::default().fg(Color::Gray),
            )]),
        ]);
        f.render_widget(header, chunks[0]);

        // Menu area
        let menu_area = chunks[1];
        self.render_menu(f, menu_area);

        // Footer with help text
        let footer_text = if !self.message.is_empty() {
            self.message.clone()
        } else {
            "↑↓: Navigate | Enter: Select | Q/Esc: Exit".to_string()
        };

        let footer_style = if self.message.contains("Selected")
            || self.message.contains("Opening")
            || self.message.contains("Exiting")
        {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Gray)
        };

        let footer = Paragraph::new(footer_text).style(footer_style);
        f.render_widget(footer, chunks[2]);
    }

    fn render_menu(&self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .modules
            .iter()
            .enumerate()
            .map(|(idx, module)| {
                let is_selected = idx == self.selected_index;
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(Span::styled(format!("  {} ", module), style))
            })
            .chain(
                std::iter::once({
                    let is_selected = self.selected_index == self.modules.len();
                    let style = if is_selected {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Cyan)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    };
                    ListItem::new(Span::styled("  Change Connection Configuration", style))
                })
                .into_iter(),
            )
            .chain(
                std::iter::once({
                    let is_selected = self.selected_index == self.modules.len() + 1;
                    let style = if is_selected {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Red)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::Yellow)
                    };
                    ListItem::new(Span::styled("  Exit", style))
                })
                .into_iter(),
            )
            .collect();

        let list = List::new(items).block(
            Block::default()
                .title("Available Options")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

        f.render_widget(list, area);
    }
}
