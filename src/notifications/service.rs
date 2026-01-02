// Service logic for notification automations will be implemented here

use crate::app_state::SharedAppState;
use crate::notifications::models::{NotificationAutomation, AutomationType};
use crate::config::Config;
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use std::path::Path;

/// Play a sound file (supports .wav and .mp3)
fn play_sound(sound_path: &str) {
    use rodio::{Decoder, OutputStream, Sink};
    use std::fs::File;
    use std::io::BufReader;
    
    let path = Path::new(sound_path);
    
    // If relative path, try to resolve from common locations
    let resolved_path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        // Try current directory first
        if Path::new(sound_path).exists() {
            Path::new(sound_path).to_path_buf()
        } else {
            // Try in ProgramData/BeeperAutomations/sounds
            let program_data = std::env::var("PROGRAMDATA")
                .unwrap_or_else(|_| "C:\\ProgramData".to_string());
            let sounds_dir = Path::new(&program_data).join("BeeperAutomations").join("sounds");
            sounds_dir.join(sound_path)
        }
    };
    
    if !resolved_path.exists() {
        eprintln!("Sound file not found: {:?}", resolved_path);
        return;
    }
    
    // Spawn a thread to play sound asynchronously
    let resolved_path = resolved_path.clone();
    std::thread::spawn(move || {
        match File::open(&resolved_path) {
            Ok(file) => {
                let buf_reader = BufReader::new(file);
                match Decoder::new(buf_reader) {
                    Ok(source) => {
                        // Create output stream and sink
                        match OutputStream::try_default() {
                            Ok((_stream, stream_handle)) => {
                                match Sink::try_new(&stream_handle) {
                                    Ok(sink) => {
                                        sink.append(source);
                                        sink.sleep_until_end();
                                    }
                                    Err(e) => eprintln!("Failed to create audio sink: {}", e),
                                }
                            }
                            Err(e) => eprintln!("Failed to create audio output stream: {}", e),
                        }
                    }
                    Err(e) => eprintln!("Failed to decode sound file: {}", e),
                }
            }
            Err(e) => eprintln!("Failed to open sound file {:?}: {}", resolved_path, e),
        }
    });
}

#[allow(unused)]
#[derive(Debug, Clone)]
struct LastMessageCache {
    message_id: String,
    sort_key: String,
    notification_start_time: Option<std::time::Instant>,
}

#[derive(Debug)]
struct AutomationTask {
    automation_id: String,
    handle: JoinHandle<()>,
}

#[allow(dead_code)]
pub struct NotificationService {
    app_state: SharedAppState,
    automation_tasks: Arc<RwLock<Vec<AutomationTask>>>,
    last_messages: Arc<RwLock<HashMap<String, LastMessageCache>>>,
    reload_rx: Arc<RwLock<tokio::sync::mpsc::Receiver<Config>>>,
}

impl Drop for NotificationService {
    fn drop(&mut self) {
        // Cancel all running tasks when service is dropped
        if let Ok(tasks) = self.automation_tasks.try_read() {
            for task in tasks.iter() {
                task.handle.abort();
            }
        }
    }
}

impl NotificationService {
    pub fn new(
        app_state: SharedAppState,
        reload_rx: tokio::sync::mpsc::Receiver<Config>,
    ) -> Self {
        let last_messages = Arc::new(RwLock::new(HashMap::new()));
        let reload_rx = Arc::new(RwLock::new(reload_rx));
        
        let service = Self {
            app_state: app_state.clone(),
            automation_tasks: Arc::new(RwLock::new(Vec::new())),
            last_messages: last_messages.clone(),
            reload_rx: reload_rx.clone(),
        };
        
        // Start automation loops based on config
        tokio::spawn({
            let app_state = app_state.clone();
            let automation_tasks = service.automation_tasks.clone();
            let last_messages = last_messages.clone();
            let reload_rx = reload_rx.clone();
            
            async move {
                Self::run_service(app_state, automation_tasks, last_messages, reload_rx).await;
            }
        });
        
        service
    }
    
