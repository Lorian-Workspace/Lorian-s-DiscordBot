use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

/// Types of messages that can have buttons
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageType {
    /// Support ticket system
    Ticket,
    /// Commission system
    Commission,
    /// Feedback system
    Feedback,
    /// General message with buttons
    General,
}

/// Actions that buttons can trigger
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ButtonAction {
    /// Create a new ticket
    CreateTicket {
        channel_id: String,
        user_id: String,
    },
    /// Close an existing ticket
    CloseTicket {
        ticket_channel_id: String,
        creator_id: String,
    },
    /// Create a commission ticket
    CreateCommission {
        channel_id: String,
        user_id: String,
    },
    /// Close a commission ticket
    CloseCommission {
        commission_channel_id: String,
        creator_id: String,
    },
    /// React to feedback (upvote/downvote)
    FeedbackReaction {
        reaction_type: FeedbackReactionType,
        original_user_id: String,
    },
    /// Custom action with parameters
    Custom {
        action_name: String,
        parameters: std::collections::HashMap<String, String>,
    },
}

/// Types of feedback reactions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FeedbackReactionType {
    Upvote,
    Downvote,
}

/// Data stored for messages with buttons
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonMessageData {
    /// ID of the message containing buttons
    pub message_id: String,
    /// Channel where the message is located
    pub channel_id: String,
    /// Type of message
    pub message_type: MessageType,
    /// Actions mapped to button custom_ids
    pub button_actions: std::collections::HashMap<String, ButtonAction>,
    /// When this message was created
    pub created_at: DateTime<Utc>,
    /// Additional metadata
    pub metadata: std::collections::HashMap<String, String>,
}

impl ButtonMessageData {
    pub fn new(
        message_id: String,
        channel_id: String,
        message_type: MessageType,
    ) -> Self {
        Self {
            message_id,
            channel_id,
            message_type,
            button_actions: std::collections::HashMap::new(),
            created_at: Utc::now(),
            metadata: std::collections::HashMap::new(),
        }
    }

    /// Add a button action
    pub fn add_button_action(&mut self, custom_id: String, action: ButtonAction) {
        self.button_actions.insert(custom_id, action);
    }

    /// Get button action by custom_id
    pub fn get_button_action(&self, custom_id: &str) -> Option<&ButtonAction> {
        self.button_actions.get(custom_id)
    }

    /// Add metadata
    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }

    /// Get metadata
    pub fn get_metadata(&self, key: &str) -> Option<&String> {
        self.metadata.get(key)
    }

    /// Check if this is a ticket-related message
    pub fn is_ticket_message(&self) -> bool {
        matches!(self.message_type, MessageType::Ticket | MessageType::Commission)
    }

    /// Check if this is a feedback message
    pub fn is_feedback_message(&self) -> bool {
        matches!(self.message_type, MessageType::Feedback)
    }
}