use crate::notifications::NotificationAutomation;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
    Frame, Terminal,
};

pub enum ScreenState {
    List,
    EditingAutomation(AutomationForm),
    AddingAutomation(AutomationForm),
    SelectingChats(AutomationForm, ChatSelector),
    ConfiguringLoop(AutomationForm),
}

#[derive(Debug, Clone)]
pub struct ChatSelector {
    pub available_chats: Vec<(String, String)>,  // (id, name) pairs
    pub filter: String,
    pub selected_index: usize,
    pub scroll_offset: usize,  // For scrolling through long lists
    pub loading: bool,
    pub cursor: Option<String>,  // Cursor for pagination
    pub has_more: bool,  // Whether there are more chats to fetch
}

impl ChatSelector {
    fn new() -> Self {
        Self {
            available_chats: Vec::new(),
            filter: String::new(),
            selected_index: 0,
            scroll_offset: 0,
            loading: false,
            cursor: None,
            has_more: true,
        }
    }

    fn filtered_chats(&self) -> Vec<(String, String)> {
        if self.filter.is_empty() {
            self.available_chats.clone()
        } else {
            self.available_chats
                .iter()
                .filter(|(_, name)| name.to_lowercase().contains(&self.filter.to_lowercase()))
                .cloned()
                .collect()
        }
    }
}

#[derive(Debug, Clone)]
pub struct AutomationForm {
    pub id: Option<String>,  // None for new, Some for editing
    pub name: String,
    pub chat_ids: Vec<String>,  // Selected chat IDs
    pub automation_type: crate::notifications::AutomationType,
    pub loop_until: crate::notifications::LoopUntil,
    pub loop_time: String,  // String for input, converted to u64
    pub check_interval: String,  // String for input
    pub notification_sound: String,
    pub focus_chat: bool,
    pub enabled: bool,
    pub selected_field: usize,  // Current field being edited
}

impl AutomationForm {
    fn new() -> Self {
        Self {
            id: None,
            name: String::new(),
            chat_ids: Vec::new(),
            automation_type: crate::notifications::AutomationType::Immediate,
            loop_until: crate::notifications::LoopUntil::MessageSeen,
            loop_time: String::new(),
            check_interval: "3000".to_string(),
            notification_sound: String::new(),
            focus_chat: false,
            enabled: true,
            selected_field: 0,
        }
    }

    fn from_automation(automation: &NotificationAutomation) -> Self {
        let (loop_until, loop_time, check_interval) = if let Some(loop_config) = &automation.loop_config {
            (
                loop_config.until,
                loop_config.time.map(|t| t.to_string()).unwrap_or_default(),
                loop_config.check_interval.to_string(),
            )
        } else {
            (crate::notifications::LoopUntil::MessageSeen, String::new(), "3000".to_string())
        };

        Self {
            id: Some(automation.id.clone()),
            name: automation.name.clone(),
            chat_ids: automation.chat_ids.clone(),
            automation_type: automation.automation_type,
            loop_until,
            loop_time,
            check_interval,
            notification_sound: automation.notification_sound.clone().unwrap_or_default(),
            focus_chat: automation.focus_chat,
            enabled: automation.enabled,
            selected_field: 0,
        }
    }

    fn to_automation(&self) -> NotificationAutomation {
        let loop_config = if self.automation_type == crate::notifications::AutomationType::Loop {
            Some(crate::notifications::LoopConfig {
                until: self.loop_until,
                time: if !self.loop_time.is_empty() {
                    self.loop_time.parse().ok()
                } else {
                    None
                },
                check_interval: self.check_interval.parse().unwrap_or(3000),
            })
        } else {
            None
        };

        NotificationAutomation {
            id: self.id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
            name: self.name.clone(),
            chat_ids: self.chat_ids.clone(),
            automation_type: self.automation_type,
            notification_sound: if !self.notification_sound.is_empty() {
                Some(self.notification_sound.clone())
            } else {
                None
            },
            focus_chat: self.focus_chat,
            loop_config,
            enabled: self.enabled,
        }
    }