    async fn run_service(
        app_state: SharedAppState,
        automation_tasks: Arc<RwLock<Vec<AutomationTask>>>,
        last_messages: Arc<RwLock<HashMap<String, LastMessageCache>>>,
        reload_rx: Arc<RwLock<tokio::sync::mpsc::Receiver<Config>>>,
    ) {
        // Listen for config reload signals (including initial config)
        loop {
            let new_config = {
                let mut rx = reload_rx.write().await;
                rx.recv().await
            };
            
            match new_config {
                Some(config) => {
                    println!("\n🔄 Hot reloading automations...");
                    Self::handle_config_reload(&app_state, &automation_tasks, &last_messages, config).await;
                    println!("✓ Hot reload complete.\n");
                }
                None => {
                    println!("Config reload channel closed, stopping service.");
                    break;
                }
            }
        }
    }
    
    async fn handle_config_reload(
        app_state: &SharedAppState,
        automation_tasks: &Arc<RwLock<Vec<AutomationTask>>>,
        last_messages: &Arc<RwLock<HashMap<String, LastMessageCache>>>,
        new_config: Config,
    ) {
        // Update app state with new config
        if let Err(e) = app_state.update_config(new_config.clone()) {
            eprintln!("Error updating app state: {}", e);
            return;
        }
        
        let old_tasks = automation_tasks.read().await;
        let old_automation_ids: Vec<String> = old_tasks.iter()
            .map(|t| t.automation_id.clone())
            .collect();
        drop(old_tasks);
        
        // Build map of new automations
        let new_automations: HashMap<String, &NotificationAutomation> = new_config
            .notifications
            .automations
            .iter()
            .filter(|a| a.enabled)
            .map(|a| (a.id.clone(), a))
            .collect();
        
        let new_automation_ids: Vec<String> = new_automations.keys().cloned().collect();
        
        // Determine what changed
        let to_stop: Vec<String> = old_automation_ids
            .iter()
            .filter(|id| !new_automation_ids.contains(id))
            .cloned()
            .collect();
        
        let to_start: Vec<String> = new_automation_ids
            .iter()
            .filter(|id| !old_automation_ids.contains(id))
            .cloned()
            .collect();
        
        // For simplicity, restart ALL existing automations since they might have changed
        // This ensures config changes like changing loop conditions are applied
        let to_restart: Vec<String> = new_automation_ids
            .iter()
            .filter(|id| old_automation_ids.contains(id))
            .cloned()
            .collect();
        
        // Stop removed/disabled automations
        if !to_stop.is_empty() {
            println!("  Stopping {} automation(s)...", to_stop.len());
            let mut tasks = automation_tasks.write().await;
            tasks.retain(|task| {
                if to_stop.contains(&task.automation_id) {
                    println!("    ✗ Stopping automation: {}", task.automation_id);
                    task.handle.abort();
                    false
                } else {
                    true
                }
            });
        }
        
        // Restart modified automations
        if !to_restart.is_empty() {
            println!("  Restarting {} modified automation(s)...", to_restart.len());
            let mut tasks = automation_tasks.write().await;
            
            // Stop the old versions
            tasks.retain(|task| {
                if to_restart.contains(&task.automation_id) {
                    println!("    ↻ Restarting automation: {}", task.automation_id);
                    task.handle.abort();
                    false
                } else {
                    true
                }
            });
            
            // Start the new versions
            for automation_id in &to_restart {
                if let Some(automation) = new_automations.get(automation_id) {
                    match automation.automation_type {
                        AutomationType::Loop => {
                            let handle = Self::start_loop_automation_static(
                                app_state.clone(),
                                (*automation).clone(),
                            );
                            tasks.push(AutomationTask {
                                automation_id: automation_id.clone(),
                                handle,
                            });
                        }
                        AutomationType::Immediate => {
                            // Will be handled by immediate watcher restart below
                        }
                    }
                }
            }
        }
        
        // Start new automations
        if !to_start.is_empty() {
            println!("  Starting {} new automation(s)...", to_start.len());
            let mut tasks = automation_tasks.write().await;
            
            // Collect immediate automation chat IDs
            let mut immediate_chat_ids = Vec::new();
            
            for automation_id in &to_start {
                if let Some(automation) = new_automations.get(automation_id) {
                    match automation.automation_type {
                        AutomationType::Loop => {
                            println!("    ✓ Starting loop automation: {}", automation.name);
                            let handle = Self::start_loop_automation_static(
                                app_state.clone(),
                                (*automation).clone(),
                            );
                            tasks.push(AutomationTask {
                                automation_id: automation_id.clone(),
                                handle,
                            });
                        }
                        AutomationType::Immediate => {
                            immediate_chat_ids.extend(automation.chat_ids.clone());
                        }
                    }
                }
            }
            
            // Start immediate watcher if needed
            if !immediate_chat_ids.is_empty() {
                immediate_chat_ids.sort();
                immediate_chat_ids.dedup();
                
                // Check if we already have an immediate watcher
                let has_immediate = tasks.iter().any(|t| t.automation_id == "__immediate_watcher__");
                
                if has_immediate {
                    // Stop old immediate watcher and start new one
                    println!("    ↻ Restarting immediate automation watcher");
                    tasks.retain(|task| {
                        if task.automation_id == "__immediate_watcher__" {
                            task.handle.abort();
                            false
                        } else {
                            true
                        }
                    });
                } else {
                    println!("    ✓ Starting immediate automation watcher");
                }
                
                let handle = Self::start_immediate_watcher_static(
                    app_state.clone(),
                    last_messages.clone(),
                    immediate_chat_ids,
                );
                tasks.push(AutomationTask {
                    automation_id: "__immediate_watcher__".to_string(),
                    handle,
                });
            }
        }
        
        // Check if we need to handle immediate automations for restarted ones
        let all_immediate_chat_ids: Vec<String> = new_config
            .notifications
            .automations
            .iter()
            .filter(|a| a.enabled && a.automation_type == AutomationType::Immediate)
            .flat_map(|a| a.chat_ids.clone())
            .collect();
        
        if !all_immediate_chat_ids.is_empty() {
            let mut immediate_chat_ids = all_immediate_chat_ids;
            immediate_chat_ids.sort();
            immediate_chat_ids.dedup();
            
            let mut tasks = automation_tasks.write().await;
            let has_immediate = tasks.iter().any(|t| t.automation_id == "__immediate_watcher__");
            
            if has_immediate && to_restart.iter().any(|id| {
                new_automations.get(id).map(|a| a.automation_type == AutomationType::Immediate).unwrap_or(false)
            }) {
                // Restart immediate watcher if any immediate automation was modified
                println!("  Restarting immediate watcher due to modified immediate automation");
                tasks.retain(|task| {
                    if task.automation_id == "__immediate_watcher__" {
                        task.handle.abort();
                        false
                    } else {
                        true
                    }
                });
                
                let handle = Self::start_immediate_watcher_static(
                    app_state.clone(),
                    last_messages.clone(),
                    immediate_chat_ids,
                );
                tasks.push(AutomationTask {
                    automation_id: "__immediate_watcher__".to_string(),
                    handle,
                });
            }
        }
        
        // Clean up message cache for chats no longer being tracked
        let all_tracked_chat_ids: Vec<String> = new_config
            .notifications
            .automations
            .iter()
            .filter(|a| a.enabled)
            .flat_map(|a| a.chat_ids.clone())
            .collect();
        
        let mut cache = last_messages.write().await;
        cache.retain(|chat_id, _| all_tracked_chat_ids.contains(chat_id));
    }
    
