use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};

pub mod persistence;
pub mod message_data;
pub mod conversation_data;

pub use persistence::DataManager;
pub use message_data::ButtonMessageData;
pub use conversation_data::{ConversationContext, AIMessage, MessageRole};

/// Reminder data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reminder {
    pub id: String,
    pub user_id: String,
    pub user_name: String,
    pub message: String,
    pub channel_id: String,
    pub reminder_time: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub is_sent: bool,
    pub is_private: bool,
    pub mention_type: String, // "none", "creator", or "everyone"
    pub has_status: bool, // whether to show status dropdown
}

/// Feedback message data structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackMessage {
    pub message_id: String,
    pub original_author_id: String,
    pub original_author_name: String,
    pub original_author_avatar: String,
    pub content: String,
    pub channel_id: String,
    pub upvotes: i32,
    pub downvotes: i32,
    pub created_at: DateTime<Utc>,
}

/// Main data structure that contains all bot persistent data
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BotData {
    /// Messages with buttons (tickets, commissions, etc.)
    pub button_messages: HashMap<String, ButtonMessageData>,
    /// AI conversation contexts per user
    pub conversations: HashMap<String, ConversationContext>,
    /// Active reminders
    pub reminders: HashMap<String, Reminder>,
    /// Feedback messages
    pub feedback_messages: HashMap<String, FeedbackMessage>,
    /// Last update timestamp
    pub last_updated: DateTime<Utc>,
}

impl BotData {
    pub fn new() -> Self {
        Self {
            button_messages: HashMap::new(),
            conversations: HashMap::new(),
            reminders: HashMap::new(),
            feedback_messages: HashMap::new(),
            last_updated: Utc::now(),
        }
    }

    /// Add or update a button message
    pub fn add_button_message(&mut self, message_id: String, data: ButtonMessageData) {
        self.button_messages.insert(message_id, data);
        self.last_updated = Utc::now();
    }

    /// Get button message data
    pub fn get_button_message(&self, message_id: &str) -> Option<&ButtonMessageData> {
        self.button_messages.get(message_id)
    }

    /// Remove button message data
    pub fn remove_button_message(&mut self, message_id: &str) -> Option<ButtonMessageData> {
        self.last_updated = Utc::now();
        self.button_messages.remove(message_id)
    }

    /// Add or update conversation context for a user
    pub fn add_conversation_context(&mut self, user_id: String, context: ConversationContext) {
        self.conversations.insert(user_id, context);
        self.last_updated = Utc::now();
    }

    /// Get conversation context for a user
    pub fn get_conversation_context(&self, user_id: &str) -> Option<&ConversationContext> {
        self.conversations.get(user_id)
    }

    /// Get mutable conversation context for a user
    pub fn get_conversation_context_mut(&mut self, user_id: &str) -> Option<&mut ConversationContext> {
        self.conversations.get_mut(user_id)
    }

    /// Add or update feedback message
    pub fn add_feedback_message(&mut self, message_id: String, data: FeedbackMessage) {
        self.feedback_messages.insert(message_id, data);
        self.last_updated = Utc::now();
    }

    /// Get feedback message data
    pub fn get_feedback_message(&self, message_id: &str) -> Option<&FeedbackMessage> {
        self.feedback_messages.get(message_id)
    }

    /// Get mutable feedback message data
    pub fn get_feedback_message_mut(&mut self, message_id: &str) -> Option<&mut FeedbackMessage> {
        self.feedback_messages.get_mut(message_id)
    }

    /// Remove feedback message data
    pub fn remove_feedback_message(&mut self, message_id: &str) -> Option<FeedbackMessage> {
        self.last_updated = Utc::now();
        self.feedback_messages.remove(message_id)
    }

    /// Clean old feedback messages, keeping only the newest N messages
    pub fn clean_old_feedback_messages(&mut self, max_messages: usize) {
        if self.feedback_messages.len() <= max_messages {
            return;
        }

        // Collect all feedback messages with their creation time
        let mut messages: Vec<(String, DateTime<Utc>)> = self.feedback_messages
            .iter()
            .map(|(id, msg)| (id.clone(), msg.created_at))
            .collect();

        // Sort by creation time (newest first)
        messages.sort_by(|a, b| b.1.cmp(&a.1));

        // Keep only the newest max_messages
        let to_keep: std::collections::HashSet<String> = messages
            .into_iter()
            .take(max_messages)
            .map(|(id, _)| id)
            .collect();

        // Remove old messages
        self.feedback_messages.retain(|id, _| to_keep.contains(id));
        self.last_updated = Utc::now();
    }

    /// Add message to user's conversation context
    pub fn add_message_to_conversation(&mut self, user_id: &str, message: AIMessage) {
        if let Some(context) = self.conversations.get_mut(user_id) {
            context.add_message(message);
        } else {
            let mut new_context = ConversationContext::new();
            new_context.add_message(message);
            self.conversations.insert(user_id.to_string(), new_context);
        }
        self.last_updated = Utc::now();
    }

    /// Add message to user's conversation context with user name
    pub fn add_message_to_conversation_with_name(&mut self, user_id: &str, user_name: &str, message: AIMessage) {
        if let Some(context) = self.conversations.get_mut(user_id) {
            // Update user name if it's empty or different
            if context.user_name.is_empty() || context.user_name != user_name {
                context.user_name = user_name.to_string();
            }
            context.add_message(message);
        } else {
            let mut new_context = ConversationContext::new()
                .with_user_name(user_name.to_string());
            new_context.add_message(message);
            self.conversations.insert(user_id.to_string(), new_context);
        }
        self.last_updated = Utc::now();
    }

    /// Clean old conversation data (older than specified days)
    pub fn clean_old_conversations(&mut self, days: i64) {
        let cutoff = Utc::now() - chrono::Duration::days(days);
        self.conversations.retain(|_, context| context.last_updated > cutoff);
        self.last_updated = Utc::now();
    }

    /// Add a reminder
    pub fn add_reminder(&mut self, reminder: Reminder) {
        self.reminders.insert(reminder.id.clone(), reminder);
        self.last_updated = Utc::now();
    }

    /// Get a reminder by ID
    pub fn get_reminder(&self, reminder_id: &str) -> Option<&Reminder> {
        self.reminders.get(reminder_id)
    }

    /// Get mutable reminder by ID
    pub fn get_reminder_mut(&mut self, reminder_id: &str) -> Option<&mut Reminder> {
        self.reminders.get_mut(reminder_id)
    }

    /// Remove a reminder
    pub fn remove_reminder(&mut self, reminder_id: &str) -> Option<Reminder> {
        self.last_updated = Utc::now();
        self.reminders.remove(reminder_id)
    }

    /// Get all pending reminders (not sent and time has passed)
    pub fn get_pending_reminders(&self) -> Vec<&Reminder> {
        let now = Utc::now();
        self.reminders.values()
            .filter(|r| !r.is_sent && r.reminder_time <= now)
            .collect()
    }

    /// Mark reminder as sent
    pub fn mark_reminder_sent(&mut self, reminder_id: &str) {
        if let Some(reminder) = self.reminders.get_mut(reminder_id) {
            reminder.is_sent = true;
            self.last_updated = Utc::now();
        }
    }

    /// Clean old sent reminders (older than specified days)
    pub fn clean_old_reminders(&mut self, days: i64) {
        let cutoff = Utc::now() - chrono::Duration::days(days);
        self.reminders.retain(|_, reminder| 
            !reminder.is_sent || reminder.created_at > cutoff
        );
        self.last_updated = Utc::now();
    }
}