    fn field_count(&self) -> usize {
        // Base fields: name, chat_ids, type, sound, focus_chat, enabled
        // Loop configuration is now in a separate screen
        6
    }

    fn loop_field_count(&self) -> usize {
        // Loop fields: loop_until, check_interval, and optionally loop_time
        if self.loop_until == crate::notifications::LoopUntil::ForATime {
            3  // loop_until, loop_time, check_interval
        } else {
            2  // loop_until, check_interval
        }
    }
}

pub struct NotificationScreen {
    app_state: crate::app_state::SharedAppState,
    automations: Vec<NotificationAutomation>,
    selected_index: usize,
    message: String,
    state: ScreenState,
}

impl NotificationScreen {
    pub fn new(app_state: crate::app_state::SharedAppState) -> Self {
        let automations = app_state
            .get_config()
            .map(|c| c.notifications.automations.clone())
            .unwrap_or_default();
        
        Self {
            app_state,
            automations,
            selected_index: 0,
            message: String::new(),
            state: ScreenState::List,
        }
    }

    fn save_to_config(&self) -> Result<()> {
        self.app_state.with_config_mut(|config| {
            config.notifications.automations = self.automations.clone();
        }).map_err(|e| anyhow::anyhow!(e))?;
        
        // Save to disk
        if let Ok(config) = self.app_state.get_config() {
            config.save()?;
        }
        
        Ok(())
    }

