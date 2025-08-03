use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct Messages {
    pub commands: Commands,
    pub responses: Responses,
    pub embeds: Embeds,
    #[allow(dead_code)]
    pub errors: Errors,
    pub system: System,
}

#[derive(Debug, Deserialize)]
pub struct Commands {
    pub ping: CommandInfo,
    pub info: CommandInfo,
    pub hello: CommandInfo,
    pub help: CommandInfo,
    pub images: CommandInfo,
    pub userinfo: CommandInfo,
}

#[derive(Debug, Deserialize)]
pub struct CommandInfo {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct Responses {
    #[allow(dead_code)]
    pub ping: String,
    pub info: String,
    pub hello: String,
    pub help: String,
    pub unknown_command: String,
}

#[derive(Debug, Deserialize)]
pub struct Embeds {
    pub ping: PingEmbed,
    pub images: ImagesEmbed,
    pub userinfo: UserInfoEmbed,
}

#[derive(Debug, Deserialize)]
pub struct PingEmbed {
    pub title: String,
    pub description: String,
    pub latency_field: String,
    pub uptime_field: String,
    pub memory_field: String,
    pub latency_value: String,
    pub uptime_value: String,
    pub memory_value: String,
    pub footer: String,
    pub thumbnail: String,
}

#[derive(Debug, Deserialize)]
pub struct ImagesEmbed {
    pub title: String,
    pub description: String,
    pub footer: String,
}

#[derive(Debug, Deserialize)]
pub struct UserInfoEmbed {
    pub title: String,
    pub description: String,
    pub user_id_field: String,
    pub account_created_field: String,
    pub server_joined_field: String,
    pub roles_field: String,
    pub status_field: String,
    pub activities_field: String,
    pub premium_field: String,
    pub no_roles: String,
    pub no_activities: String,
    pub no_premium: String,
    pub footer: String,
    pub thumbnail: String,
}

#[derive(Debug, Deserialize)]
pub struct Errors {
    #[allow(dead_code)]
    pub client_creation: String,
    #[allow(dead_code)]
    pub bot_start: String,
    #[allow(dead_code)]
    pub token_missing: String,
}

#[derive(Debug, Deserialize)]
pub struct System {
    pub bot_connected: String,
}

pub struct LanguageManager {
    messages: Messages,
}

#[derive(Debug, Deserialize)]
pub struct BotImages {
    pub avatar: HashMap<String, String>,
    pub emotions: HashMap<String, String>,
    pub reactions: HashMap<String, String>,
    pub talking: HashMap<String, String>,
    pub thinking: HashMap<String, String>,
    pub showing: HashMap<String, String>,
    pub misc: HashMap<String, String>,
    pub defaults: HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct BotEmojis {
    pub status: HashMap<String, String>,
    pub confirmations: HashMap<String, String>,
    pub emotions: HashMap<String, String>,
    pub technology: HashMap<String, String>,
    pub actions: HashMap<String, String>,
    pub interface: HashMap<String, String>,
    pub defaults: HashMap<String, String>,
}

pub struct ImageManager {
    images: BotImages,
}

pub struct EmojiManager {
    emojis: BotEmojis,
}

impl ImageManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let images_content = include_str!("../../bot_images.toml");
        let images: BotImages = toml::from_str(images_content)?;
        
        Ok(ImageManager { images })
    }
    
    pub fn get_image(&self, category: &str, name: &str) -> Option<&String> {
        match category {
            "avatar" => self.images.avatar.get(name),
            "emotions" => self.images.emotions.get(name),
            "reactions" => self.images.reactions.get(name),
            "talking" => self.images.talking.get(name),
            "thinking" => self.images.thinking.get(name),
            "showing" => self.images.showing.get(name),
            "misc" => self.images.misc.get(name),
            _ => None,
        }
    }
    
    pub fn get_default_image(&self, situation: &str) -> Option<&String> {
        if let Some(default_ref) = self.images.defaults.get(situation) {
            // Parse reference like "avatar.what_pointing"
            if let Some(dot_pos) = default_ref.find('.') {
                let category = &default_ref[..dot_pos];
                let name = &default_ref[dot_pos + 1..];
                self.get_image(category, name)
            } else {
                // Try to find in defaults directly
                self.images.defaults.get(default_ref)
            }
        } else {
            None
        }
    }
    
    pub fn get_random_avatar(&self) -> Option<&String> {
        let avatars: Vec<&String> = self.images.avatar.values().collect();
        if !avatars.is_empty() {
            Some(avatars[0]) // For now return first one, could add randomization later
        } else {
            None
        }
    }
    
    pub fn list_categories(&self) -> Vec<&str> {
        vec!["avatar", "emotions", "reactions", "talking", "thinking", "showing", "misc"]
    }
    