    fn start_immediate_watcher_static(
        app_state: SharedAppState,
        last_messages: Arc<RwLock<HashMap<String, LastMessageCache>>>,
        chat_ids: Vec<String>,
    ) -> JoinHandle<()> {
        
        tokio::spawn(async move {
            println!("Starting immediate automation watcher for {} chats", chat_ids.len());
            
            loop {
                // Check each chat for new messages
                for chat_id in &chat_ids {
                    // Fetch latest message for this chat
                    let result = app_state.with_client(|client| {
                        tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async {
                                client.list_messages(chat_id, None, None).await
                            })
                        })
                    });
                    
                    match result {
                        Ok(Ok(messages_response)) => {
                            if let Some(latest_message) = messages_response.items.first() {
                                let mut cache = last_messages.write().await;
                                
                                // Check if this is a new message
                                if let Some(cached) = cache.get(chat_id) {
                                    if cached.sort_key < latest_message.sort_key {
                                        println!("New message detected in chat {}: {}", chat_id, latest_message.id);
                                        
                                        // Update cache
                                        cache.insert(chat_id.clone(), LastMessageCache {
                                            message_id: latest_message.id.clone(),
                                            sort_key: latest_message.sort_key.clone(),
                                            notification_start_time: None,
                                        });
                                        
                                        // Drop the lock before async operations
                                        drop(cache);
                                        
                                        // Get the automation config for this chat
                                        if let Ok(config) = app_state.get_config() {
                                            for automation in &config.notifications.automations {
                                                if automation.enabled && 
                                                   automation.automation_type == AutomationType::Immediate &&
                                                   automation.chat_ids.contains(chat_id) {
                                                    // Trigger focus action
                                                    if automation.focus_chat {
                                                        let result = app_state.with_client(|client| {
                                                            tokio::task::block_in_place(|| {
                                                                tokio::runtime::Handle::current().block_on(async {
                                                                    use beeper_desktop_api::FocusAppInput;
                                                                    
                                                                    let focus_input = FocusAppInput {
                                                                        chat_id: Some(chat_id.clone()),
                                                                        message_id: None,
                                                                        draft: None,
                                                                    };
                                                                    
                                                                    client.focus_app(Some(focus_input)).await
                                                                })
                                                            })
                                                        });
                                                        
                                                        match result {
                                                            Ok(Ok(response)) => {
                                                                if response.success {
                                                                    println!("✓ Focused chat {} for automation '{}'", chat_id, automation.name);
                                                                }
                                                            }
                                                            Ok(Err(e)) => {
                                                                eprintln!("Error focusing chat {}: {}", chat_id, e);
                                                            }
                                                            Err(e) => {
                                                                eprintln!("Error accessing client for focus: {}", e);
                                                            }
                                                        }
                                                    }
                                                    
                                                    // Trigger notification sound if configured
                                                    if let Some(sound_path) = &automation.notification_sound {
                                                        if !sound_path.is_empty() {
                                                            println!("▶ Playing notification sound: {}", sound_path);
                                                            play_sound(sound_path);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                } else {
                                    // First time seeing this chat, initialize cache
                                    cache.insert(chat_id.clone(), LastMessageCache {
                                        message_id: latest_message.id.clone(),
                                        sort_key: latest_message.sort_key.clone(),
                                        notification_start_time: None,
                                    });
                                    println!("Initialized cache for chat {}", chat_id);
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            eprintln!("Error fetching messages for chat {}: {}", chat_id, e);
                        }
                        Err(e) => {
                            eprintln!("Error accessing client for chat {}: {}", chat_id, e);
                        }
                    }
                }
                
                // Wait 3 seconds before next check
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
            }
        })
    }
    
    fn start_loop_automation_static(
        app_state: SharedAppState,
        automation: NotificationAutomation,
    ) -> JoinHandle<()> {
        
        tokio::spawn(async move {
            use crate::notifications::models::LoopUntil;
            use std::collections::HashMap;
            
            println!("Starting loop automation: {} (ID: {})", automation.name, automation.id);
            
            let loop_config = match &automation.loop_config {
                Some(config) => config,
                None => {
                    eprintln!("Loop automation {} has no loop config!", automation.id);
                    return;
                }
            };
            
            let check_interval = std::time::Duration::from_millis(loop_config.check_interval);
            
            // Track last seen message and notification start time per chat
            let mut last_messages: HashMap<String, LastMessageCache> = HashMap::new();
            
            loop {
                // Check each chat in this automation
                for chat_id in &automation.chat_ids {
                    // Fetch latest message to check if it's new
                    let message_result = app_state.with_client(|client| {
                        tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async {
                                client.list_messages(chat_id, None, None).await
                            })
                        })
                    });
                    
                    // Also fetch chat status for unread count
                    let chat_result = app_state.with_client(|client| {
                        tokio::task::block_in_place(|| {
                            tokio::runtime::Handle::current().block_on(async {
                                client.list_chats(None, None).await
                            })
                        })
                    });
                    
                    match (message_result, chat_result) {
                        (Ok(Ok(messages_response)), Ok(Ok(chats_response))) => {
                            if let Some(latest_message) = messages_response.items.first() {
                                let current_sort_key = &latest_message.sort_key;
                                
                                // Check if this is a new message
                                let is_new_message = match last_messages.get(chat_id) {
                                    Some(cached) => &cached.sort_key < current_sort_key,
                                    None => {
                                        // First time seeing this chat, initialize
                                        last_messages.insert(chat_id.clone(), LastMessageCache {
                                            message_id: latest_message.id.clone(),
                                            sort_key: current_sort_key.clone(),
                                            notification_start_time: None,
                                        });
                                        println!("Loop automation '{}': Initialized tracking for chat {}", automation.name, chat_id);
                                        false // Don't treat first message as new
                                    }
                                };
                                
                                if is_new_message {
                                    // For ForATime, start the notification timer on new message
                                    let start_time = if loop_config.until == LoopUntil::ForATime {
                                        println!("Loop automation '{}': New message detected, started notification timer for chat {}", automation.name, chat_id);
                                        Some(std::time::Instant::now())
                                    } else {
                                        None
                                    };
                                    
                                    // Update cache with new message
                                    last_messages.insert(chat_id.clone(), LastMessageCache {
                                        message_id: latest_message.id.clone(),
                                        sort_key: current_sort_key.clone(),
                                        notification_start_time: start_time,
                                    });
                                }
                                
                                // Find chat to check unread status
                                if let Some(chat) = chats_response.items.iter().find(|c| &c.id == chat_id) {
                                    let should_notify = match loop_config.until {
                                        LoopUntil::MessageSeen => {
                                            // Keep notifying while there are unread messages
                                            chat.unread_count > 0
                                        }
                                        LoopUntil::Answer => {
                                            // Check if last message is from me (I answered)
                                            // If last message is from me, stop notifying
                                            // If last message is from them, keep notifying
                                            if let Some(is_sender) = latest_message.is_sender {
                                                !is_sender // Keep notifying if last message is NOT from me
                                            } else {
                                                // If is_sender is not available, fall back to unread count
                                                chat.unread_count > 0
                                            }
                                        }
                                        LoopUntil::ForATime => {
                                            // Check if timer has started and not expired for this specific chat
                                            if let Some(cached) = last_messages.get(chat_id) {
                                                if let Some(start_time) = cached.notification_start_time {
                                                    if let Some(time_limit) = loop_config.time {
                                                        if start_time.elapsed().as_millis() >= time_limit as u128 {
                                                            println!("Loop automation '{}': Time limit reached for chat {}, stopping notifications", automation.name, chat_id);
                                                            // Reset timer by updating cache
                                                            last_messages.insert(chat_id.clone(), LastMessageCache {
                                                                message_id: cached.message_id.clone(),
                                                                sort_key: cached.sort_key.clone(),
                                                                notification_start_time: None,
                                                            });
                                                            false
                                                        } else {
                                                            true // Keep notifying until time runs out
                                                        }
                                                    } else {
                                                        false // No time limit set, shouldn't happen
                                                    }
                                                } else {
                                                    false // No new message yet, don't notify
                                                }
                                            } else {
                                                false // Chat not in cache yet
                                            }
                                        }
                                    };
                                    
                                    if should_notify {
                                        println!("Loop automation '{}': Chat {} needs notification (unread: {})", 
                                            automation.name, chat_id, chat.unread_count);
                                        
                                        // Trigger focus action
                                        if automation.focus_chat {
                                            let result = app_state.with_client(|client| {
                                                tokio::task::block_in_place(|| {
                                                    tokio::runtime::Handle::current().block_on(async {
                                                        use beeper_desktop_api::FocusAppInput;
                                                        
                                                        let focus_input = FocusAppInput {
                                                            chat_id: Some(chat_id.clone()),
                                                            message_id: None,
                                                            draft: None,
                                                        };
                                                        
                                                        client.focus_app(Some(focus_input)).await
                                                    })
                                                })
                                            });
                                            
                                            match result {
                                                Ok(Ok(response)) => {
                                                    if response.success {
                                                        println!("✓ Focused chat {} for automation '{}'", chat_id, automation.name);
                                                    }
                                                }
                                                Ok(Err(e)) => {
                                                    eprintln!("Error focusing chat {}: {}", chat_id, e);
                                                }
                                                Err(e) => {
                                                    eprintln!("Error accessing client for focus: {}", e);
                                                }
                                            }
                                        }
                                        
                                        // Trigger notification sound if configured
                                        if let Some(sound_path) = &automation.notification_sound {
                                            if !sound_path.is_empty() {
                                                println!("▶ Playing notification sound: {}", sound_path);
                                                play_sound(sound_path);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        (Ok(Err(e)), _) | (_, Ok(Err(e))) => {
                            eprintln!("Error fetching data for automation {}: {}", automation.name, e);
                        }
                        (Err(e), _) | (_, Err(e)) => {
                            eprintln!("Error accessing client for automation {}: {}", automation.name, e);
                        }
                    }
                }
                
                // Wait for the configured check interval
                tokio::time::sleep(check_interval).await;
            }
        })
    }
}