    fn load_chats_sync(&self, cursor: Option<String>) -> (Vec<(String, String)>, Option<String>, bool) {
        // Get a handle to the current runtime and spawn a blocking task
        let handle = tokio::runtime::Handle::current();
        
        // Use spawn_blocking to avoid blocking the async runtime
        std::thread::scope(|s| {
            let thread_handle = s.spawn(|| {
                handle.block_on(async {
                    // Fetch one page of chats from Beeper API
                    self.app_state.with_client(|client| {
                        // Create a new runtime for the blocking call
                        tokio::task::block_in_place(|| {
                            handle.block_on(async {
                                match client.list_chats(cursor.as_deref(), None).await {
                                    Ok(response) => {
                                        let chats: Vec<(String, String)> = response.items
                                            .iter()
                                            .map(|chat| (chat.id.clone(), chat.display_name()))
                                            .collect();
                                        
                                        (chats, response.oldest_cursor, response.has_more)
                                    }
                                    Err(_) => (Vec::new(), None, false),
                                }
                            })
                        })
                    }).unwrap_or_else(|_| (Vec::new(), None, false))
                })
            });
            
            thread_handle.join().unwrap()
        })
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<bool> {
        use crossterm::event::{self, Event};

        loop {
            terminal.draw(|f| self.ui(f))?;

            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    if self.handle_key(key)? {
                        return Ok(true);
                    }
                }
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        match &mut self.state {
            ScreenState::List => self.handle_list_key(key),
            ScreenState::EditingAutomation(_) => self.handle_form_key(key),
            ScreenState::AddingAutomation(_) => self.handle_form_key(key),
            ScreenState::SelectingChats(_, _) => self.handle_chat_selector_key(key),
            ScreenState::ConfiguringLoop(_) => self.handle_loop_config_key(key),
        }
    }

    fn handle_list_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => Ok(true),
            KeyCode::Char('n') | KeyCode::Char('N') => {
                // Add new automation
                self.state = ScreenState::AddingAutomation(AutomationForm::new());
                Ok(false)
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                // Delete selected automation
                if !self.automations.is_empty() {
                    let deleted_name = self.automations[self.selected_index].name.clone();
                    self.automations.remove(self.selected_index);
                    
                    // Adjust selected_index if needed
                    if self.selected_index >= self.automations.len() && self.selected_index > 0 {
                        self.selected_index -= 1;
                    }
                    
                    // Save to config
                    if let Err(e) = self.save_to_config() {
                        self.message = format!("Warning: Failed to save config: {}", e);
                    } else {
                        self.message = format!("Deleted automation: {}", deleted_name);
                    }
                }
                Ok(false)
            }
            KeyCode::Up => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                } else if !self.automations.is_empty() {
                    self.selected_index = self.automations.len() - 1;
                }
                Ok(false)
            }
            KeyCode::Down => {
                if !self.automations.is_empty() {
                    self.selected_index = (self.selected_index + 1) % self.automations.len();
                }
                Ok(false)
            }
            KeyCode::Enter => {
                if !self.automations.is_empty() {
                    let form = AutomationForm::from_automation(&self.automations[self.selected_index]);
                    self.state = ScreenState::EditingAutomation(form);
                }
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    fn handle_form_key(&mut self, key: KeyEvent) -> Result<bool> {
        let (form, is_editing) = match self.state {
            ScreenState::EditingAutomation(ref mut f) => (f, true),
            ScreenState::AddingAutomation(ref mut f) => (f, false),
            _ => return Ok(false),
        };

        match key.code {
            KeyCode::Esc => {
                self.state = ScreenState::List;
                self.message.clear();
                Ok(false)
            }
            KeyCode::Enter => {
                // Check if we're on a field that uses Enter for its own purpose
                match form.selected_field {
                    1 => {
                        // Chat selector - open selector instead of saving
                        let form_clone = form.clone();
                        let mut selector = ChatSelector::new();
                        selector.loading = true;
                        
                        let (chats, cursor, has_more) = self.load_chats_sync(None);
                        selector.available_chats = chats;
                        selector.cursor = cursor;
                        selector.has_more = has_more;
                        selector.loading = false;
                        
                        self.state = ScreenState::SelectingChats(form_clone, selector);
                        return Ok(false);
                    }
                    2 if form.automation_type == crate::notifications::AutomationType::Loop => {
                        // Open loop configuration screen
                        let form_clone = form.clone();
                        self.state = ScreenState::ConfiguringLoop(form_clone);
                        return Ok(false);
                    }
                    _ => {}
                }
                
                // Save automation for all other fields
                if form.name.is_empty() {
                    self.message = "Name cannot be empty!".to_string();
                    return Ok(false);
                }

                let automation = form.to_automation();
                
                if is_editing {
                    // Find and update existing automation
                    if let Some(pos) = self.automations.iter().position(|a| a.id == automation.id) {
                        self.automations[pos] = automation;
                        self.message = "Automation updated!".to_string();
                    }
                } else {
                    // Add new automation
                    self.automations.push(automation);
                    self.message = "Automation created!".to_string();
                }
                
                // Save to config
                if let Err(e) = self.save_to_config() {
                    self.message = format!("Warning: Failed to save config: {}", e);
                }
                
                self.state = ScreenState::List;
                Ok(false)
            }
            KeyCode::Tab | KeyCode::Down => {
                form.selected_field = (form.selected_field + 1) % form.field_count();
                Ok(false)
            }
            KeyCode::BackTab | KeyCode::Up => {
                if form.selected_field > 0 {
                    form.selected_field -= 1;
                } else {
                    form.selected_field = form.field_count() - 1;
                }
                Ok(false)
            }
            KeyCode::Char(' ') => {
                // Space to toggle all toggleable fields
                match form.selected_field {
                    2 => {
                        // Toggle automation type
                        form.automation_type = match form.automation_type {
                            crate::notifications::AutomationType::Immediate => crate::notifications::AutomationType::Loop,
                            crate::notifications::AutomationType::Loop => crate::notifications::AutomationType::Immediate,
                        };
                    }
                    4 => form.focus_chat = !form.focus_chat,  // Toggle focus_chat
                    5 => form.enabled = !form.enabled,  // Toggle enabled
                    _ => {}
                }
                Ok(false)
            }
            KeyCode::Backspace => {
                // Handle backspace for text fields
                match form.selected_field {
                    0 => { form.name.pop(); }
                    3 => { form.notification_sound.pop(); }
                    _ => {}
                }
                Ok(false)
            }
            KeyCode::Char(c) => {
                // Handle character input for text fields
                match form.selected_field {
                    0 => form.name.push(c),
                    3 => form.notification_sound.push(c),
                    _ => {}
                }
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    fn handle_chat_selector_key(&mut self, key: KeyEvent) -> Result<bool> {
        let (form, selector) = match self.state {
            ScreenState::SelectingChats(ref mut f, ref mut s) => (f, s),
            _ => return Ok(false),
        };

        match key.code {
            KeyCode::Esc => {
                // Return to form without changes
                let form_clone = form.clone();
                self.state = if form.id.is_some() {
                    ScreenState::EditingAutomation(form_clone)
                } else {
                    ScreenState::AddingAutomation(form_clone)
                };
                Ok(false)
            }
            KeyCode::Enter => {
                // Add selected chat to form
                let filtered = selector.filtered_chats();
                if !filtered.is_empty() && selector.selected_index < filtered.len() {
                    let (chat_id, _) = &filtered[selector.selected_index];
                    if !form.chat_ids.contains(chat_id) {
                        form.chat_ids.push(chat_id.clone());
                    }
                }
                Ok(false)
            }
            KeyCode::Char(' ') | KeyCode::Char('d') | KeyCode::Char('D') => {
                // Remove last added chat (Delete)
                if !form.chat_ids.is_empty() {
                    form.chat_ids.pop();
                }
                Ok(false)
            }
            KeyCode::Up => {
                if selector.selected_index > 0 {
                    selector.selected_index -= 1;
                    // Scroll up if needed
                    if selector.selected_index < selector.scroll_offset {
                        selector.scroll_offset = selector.selected_index;
                    }
                }
                Ok(false)
            }
            KeyCode::Down => {
                let filtered = selector.filtered_chats();
                if !filtered.is_empty() && selector.selected_index < filtered.len() - 1 {
                    selector.selected_index += 1;
                    // Scroll down if needed (visible items calculated in render)
                    // We'll adjust scroll_offset in the render method based on visible height
                }
                
                // Check if we need to load more chats (outside the if to avoid borrow issues)
                let should_load = selector.filter.is_empty() && // Only auto-load when not filtering
                                  selector.has_more && 
                                  !selector.loading && 
                                  selector.selected_index >= selector.available_chats.len().saturating_sub(5);
                
                if should_load {
                    let cursor = selector.cursor.clone();
                    // Temporarily extract selector to avoid borrow issues
                    let (form_temp, mut selector_temp) = match std::mem::replace(&mut self.state, ScreenState::List) {
                        ScreenState::SelectingChats(f, s) => (f, s),
                        other => {
                            self.state = other;
                            return Ok(false);
                        }
                    };
                    
                    selector_temp.loading = true;
                    let (new_chats, new_cursor, has_more) = self.load_chats_sync(cursor);
                    selector_temp.available_chats.extend(new_chats);
                    selector_temp.cursor = new_cursor;
                    selector_temp.has_more = has_more;
                    selector_temp.loading = false;
                    
                    self.state = ScreenState::SelectingChats(form_temp, selector_temp);
                }
                
                Ok(false)
            }
            KeyCode::Backspace => {
                selector.filter.pop();
                selector.selected_index = 0;
                selector.scroll_offset = 0;
                selector.scroll_offset = 0;
                Ok(false)
            }
            KeyCode::Char(c) => {
                selector.filter.push(c);
                selector.selected_index = 0;
                selector.scroll_offset = 0;
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    fn handle_loop_config_key(&mut self, key: KeyEvent) -> Result<bool> {
        let form = match self.state {
            ScreenState::ConfiguringLoop(ref mut f) => f,
            _ => return Ok(false),
        };

        match key.code {
            KeyCode::Esc => {
                // Return to main form
                let form_clone = form.clone();
                self.state = if form.id.is_some() {
                    ScreenState::EditingAutomation(form_clone)
                } else {
                    ScreenState::AddingAutomation(form_clone)
                };
                Ok(false)
            }
            KeyCode::Enter => {
                // Validate: if ForATime is selected, loop_time is required
                if form.loop_until == crate::notifications::LoopUntil::ForATime && form.loop_time.is_empty() {
                    self.message = "Loop Time is required when 'For A Time' is selected!".to_string();
                    return Ok(false);
                }
                
                // Save and return to main form
                let form_clone = form.clone();
                self.state = if form.id.is_some() {
                    ScreenState::EditingAutomation(form_clone)
                } else {
                    ScreenState::AddingAutomation(form_clone)
                };
                self.message = "Loop settings configured!".to_string();
                Ok(false)
            }
            KeyCode::Tab | KeyCode::Down => {
                let max_field = form.loop_field_count();
                form.selected_field = (form.selected_field + 1) % max_field;
                Ok(false)
            }
            KeyCode::BackTab | KeyCode::Up => {
                let max_field = form.loop_field_count();
                if form.selected_field > 0 {
                    form.selected_field -= 1;
                } else {
                    form.selected_field = max_field - 1;
                }
                Ok(false)
            }
            KeyCode::Char(' ') => {
                // Space to toggle loop_until
                if form.selected_field == 0 {
                    form.loop_until = match form.loop_until {
                        crate::notifications::LoopUntil::MessageSeen => crate::notifications::LoopUntil::Answer,
                        crate::notifications::LoopUntil::Answer => crate::notifications::LoopUntil::ForATime,
                        crate::notifications::LoopUntil::ForATime => crate::notifications::LoopUntil::MessageSeen,
                    };
                }
                Ok(false)
            }
            KeyCode::Backspace => {
                // Handle backspace for text fields
                let is_for_time = form.loop_until == crate::notifications::LoopUntil::ForATime;
                match form.selected_field {
                    1 if is_for_time => { form.loop_time.pop(); }
                    2 if is_for_time => { form.check_interval.pop(); }
                    1 if !is_for_time => { form.check_interval.pop(); }
                    _ => {}
                }
                Ok(false)
            }
            KeyCode::Char(c) => {
                // Handle character input for text fields
                let is_for_time = form.loop_until == crate::notifications::LoopUntil::ForATime;
                match form.selected_field {
                    1 if is_for_time => {
                        if c.is_ascii_digit() {
                            form.loop_time.push(c);
                        }
                    }
                    2 if is_for_time => {
                        if c.is_ascii_digit() {
                            form.check_interval.push(c);
                        }
                    }
                    1 if !is_for_time => {
                        if c.is_ascii_digit() {
                            form.check_interval.push(c);
                        }
                    }
                    _ => {}
                }
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    fn ui(&self, f: &mut Frame) {
        let size = f.area();

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
                "Notification Automations",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )]),
            Line::from(""),
        ]);
        f.render_widget(header, chunks[0]);

        // Content based on state
        match &self.state {
            ScreenState::List => {
                self.render_automation_list(f, chunks[1]);
            }
            ScreenState::EditingAutomation(form) => {
                self.render_form(f, size, form, "Edit Automation");
            }
            ScreenState::AddingAutomation(form) => {
                self.render_form(f, size, form, "New Automation");
            }
            ScreenState::SelectingChats(form, selector) => {
                self.render_chat_selector(f, size, form, selector);
            }
            ScreenState::ConfiguringLoop(form) => {
                self.render_loop_config(f, size, form);
            }
        }

        // Footer
        let footer_text = if !self.message.is_empty() {
            self.message.clone()
        } else {
            match &self.state {
                ScreenState::List => "↑↓: Navigate | N: New | Enter: Edit | D: Delete | Q/Esc: Back".to_string(),
                ScreenState::EditingAutomation(_) => {
                    "Tab/↑↓: Navigate | Space: Toggle | Enter: Save/Configure | Esc: Cancel".to_string()
                }
                ScreenState::AddingAutomation(_) => {
                    "Tab/↑↓: Navigate | Space: Toggle | Enter: Save/Configure | Esc: Cancel".to_string()
                }
                ScreenState::SelectingChats(_, _) => {
                    "↑↓: Navigate | Enter: Add | D: Remove Last | Type to filter | Esc: Back".to_string()
                }
                ScreenState::ConfiguringLoop(_) => {
                    "Tab/↑↓: Navigate | Space: Toggle | Enter: Done | Esc: Cancel".to_string()
                }
            }
        };

        let footer = Paragraph::new(footer_text).style(Style::default().fg(Color::Gray));
        f.render_widget(footer, chunks[2]);
    }

    fn render_automation_list(&self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .automations
            .iter()
            .enumerate()
            .map(|(idx, automation)| {
                let is_selected = idx == self.selected_index;
                let enabled_status = if automation.enabled { "✓" } else { "✗" };
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };

                let label = format!(
                    "  [{}] {} ({} - {} chats)",
                    enabled_status,
                    automation.name,
                    automation.automation_type,
                    automation.chat_ids.len()
                );

                ListItem::new(Span::styled(label, style))
            })
            .collect();

        let list = if items.is_empty() {
            List::new(vec![ListItem::new(
                Span::styled(
                    "No automations configured",
                    Style::default().fg(Color::DarkGray),
                )
            )])
        } else {
            List::new(items)
        };

        let list = list
            .block(
                Block::default()
                    .title("Automations")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            );

        f.render_widget(list, area);
    }

    fn render_form(&self, f: &mut Frame, area: Rect, form: &AutomationForm, title: &str) {
        use ratatui::widgets::Clear;

        // Calculate modal size (centered, about 70% of screen width and height)
        let modal_width = std::cmp::min((area.width as usize * 70) / 100, 80);
        let modal_height = std::cmp::min((area.height as usize * 80) / 100, 25);

        let modal_x = (area.width as usize - modal_width) / 2;
        let modal_y = (area.height as usize - modal_height) / 2;

        let modal_area = Rect {
            x: modal_x as u16,
            y: modal_y as u16,
            width: modal_width as u16,
            height: modal_height as u16,
        };

        // Draw background overlay
        f.render_widget(Clear, modal_area);
        let modal_block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        f.render_widget(modal_block, modal_area);

        // Create form content area
        let inner_area = Rect {
            x: modal_area.x + 2,
            y: modal_area.y + 2,
            width: modal_area.width.saturating_sub(4),
            height: modal_area.height.saturating_sub(4),
        };

        // All forms have the same 6 base fields
        let field_constraints = vec![
            Constraint::Length(3), // 0: Name
            Constraint::Length(3), // 1: Chat IDs
            Constraint::Length(3), // 2: Type (with config button for Loop)
            Constraint::Length(3), // 3: Sound
            Constraint::Length(3), // 4: Focus Chat
            Constraint::Length(3), // 5: Enabled
            Constraint::Min(1),    // Spacer
        ];

        let form_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(field_constraints)
            .split(inner_area);

        // Field 0: Name
        self.render_text_field(f, form_chunks[0], "Name", &form.name, form.selected_field == 0);

        // Field 1: Chat IDs (selector button)
        let chat_display = if form.chat_ids.is_empty() {
            "No chats selected (Press Enter to select)".to_string()
        } else {
            format!("{} chat(s) selected (Press Enter to modify)", form.chat_ids.len())
        };
        self.render_enum_field(f, form_chunks[1], "Chats", &chat_display, form.selected_field == 1);

        // Field 2: Automation Type (with Loop config button)
        let type_display = if form.automation_type == crate::notifications::AutomationType::Loop {
            format!("{} (Press Enter to configure loop)", form.automation_type)
        } else {
            format!("{}", form.automation_type)
        };
        self.render_enum_field(f, form_chunks[2], "Type", &type_display, form.selected_field == 2);

        // Field 3: Notification Sound
        self.render_text_field(f, form_chunks[3], "Sound (optional)", &form.notification_sound, form.selected_field == 3);

        // Field 4: Focus Chat
        self.render_bool_field(f, form_chunks[4], "Focus Chat", form.focus_chat, form.selected_field == 4);

        // Field 5: Enabled
        self.render_bool_field(f, form_chunks[5], "Enabled", form.enabled, form.selected_field == 5);
    }

    fn render_text_field(&self, f: &mut Frame, area: Rect, label: &str, value: &str, selected: bool) {
        let display = if value.is_empty() { "_" } else { value };
        let style = if selected {
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let border_style = if selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Gray)
        };

        let block = Block::default()
            .title(label)
            .borders(Borders::ALL)
            .border_style(border_style);
        let paragraph = Paragraph::new(display).block(block).style(style);
        f.render_widget(paragraph, area);
    }

    fn render_enum_field(&self, f: &mut Frame, area: Rect, label: &str, value: &str, selected: bool) {
        let style = if selected {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let border_style = if selected {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::Gray)
        };

        let block = Block::default()
            .title(label)
            .borders(Borders::ALL)
            .border_style(border_style);
        let paragraph = Paragraph::new(value).block(block).style(style);
        f.render_widget(paragraph, area);
    }

    fn render_bool_field(&self, f: &mut Frame, area: Rect, label: &str, value: bool, selected: bool) {
        let display = if value { "✓ Yes" } else { "✗ No" };
        let style = if selected {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        let border_style = if selected {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::Gray)
        };

        let block = Block::default()
            .title(label)
            .borders(Borders::ALL)
            .border_style(border_style);
        let paragraph = Paragraph::new(display).block(block).style(style);
        f.render_widget(paragraph, area);
    }

    fn render_chat_selector(&self, f: &mut Frame, area: Rect, form: &AutomationForm, selector: &ChatSelector) {
        use ratatui::widgets::Clear;

        // Calculate modal size
        let modal_width = std::cmp::min((area.width as usize * 70) / 100, 80);
        let modal_height = std::cmp::min((area.height as usize * 80) / 100, 25);

        let modal_x = (area.width as usize - modal_width) / 2;
        let modal_y = (area.height as usize - modal_height) / 2;

        let modal_area = Rect {
            x: modal_x as u16,
            y: modal_y as u16,
            width: modal_width as u16,
            height: modal_height as u16,
        };

        // Draw background
        f.render_widget(Clear, modal_area);
        let modal_block = Block::default()
            .title("Select Chats")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));
        f.render_widget(modal_block, modal_area);

        // Split modal into sections
        let inner_area = Rect {
            x: modal_area.x + 2,
            y: modal_area.y + 2,
            width: modal_area.width.saturating_sub(4),
            height: modal_area.height.saturating_sub(4),
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Filter input
                Constraint::Length(3), // Selected chats display
                Constraint::Min(5),    // Available chats list
            ])
            .split(inner_area);

        // Filter input
        let filter_display = if selector.filter.is_empty() {
            "Type to filter...".to_string()
        } else {
            selector.filter.clone()
        };
        let filter_block = Block::default()
            .title("Filter")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));
        let filter = Paragraph::new(filter_display)
            .block(filter_block)
            .style(Style::default().fg(Color::Yellow));
        f.render_widget(filter, chunks[0]);

        // Selected chats
        let selected_text = if form.chat_ids.is_empty() {
            "No chats selected yet".to_string()
        } else {
            format!("Selected: {} chat(s)", form.chat_ids.len())
        };
        let selected_block = Block::default()
            .title("Selected Chats")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green));
        let selected = Paragraph::new(selected_text)
            .block(selected_block)
            .style(Style::default().fg(Color::Green));
        f.render_widget(selected, chunks[1]);

        // Available chats list with scrolling
        let filtered = selector.filtered_chats();
        
        // Calculate visible window size (account for borders)
        let visible_height = chunks[2].height.saturating_sub(2) as usize;
        
        // Adjust scroll offset to keep selected item visible
        let mut scroll_offset = selector.scroll_offset;
        if selector.selected_index >= scroll_offset + visible_height {
            scroll_offset = selector.selected_index.saturating_sub(visible_height - 1);
        } else if selector.selected_index < scroll_offset {
            scroll_offset = selector.selected_index;
        }
        
        // Get visible slice of items
        let visible_end = std::cmp::min(scroll_offset + visible_height, filtered.len());
        let visible_items = &filtered[scroll_offset..visible_end];
        
        let items: Vec<ListItem> = visible_items
            .iter()
            .enumerate()
            .map(|(visible_idx, (id, name))| {
                let actual_idx = scroll_offset + visible_idx;
                let is_selected = actual_idx == selector.selected_index;
                let is_added = form.chat_ids.contains(id);
                let prefix = if is_added { "✓ " } else { "  " };
                
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else if is_added {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::White)
                };

                let label = format!("{}{}", prefix, name);
                ListItem::new(Span::styled(label, style))
            })
            .collect();

        let list = if items.is_empty() {
            if selector.loading {
                List::new(vec![ListItem::new(Span::styled(
                    "Loading chats...",
                    Style::default().fg(Color::Yellow),
                ))])
            } else {
                List::new(vec![ListItem::new(Span::styled(
                    "No chats found",
                    Style::default().fg(Color::DarkGray),
                ))])
            }
        } else {
            List::new(items)
        };

        let title = if !filtered.is_empty() {
            format!(
                "Available Chats ({}/{})",
                selector.selected_index + 1,
                filtered.len()
            )
        } else {
            "Available Chats".to_string()
        };

        let list = list.block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        );

        f.render_widget(list, chunks[2]);
    }

    fn render_loop_config(&self, f: &mut Frame, size: Rect, form: &AutomationForm) {
        // Calculate modal dimensions (smaller than main form)
        let modal_width = (size.width as f32 * 0.6).max(40.0) as usize;
        let modal_height = 16; // Fixed height for 3 fields
        let modal_x = (size.width as usize - modal_width) / 2;
        let modal_y = (size.height as usize - modal_height) / 2;

        let modal_area = Rect {
            x: modal_x as u16,
            y: modal_y as u16,
            width: modal_width as u16,
            height: modal_height as u16,
        };

        // Draw background overlay
        f.render_widget(Clear, modal_area);
        let modal_block = Block::default()
            .title("Loop Configuration")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta));
        f.render_widget(modal_block, modal_area);

        // Create form content area
        let inner_area = Rect {
            x: modal_area.x + 2,
            y: modal_area.y + 2,
            width: modal_area.width.saturating_sub(4),
            height: modal_area.height.saturating_sub(4),
        };

        let is_for_time = form.loop_until == crate::notifications::LoopUntil::ForATime;
        
        let mut field_constraints = vec![
            Constraint::Length(3), // 0: Loop Until
        ];
        
        if is_for_time {
            field_constraints.push(Constraint::Length(3)); // 1: Loop Time (only for ForATime)
        }
        
        field_constraints.push(Constraint::Length(3)); // Check Interval
        field_constraints.push(Constraint::Min(1));    // Spacer

        let form_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(field_constraints)
            .split(inner_area);

        // Field 0: Loop Until
        self.render_enum_field(f, form_chunks[0], "Loop Until", &format!("{}", form.loop_until), form.selected_field == 0);

        let mut chunk_idx = 1;
        
        // Field 1: Loop Time (only shown for ForATime)
        if is_for_time {
            self.render_text_field(f, form_chunks[chunk_idx], "Loop Time (ms) *required*", &form.loop_time, form.selected_field == 1);
            chunk_idx += 1;
        }

        // Check Interval (field 1 or 2 depending on is_for_time)
        let check_interval_field_idx = if is_for_time { 2 } else { 1 };
        self.render_text_field(f, form_chunks[chunk_idx], "Check Interval (ms)", &form.check_interval, form.selected_field == check_interval_field_idx);
    }
}
