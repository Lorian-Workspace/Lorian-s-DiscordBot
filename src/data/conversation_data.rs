use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Maximum number of messages to keep in context
pub const MAX_CONTEXT_MESSAGES: usize = 15;

/// Role of the message sender
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageRole {
    /// Message from the user
    User,
    /// Message from the AI bot
    Assistant,
    /// System message (instructions, context)
    System,
}

/// A single AI conversation message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIMessage {
    /// Role of the sender
    pub role: MessageRole,
    /// Content of the message
    pub content: String,
    /// Timestamp when message was sent
    pub timestamp: DateTime<Utc>,
    /// Optional channel ID where message was sent
    pub channel_id: Option<String>,
    /// Optional message ID in Discord
    pub discord_message_id: Option<String>,
}

impl AIMessage {
    pub fn new(role: MessageRole, content: String) -> Self {
        Self {
            role,
            content,
            timestamp: Utc::now(),
            channel_id: None,
            discord_message_id: None,
        }
    }

    pub fn with_channel(mut self, channel_id: String) -> Self {
        self.channel_id = Some(channel_id);
        self
    }

    pub fn with_discord_message_id(mut self, message_id: String) -> Self {
        self.discord_message_id = Some(message_id);
        self
    }
}

/// Conversation context for a user with AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationContext {
    /// User name this context belongs to
    pub user_name: String,
    /// List of messages in the conversation (limited to MAX_CONTEXT_MESSAGES)
    pub messages: Vec<AIMessage>,
    /// When this context was last updated
    pub last_updated: DateTime<Utc>,
    /// Additional user preferences or settings
    pub settings: std::collections::HashMap<String, String>,
    /// AI-generated summary of important user information
    #[serde(default)]
    pub user_summary: String,
    /// Counter for messages to trigger summary analysis (resets every 20 messages)
    #[serde(default)]
    pub message_count_for_summary: u32,
}

impl ConversationContext {
    pub fn new() -> Self {
        Self {
            user_name: String::new(),
            messages: Vec::new(),
            last_updated: Utc::now(),
            settings: std::collections::HashMap::new(),
            user_summary: String::new(),
            message_count_for_summary: 0,
        }
    }

    pub fn with_user_name(mut self, user_name: String) -> Self {
        self.user_name = user_name;
        self
    }

    /// Add a message to the conversation context
    pub fn add_message(&mut self, message: AIMessage) {
        self.messages.push(message);
        self.last_updated = Utc::now();

        // Keep only the last MAX_CONTEXT_MESSAGES
        if self.messages.len() > MAX_CONTEXT_MESSAGES {
            let excess = self.messages.len() - MAX_CONTEXT_MESSAGES;
            self.messages.drain(0..excess);
        }
    }

    /// Get the most recent messages
    pub fn get_recent_messages(&self, count: usize) -> &[AIMessage] {
        let start = if self.messages.len() > count {
            self.messages.len() - count
        } else {
            0
        };
        &self.messages[start..]
    }

    /// Get all messages
    pub fn get_all_messages(&self) -> &[AIMessage] {
        &self.messages
    }

    /// Clear all messages
    pub fn clear_messages(&mut self) {
        self.messages.clear();
        self.last_updated = Utc::now();
    }

    /// Get conversation summary for API context
    pub fn get_conversation_summary(&self) -> String {
        if self.messages.is_empty() {
            return "No previous conversation.".to_string();
        }

        let mut summary = String::new();
        for message in &self.messages {
            let role_str = match message.role {
                MessageRole::User => "User",
                MessageRole::Assistant => "Assistant",
                MessageRole::System => "System",
            };
            summary.push_str(&format!("{}: {}\n", role_str, message.content));
        }
        summary
    }

    /// Check if context is empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Get message count
    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    /// Add or update a setting
    pub fn set_setting(&mut self, key: String, value: String) {
        self.settings.insert(key, value);
        self.last_updated = Utc::now();
    }

    /// Get a setting value
    pub fn get_setting(&self, key: &str) -> Option<&String> {
        self.settings.get(key)
    }

    /// Increment message counter and return true if summary analysis should be triggered
    pub fn increment_message_counter(&mut self) -> bool {
        self.message_count_for_summary += 1;
        self.last_updated = Utc::now();
        
        if self.message_count_for_summary >= 20 {
            self.message_count_for_summary = 0; // Reset counter
            true // Trigger summary analysis
        } else {
            false
        }
    }

    /// Update user summary
    pub fn update_summary(&mut self, new_summary: String) {
        self.user_summary = new_summary;
        self.last_updated = Utc::now();
    }

    /// Get user summary
    pub fn get_summary(&self) -> &str {
        &self.user_summary
    }

    /// Check if user has a summary
    pub fn has_summary(&self) -> bool {
        !self.user_summary.is_empty()
    }
}

impl Default for ConversationContext {
    fn default() -> Self {
        Self::new()
    }
}