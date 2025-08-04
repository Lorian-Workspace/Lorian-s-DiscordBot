use serenity::builder::{CreateEmbed, CreateActionRow, CreateButton};
use serenity::model::prelude::*;
use crate::ai::emotions::EmotionType;
use crate::lang::{ImageManager, EmojiManager};
use chrono::Utc;

/// Types of buttons that can be added to AI responses
#[derive(Debug, Clone)]
pub enum ButtonType {
    Commission {
        label: String,
        service_type: String,
    },
    Ticket {
        label: String,
    },
    Feedback {
        label: String,
        action: String,
    },
    Custom {
        label: String,
        custom_id: String,
        style: ButtonStyle,
    },
}

impl ButtonType {
    /// Generate a unique custom_id for this button
    pub fn custom_id(&self) -> String {
        match self {
            ButtonType::Commission { service_type, .. } => {
                format!("ai_commission_{}", service_type)
            },
            ButtonType::Ticket { .. } => {
                "ai_create_ticket".to_string()
            },
            ButtonType::Feedback { action, .. } => {
                format!("ai_feedback_{}", action)
            },
            ButtonType::Custom { custom_id, .. } => {
                custom_id.clone()
            },
        }
    }

    /// Get the button style
    pub fn style(&self) -> ButtonStyle {
        match self {
            ButtonType::Commission { .. } => ButtonStyle::Primary,
            ButtonType::Ticket { .. } => ButtonStyle::Secondary,
            ButtonType::Feedback { .. } => ButtonStyle::Success,
            ButtonType::Custom { style, .. } => *style,
        }
    }

    /// Get the button label
    pub fn label(&self) -> &str {
        match self {
            ButtonType::Commission { label, .. } => label,
            ButtonType::Ticket { label } => label,
            ButtonType::Feedback { label, .. } => label,
            ButtonType::Custom { label, .. } => label,
        }
    }
}

/// Builder for AI responses with embeds and buttons
pub struct AIResponseBuilder {
    content: String,
    emotion: EmotionType,
    buttons: Vec<ButtonType>,
    thumbnail_override: Option<String>,
    footer_text: Option<String>,
    custom_color: Option<(u8, u8, u8)>,
}

impl AIResponseBuilder {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            emotion: EmotionType::Neutral,
            buttons: Vec::new(),
            thumbnail_override: None,
            footer_text: None,
            custom_color: None,
        }
    }

    pub fn content(mut self, content: String) -> Self {
        self.content = content;
        self
    }

    pub fn emotion(mut self, emotion: EmotionType) -> Self {
        self.emotion = emotion;
        self
    }

    pub fn add_button(mut self, button: ButtonType) -> Self {
        self.buttons.push(button);
        self
    }

    pub fn thumbnail(mut self, thumbnail_url: String) -> Self {
        self.thumbnail_override = Some(thumbnail_url);
        self
    }

    pub fn footer(mut self, footer_text: String) -> Self {
        self.footer_text = Some(footer_text);
        self
    }

    pub fn custom_color(mut self, color: (u8, u8, u8)) -> Self {
        self.custom_color = Some(color);
        self
    }

    /// Build the Discord embed for this AI response
    pub fn build_embed(
        &self,
        images: &ImageManager,
        _emojis: &EmojiManager,
        author_name: &str,
        lang: &crate::lang::LanguageManager,
    ) -> CreateEmbed {
        let color = if let Some(custom_color) = self.custom_color {
            custom_color
        } else {
            self.emotion.color()
        };
        
        // Get thumbnail image based on emotion
        let thumbnail_url = if let Some(ref override_url) = self.thumbnail_override {
            override_url.clone()
        } else {
            images.get_image(self.emotion.image_category(), self.emotion.image_name())
                .or_else(|| images.get_random_avatar())
                .unwrap_or(&"https://cdn.discordapp.com/embed/avatars/0.png".to_string())
                .clone()
        };

        // Build the embed title with emotion emoji using translations
        let title = lang.format_ai_embed_title(self.emotion.emoji());

        // Build footer text using translations
        let footer_text = if let Some(ref custom_footer) = self.footer_text {
            custom_footer.clone()
        } else {
            lang.format_ai_embed_footer(author_name, self.emotion.emoji())
        };

        CreateEmbed::new()
            .title(title)
            .description(&self.content)
            .color(Color::from_rgb(color.0, color.1, color.2))
            .thumbnail(thumbnail_url)
            .footer(serenity::builder::CreateEmbedFooter::new(footer_text))
            .timestamp(Utc::now())
    }

    /// Build the action rows with buttons for this response
    pub fn build_action_rows(&self) -> Vec<CreateActionRow> {
        if self.buttons.is_empty() {
            return Vec::new();
        }

        let mut action_rows = Vec::new();
        let mut current_buttons = Vec::new();

        for button in &self.buttons {
            if current_buttons.len() >= 5 {
                // Discord limits to 5 buttons per row
                action_rows.push(CreateActionRow::Buttons(current_buttons));
                current_buttons = Vec::new();
            }

            let discord_button = CreateButton::new(button.custom_id())
                .label(button.label())
                .style(button.style());

            current_buttons.push(discord_button);
        }

        if !current_buttons.is_empty() {
            action_rows.push(CreateActionRow::Buttons(current_buttons));
        }

        action_rows
    }

    /// Get the content text
    pub fn get_content(&self) -> &str {
        &self.content
    }

    /// Get the emotion type
    pub fn get_emotion(&self) -> &EmotionType {
        &self.emotion
    }

    /// Get the buttons
    pub fn get_buttons(&self) -> &[ButtonType] {
        &self.buttons
    }

    /// Check if response has buttons
    pub fn has_buttons(&self) -> bool {
        !self.buttons.is_empty()
    }

    /// Get button metadata for data storage
    pub fn get_button_metadata(&self) -> std::collections::HashMap<String, String> {
        let mut metadata = std::collections::HashMap::new();
        
        for (index, button) in self.buttons.iter().enumerate() {
            let key = format!("button_{}", index);
            let value = match button {
                ButtonType::Commission { service_type, .. } => {
                    format!("commission:{}", service_type)
                },
                ButtonType::Ticket { .. } => {
                    "ticket:create".to_string()
                },
                ButtonType::Feedback { action, .. } => {
                    format!("feedback:{}", action)
                },
                ButtonType::Custom { custom_id, .. } => {
                    format!("custom:{}", custom_id)
                },
            };
            metadata.insert(key, value);
        }
        
        metadata.insert("emotion".to_string(), format!("{:?}", self.emotion));
        metadata.insert("button_count".to_string(), self.buttons.len().to_string());
        
        metadata
    }
}

impl Default for AIResponseBuilder {
    fn default() -> Self {
        Self::new()
    }
}