    pub fn list_images_in_category(&self, category: &str) -> Vec<&String> {
        match category {
            "avatar" => self.images.avatar.keys().collect(),
            "emotions" => self.images.emotions.keys().collect(),
            "reactions" => self.images.reactions.keys().collect(),
            "talking" => self.images.talking.keys().collect(),
            "thinking" => self.images.thinking.keys().collect(),
            "showing" => self.images.showing.keys().collect(),
            "misc" => self.images.misc.keys().collect(),
            _ => vec![],
        }
    }
}

impl LanguageManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let lang_content = include_str!("../../lang/en.toml");
        let messages: Messages = toml::from_str(lang_content)?;
        
        Ok(LanguageManager { messages })
    }
    
    pub fn get(&self) -> &Messages {
        &self.messages
    }
    
    pub fn format_hello(&self, username: &str) -> String {
        self.messages.responses.hello.replace("{username}", username)
    }
    
    pub fn format_bot_connected(&self, bot_name: &str) -> String {
        self.messages.system.bot_connected.replace("{bot_name}", bot_name)
    }
    
    pub fn format_ping_embed_latency(&self, latency: u64) -> String {
        self.messages.embeds.ping.latency_value.replace("{latency}", &latency.to_string())
    }
    
    pub fn format_ping_embed_uptime(&self, uptime: &str) -> String {
        self.messages.embeds.ping.uptime_value.replace("{uptime}", uptime)
    }
    
    pub fn format_ping_embed_memory(&self, memory: f64) -> String {
        self.messages.embeds.ping.memory_value.replace("{memory}", &format!("{:.1}", memory))
    }
    
    pub fn format_images_embed_footer(&self, total_images: usize) -> String {
        self.messages.embeds.images.footer.replace("{total_images}", &total_images.to_string())
    }
    
    pub fn format_userinfo_embed_description(&self, username: &str) -> String {
        self.messages.embeds.userinfo.description.replace("{username}", username)
    }
    
    pub fn format_userinfo_embed_footer(&self, requester: &str) -> String {
        self.messages.embeds.userinfo.footer.replace("{requester}", requester)
    }
}

impl EmojiManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let emojis_content = include_str!("../../bot_emojis.toml");
        let emojis: BotEmojis = toml::from_str(emojis_content)?;
        
        Ok(EmojiManager { emojis })
    }
    
    pub fn get_emoji(&self, category: &str, name: &str) -> Option<&String> {
        match category {
            "status" => self.emojis.status.get(name),
            "confirmations" => self.emojis.confirmations.get(name),
            "emotions" => self.emojis.emotions.get(name),
            "technology" => self.emojis.technology.get(name),
            "actions" => self.emojis.actions.get(name),
            "interface" => self.emojis.interface.get(name),
            _ => None,
        }
    }
    
    pub fn get_default_emoji(&self, situation: &str) -> Option<&String> {
        if let Some(default_ref) = self.emojis.defaults.get(situation) {
            // Parse reference like "confirmations.check"
            if let Some(dot_pos) = default_ref.find('.') {
                let category = &default_ref[..dot_pos];
                let name = &default_ref[dot_pos + 1..];
                self.get_emoji(category, name)
            } else {
                // Try to find in defaults directly or by name in any category
                self.emojis.defaults.get(default_ref)
                    .or_else(|| self.get_emoji("status", default_ref))
                    .or_else(|| self.get_emoji("confirmations", default_ref))
                    .or_else(|| self.get_emoji("emotions", default_ref))
                    .or_else(|| self.get_emoji("technology", default_ref))
                    .or_else(|| self.get_emoji("actions", default_ref))
                    .or_else(|| self.get_emoji("interface", default_ref))
            }
        } else {
            None
        }
    }
    
    pub fn list_categories(&self) -> Vec<&str> {
        vec!["status", "confirmations", "emotions", "technology", "actions", "interface"]
    }
    
    pub fn list_emojis_in_category(&self, category: &str) -> Vec<&String> {
        match category {
            "status" => self.emojis.status.keys().collect(),
            "confirmations" => self.emojis.confirmations.keys().collect(),
            "emotions" => self.emojis.emotions.keys().collect(),
            "technology" => self.emojis.technology.keys().collect(),
            "actions" => self.emojis.actions.keys().collect(),
            "interface" => self.emojis.interface.keys().collect(),
            _ => vec![],
        }
    }
    
    // Helper methods for common emojis
    pub fn success(&self) -> &str {
        self.get_default_emoji("success").map(|s| s.as_str()).unwrap_or("✅")
    }
    
    pub fn error(&self) -> &str {
        self.get_default_emoji("error").map(|s| s.as_str()).unwrap_or("❌")
    }
    
    pub fn warning(&self) -> &str {
        self.get_default_emoji("warning").map(|s| s.as_str()).unwrap_or("⚠️")
    }
    
    pub fn loading(&self) -> &str {
        self.get_default_emoji("loading").map(|s| s.as_str()).unwrap_or("💤")
    }
}