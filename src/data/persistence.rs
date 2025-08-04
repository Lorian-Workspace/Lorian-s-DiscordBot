use serde::{Deserialize, Serialize};
use serde_json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::io::Write;
use crate::data::BotData;
use crate::data::message_data::MessageType;
use chrono::Utc;

/// Manages data persistence for the bot
#[derive(Debug, Clone)]
pub struct DataManager {
    /// Path to the data directory
    data_dir: PathBuf,
    /// In-memory cache of bot data
    data: Arc<Mutex<BotData>>,
    /// Whether auto-save is enabled
    auto_save: bool,
}

/// Configuration for the data manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataConfig {
    /// Directory where data files are stored
    pub data_directory: String,
    /// Filename for the main bot data
    pub data_filename: String,
    /// Whether to enable auto-save
    pub auto_save: bool,
    /// Auto-save interval in seconds (if auto_save is true)
    pub auto_save_interval: u64,
}

impl Default for DataConfig {
    fn default() -> Self {
        Self {
            data_directory: "data".to_string(),
            data_filename: "bot_data.json".to_string(),
            auto_save: true,
            auto_save_interval: 30, // 30 seconds
        }
    }
}

impl DataManager {
    /// Create a new DataManager with default configuration
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        Self::with_config(DataConfig::default())
    }

    /// Create a new DataManager with custom configuration
    pub fn with_config(config: DataConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let data_dir = PathBuf::from(&config.data_directory);
        
        // Create data directory if it doesn't exist
        if !data_dir.exists() {
            fs::create_dir_all(&data_dir)?;
        }

        let data_file = data_dir.join(&config.data_filename);
        let data = if data_file.exists() {
            Self::load_from_file(&data_file)?
        } else {
            BotData::new()
        };

        Ok(Self {
            data_dir,
            data: Arc::new(Mutex::new(data)),
            auto_save: config.auto_save,
        })
    }

    /// Load data from file
    fn load_from_file(file_path: &Path) -> Result<BotData, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(file_path)?;
        let data: BotData = serde_json::from_str(&content)?;
        Ok(data)
    }

    /// Save data to file
    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let data = self.data.lock().unwrap();
        let file_path = self.data_dir.join("bot_data.json");
        
        // Create a backup of the current file
        if file_path.exists() {
            let backup_path = self.data_dir.join("bot_data.json.backup");
            fs::copy(&file_path, backup_path)?;
        }
        
        // Write new data
        let json = serde_json::to_string_pretty(&*data)?;
        let mut file = fs::File::create(&file_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        
        println!("Data saved to {}", file_path.display());
        Ok(())
    }

    /// Get a clone of the current bot data
    pub fn get_data(&self) -> BotData {
        self.data.lock().unwrap().clone()
    }

    /// Update bot data and optionally save
    pub fn update_data<F>(&self, update_fn: F) -> Result<(), Box<dyn std::error::Error>>
    where
        F: FnOnce(&mut BotData),
    {
        {
            let mut data = self.data.lock().unwrap();
            update_fn(&mut *data);
            data.last_updated = Utc::now();
        }
        
        if self.auto_save {
            self.save()?;
        }
        
        Ok(())
    }

    /// Get button message data
    pub fn get_button_message(&self, message_id: &str) -> Option<crate::data::ButtonMessageData> {
        let data = self.data.lock().unwrap();
        data.get_button_message(message_id).cloned()
    }

    /// Add button message data
    pub fn add_button_message(&self, message_id: String, message_data: crate::data::ButtonMessageData) -> Result<(), Box<dyn std::error::Error>> {
        self.update_data(|data| {
            data.add_button_message(message_id, message_data);
        })
    }

    /// Remove button message data
    pub fn remove_button_message(&self, message_id: &str) -> Result<Option<crate::data::ButtonMessageData>, Box<dyn std::error::Error>> {
        let removed = {
            let mut data = self.data.lock().unwrap();
            data.remove_button_message(message_id)
        };
        
        if self.auto_save {
            self.save()?;
        }
        
        Ok(removed)
    }

    /// Get conversation context for a user
    pub fn get_conversation_context(&self, user_id: &str) -> Option<crate::data::ConversationContext> {
        let data = self.data.lock().unwrap();
        data.get_conversation_context(user_id).cloned()
    }

    /// Add message to user's conversation
    pub fn add_conversation_message(&self, user_id: &str, message: crate::data::AIMessage) -> Result<(), Box<dyn std::error::Error>> {
        self.update_data(|data| {
            data.add_message_to_conversation(user_id, message);
        })
    }

    /// Add message to user's conversation with user name
    pub fn add_conversation_message_with_name(&self, user_id: &str, user_name: &str, message: crate::data::AIMessage) -> Result<(), Box<dyn std::error::Error>> {
        self.update_data(|data| {
            data.add_message_to_conversation_with_name(user_id, user_name, message);
        })
    }

    /// Clear conversation for a user
    pub fn clear_conversation(&self, user_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.update_data(|data| {
            if let Some(context) = data.get_conversation_context_mut(user_id) {
                context.clear_messages();
            }
        })
    }

    /// Clean old conversations
    pub fn clean_old_conversations(&self, days: i64) -> Result<(), Box<dyn std::error::Error>> {
        self.update_data(|data| {
            data.clean_old_conversations(days);
        })
    }

    /// Add a reminder
    pub fn add_reminder(&self, reminder: crate::data::Reminder) -> Result<(), Box<dyn std::error::Error>> {
        self.update_data(|data| {
            data.add_reminder(reminder);
        })
    }

    /// Get a reminder by ID
    pub fn get_reminder(&self, reminder_id: &str) -> Option<crate::data::Reminder> {
        let data = self.data.lock().unwrap();
        data.get_reminder(reminder_id).cloned()
    }

    /// Remove a reminder
    pub fn remove_reminder(&self, reminder_id: &str) -> Result<Option<crate::data::Reminder>, Box<dyn std::error::Error>> {
        let removed = {
            let mut data = self.data.lock().unwrap();
            data.remove_reminder(reminder_id)
        };
        
        if self.auto_save {
            self.save()?;
        }
        
        Ok(removed)
    }

    /// Get all pending reminders
    pub fn get_pending_reminders(&self) -> Vec<crate::data::Reminder> {
        let data = self.data.lock().unwrap();
        data.get_pending_reminders().into_iter().cloned().collect()
    }

    /// Mark reminder as sent
    pub fn mark_reminder_sent(&self, reminder_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.update_data(|data| {
            data.mark_reminder_sent(reminder_id);
        })
    }

    /// Clean old reminders
    pub fn clean_old_reminders(&self, days: i64) -> Result<(), Box<dyn std::error::Error>> {
        self.update_data(|data| {
            data.clean_old_reminders(days);
        })
    }

    /// Add feedback message
    pub fn add_feedback_message(&self, feedback_message: crate::data::FeedbackMessage) -> Result<(), Box<dyn std::error::Error>> {
        let message_id = feedback_message.message_id.clone();
        self.update_data(|data| {
            data.add_feedback_message(message_id, feedback_message);
        })
    }

    /// Get feedback message
    pub fn get_feedback_message(&self, message_id: &str) -> Option<crate::data::FeedbackMessage> {
        let data = self.data.lock().unwrap();
        data.get_feedback_message(message_id).cloned()
    }

    /// Update feedback message
    pub fn update_feedback_message(&self, feedback_message: crate::data::FeedbackMessage) -> Result<(), Box<dyn std::error::Error>> {
        let message_id = feedback_message.message_id.clone();
        self.update_data(|data| {
            data.add_feedback_message(message_id, feedback_message);
        })
    }

    /// Remove feedback message
    pub fn remove_feedback_message(&self, message_id: &str) -> Result<Option<crate::data::FeedbackMessage>, Box<dyn std::error::Error>> {
        let removed = {
            let mut data = self.data.lock().unwrap();
            data.remove_feedback_message(message_id)
        };
        
        if self.auto_save {
            self.save()?;
        }
        
        Ok(removed)
    }

    /// Clean old feedback messages, keeping only the newest N messages
    pub fn clean_old_feedback_messages(&self, max_messages: usize) {
        {
            let mut data = self.data.lock().unwrap();
            data.clean_old_feedback_messages(max_messages);
        }
        
        if self.auto_save {
            let _ = self.save();
        }
    }

    /// Get the path to the data directory
    pub fn get_data_dir(&self) -> &Path {
        &self.data_dir
    }

    /// Export data to a specific file
    pub fn export_to_file(&self, file_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let data = self.data.lock().unwrap();
        let json = serde_json::to_string_pretty(&*data)?;
        let mut file = fs::File::create(file_path)?;
        file.write_all(json.as_bytes())?;
        file.sync_all()?;
        Ok(())
    }

    /// Import data from a specific file
    pub fn import_from_file(&self, file_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        let imported_data = Self::load_from_file(file_path)?;
        {
            let mut data = self.data.lock().unwrap();
            *data = imported_data;
        }
        
        if self.auto_save {
            self.save()?;
        }
        
        Ok(())
    }

    /// Get statistics about stored data
    pub fn get_stats(&self) -> DataStats {
        let data = self.data.lock().unwrap();
        DataStats {
            button_messages_count: data.button_messages.len(),
            conversations_count: data.conversations.len(),
            total_messages: data.conversations.values().map(|c| c.message_count()).sum(),
            last_updated: data.last_updated,
        }
    }

    /// Migrate old data structure if needed (user_id -> user_name and add summary fields)
    pub fn migrate_data_if_needed(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut needs_migration = false;
        
        // Check if we need migration
        {
            let data = self.data.lock().unwrap();
            for context in data.conversations.values() {
                if context.user_name.is_empty() {
                    needs_migration = true;
                    break;
                }
            }
        }
        
        if needs_migration {
            println!("Migrating conversation data structure...");
            self.update_data(|data| {
                for (user_id, context) in data.conversations.iter_mut() {
                    if context.user_name.is_empty() {
                        // Set a placeholder name based on user ID
                        context.user_name = format!("User_{}", &user_id[..8]);
                    }
                    // Initialize summary fields if they don't exist (handled by Serde defaults)
                    // user_summary and message_count_for_summary will be set to defaults during deserialization
                }
            })?;
            println!("Data migration completed successfully.");
        }
        
        Ok(())
    }

    /// Increment message counter for user and return if summary analysis should be triggered
    pub fn increment_message_counter_and_check(&self, user_id: &str) -> Result<bool, Box<dyn std::error::Error>> {
        let mut should_analyze = false;
        
        self.update_data(|data| {
            if let Some(context) = data.get_conversation_context_mut(user_id) {
                should_analyze = context.increment_message_counter();
            }
        })?;
        
        Ok(should_analyze)
    }

    /// Get active ticket channels for a user
    pub fn get_user_active_tickets(&self, user_id: &str) -> Vec<String> {
        let data = self.data.lock().unwrap();
        let mut active_tickets = Vec::new();
        
        for button_data in data.button_messages.values() {
            if button_data.message_type == MessageType::Ticket {
                if let Some(creator_id) = button_data.get_metadata("creator_id") {
                    if creator_id == user_id {
                        active_tickets.push(button_data.channel_id.clone());
                    }
                }
            }
        }
        
        active_tickets
    }

    /// Check if a channel is a ticket channel
    pub fn is_ticket_channel(&self, channel_id: &str) -> bool {
        let data = self.data.lock().unwrap();
        
        for button_data in data.button_messages.values() {
            if button_data.message_type == MessageType::Ticket && 
               button_data.channel_id == channel_id {
                return true;
            }
        }
        
        false
    }

    /// Check if a user is the creator of a ticket channel
    pub fn is_ticket_creator(&self, channel_id: &str, user_id: &str) -> bool {
        let data = self.data.lock().unwrap();
        
        for button_data in data.button_messages.values() {
            if button_data.message_type == MessageType::Ticket && 
               button_data.channel_id == channel_id {
                if let Some(creator_id) = button_data.get_metadata("creator_id") {
                    return creator_id == user_id;
                }
            }
        }
        
        false
    }

    /// Clean up ticket data when a ticket is closed
    pub fn cleanup_ticket_data(&self, channel_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.update_data(|data| {
            // Remove button messages associated with this ticket channel
            let message_ids_to_remove: Vec<String> = data.button_messages
                .iter()
                .filter(|(_, button_data)| {
                    button_data.message_type == MessageType::Ticket && 
                    button_data.channel_id == channel_id
                })
                .map(|(message_id, _)| message_id.clone())
                .collect();
            
            for message_id in message_ids_to_remove {
                data.button_messages.remove(&message_id);
            }
        })
    }
}

/// Statistics about the stored data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataStats {
    pub button_messages_count: usize,
    pub conversations_count: usize,
    pub total_messages: usize,
    pub last_updated: chrono::DateTime<chrono::Utc>,
}