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
    pub ai: AISystem,
    pub commission: CommissionSystem,
    pub ticket: TicketSystem,
    pub feedback: FeedbackSystem,
}

#[derive(Debug, Deserialize)]
pub struct Commands {
    pub ping: CommandInfo,
    pub info: CommandInfo,
    pub hello: CommandInfo,
    pub help: CommandInfo,
    pub images: CommandInfo,
    pub userinfo: CommandInfo,
    pub stats: CommandInfo,
    pub purge: CommandInfo,
    pub reminder: CommandInfo,
    pub commission_setup: CommandInfo,
    pub commission_close: CommandInfo,
    pub ticket_setup: CommandInfo,
    pub ticket_close: CommandInfo,
    pub feedback_setup: CommandInfo,
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
    pub purge: PurgeEmbed,
    pub reminder: ReminderEmbed,
    pub reminder_notification: ReminderNotificationEmbed,
    pub help: HelpEmbed,
    pub commission: CommissionEmbed,
    pub commission_created: CommissionCreatedEmbed,
    pub commission_closed: CommissionClosedEmbed,
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
    pub ai_summary_field: String,
    pub no_ai_summary: String,
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
pub struct PurgeEmbed {
    pub title: String,
    pub description: String,
    pub success_message: String,
    pub error_permission: String,
    pub error_invalid_amount: String,
    pub error_failed: String,
    pub footer: String,
}

#[derive(Debug, Deserialize)]
pub struct ReminderEmbed {
    pub title: String,
    pub description: String,
    pub success_message: String,
    pub time_field: String,
    pub message_field: String,
    pub channel_field: String,
    pub visibility_field: String,
    pub mention_field: String,
    pub status_field: String,
    pub error_invalid_time: String,
    pub error_no_channel: String,
    pub error_permission: String,
    pub error_failed: String,
    pub footer: String,
    pub visibility_public: String,
    pub visibility_private: String,
    pub mention_none: String,
    pub mention_creator: String,
    pub mention_everyone: String,
    pub status_enabled: String,
    pub status_disabled: String,
}

#[derive(Debug, Deserialize)]
pub struct ReminderNotificationEmbed {
    pub title: String,
    pub description: String,
    pub user_field: String,
    pub created_field: String,
    pub footer: String,
}

#[derive(Debug, Deserialize)]
pub struct HelpEmbed {
    pub title: String,
    pub description: String,
    pub footer: String,
    pub select_placeholder: String,
    pub select_description: String,
    pub commands: HelpCommands,
}

#[derive(Debug, Deserialize)]
pub struct HelpCommands {
    pub ping: HelpCommand,
    pub info: HelpCommand,
    pub hello: HelpCommand,
    pub stats: HelpCommand,
    pub images: HelpCommand,
    pub userinfo: HelpCommand,
    pub purge: HelpCommand,
    pub reminder: HelpCommand,
    pub commission_setup: HelpCommand,
    pub commission_close: HelpCommand,
}

#[derive(Debug, Deserialize)]
pub struct HelpCommand {
    pub title: String,
    pub description: String,
    pub usage: String,
    pub details: String,
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

#[derive(Debug, Deserialize)]
pub struct AISystem {
    pub embeds: AIEmbeds,
    pub emotions: AIEmotions,
    pub prompt: AIPrompt,
    pub messages: AIMessages,
    pub data: AIData,
}

#[derive(Debug, Deserialize)]
pub struct AIEmbeds {
    pub title_format: String,
    pub footer_format: String,
    pub no_context: String,
}

#[derive(Debug, Deserialize)]
pub struct AIEmotions {
    pub happy: String,
    pub excited: String,
    pub helpful: String,
    pub thoughtful: String,
    pub curious: String,
    pub friendly: String,
    pub professional: String,
    pub creative: String,
    pub encouraging: String,
    pub neutral: String,
}

#[derive(Debug, Deserialize)]
pub struct AIPrompt {
    pub system_intro: String,
    pub owner_name_label: String,
    pub owner_email_label: String,
    pub owner_skills_label: String,
    pub owner_bio_label: String,
    pub assistant_instruction: String,
    pub emojis_header: String,
    pub emoji_active_status: String,
    pub emoji_alert: String,
    pub emoji_warning: String,
    pub emoji_correct: String,
    pub emoji_yes: String,
    pub emoji_no: String,
    pub emoji_happy: String,
    pub emoji_love: String,
    pub emoji_thanks: String,
    pub emoji_programming: String,
    pub emoji_java: String,
    pub emoji_success: String,
    pub emoji_great: String,
    pub emoji_art: String,
    pub emoji_star: String,
    pub emoji_stats: String,
    pub emojis_instruction: String,
    pub context_header: String,
    pub user_message_format: String,
}

#[derive(Debug, Deserialize)]
pub struct AIMessages {
    pub context_saved: String,
    pub response_generated: String,
    pub error_generating: String,
    pub error_saving_context: String,
}

#[derive(Debug, Deserialize)]
pub struct AIData {
    pub stats_header: String,
    pub button_messages_label: String,
    pub conversations_label: String,
    pub total_messages_label: String,
    pub last_updated_label: String,
}

#[derive(Debug, Deserialize)]
pub struct CommissionEmbed {
    pub title: String,
    pub description: String,
    pub how_it_works_title: String,
    pub how_it_works_step1: String,
    pub how_it_works_step2: String,
    pub how_it_works_step3: String,
    pub how_it_works_step4: String,
    pub services_title: String,
    pub services_web: String,
    pub services_design: String,
    pub services_programming: String,
    pub services_consulting: String,
    pub contact_info: String,
    pub footer: String,
    pub button_text: String,
}

#[derive(Debug, Deserialize)]
pub struct CommissionCreatedEmbed {
    pub title: String,
    pub description: String,
    pub welcome_message: String,
    pub next_steps_title: String,
    pub next_steps_1: String,
    pub next_steps_2: String,
    pub next_steps_3: String,
    pub next_steps_4: String,
    pub contact_info_field: String,
    pub contact_info_value: String,
    pub close_button_text: String,
    pub footer: String,
}

#[derive(Debug, Deserialize)]
pub struct CommissionClosedEmbed {
    pub title: String,
    pub description: String,
    pub closed_by_field: String,
    pub closed_at_field: String,
    pub contact_reminder: String,
    pub footer: String,
}

#[derive(Debug, Deserialize)]
pub struct CommissionSystem {
    pub messages: CommissionMessages,
}

#[derive(Debug, Deserialize)]
pub struct CommissionMessages {
    pub setup_success: String,
    pub setup_error_permission: String,
    pub setup_error_channel: String,
    pub channel_created: String,
    pub channel_creation_failed: String,
    pub close_success: String,
    pub close_error_not_commission: String,
    pub close_error_permission: String,
    pub close_error_failed: String,
    pub already_has_commission: String,
}

#[derive(Debug, Deserialize)]
pub struct TicketSystem {
    pub embeds: TicketEmbeds,
    pub messages: TicketMessages,
}

#[derive(Debug, Deserialize)]
pub struct TicketEmbeds {
    pub setup: TicketSetupEmbed,
    pub created: TicketCreatedEmbed,
}

#[derive(Debug, Deserialize)]
pub struct TicketSetupEmbed {
    pub title: String,
    pub description: String,
    pub button_text: String,
    pub footer: String,
    pub thumbnail: String,
}

#[derive(Debug, Deserialize)]
pub struct TicketCreatedEmbed {
    pub title: String,
    pub description: String,
    pub footer: String,
    pub close_button_text: String,
}

#[derive(Debug, Deserialize)]
pub struct TicketMessages {
    pub setup_success: String,
    pub setup_error_permission: String,
    pub setup_error_channel: String,
    pub channel_created: String,
    pub channel_creation_failed: String,
    pub close_success: String,
    pub close_error_not_ticket: String,
    pub close_error_permission: String,
    pub close_error_failed: String,
    pub already_has_ticket: String,
    pub owner_notification: String,
}

#[derive(Debug, Deserialize)]
pub struct FeedbackSystem {
    pub embeds: FeedbackEmbeds,
    pub messages: FeedbackMessages,
}

#[derive(Debug, Deserialize)]
pub struct FeedbackEmbeds {
    pub setup: FeedbackSetupEmbed,
    pub message: FeedbackMessageEmbed,
}

#[derive(Debug, Deserialize)]
pub struct FeedbackSetupEmbed {
    pub title: String,
    pub description: String,
    pub how_it_works_title: String,
    pub how_it_works_step1: String,
    pub how_it_works_step2: String,
    pub how_it_works_step3: String,
    pub how_it_works_step4: String,
    pub rating_system_title: String,
    pub rating_upvote: String,
    pub rating_downvote: String,
    pub rating_stars: String,
    pub footer: String,
    pub thumbnail: String,
}

#[derive(Debug, Deserialize)]
pub struct FeedbackMessageEmbed {
    pub title: String,
    pub rating_field: String,
    pub footer: String,
}

#[derive(Debug, Deserialize)]
pub struct FeedbackMessages {
    pub setup_error_permission_title: String,
    pub setup_error_permission: String,
    pub setup_success_title: String,
    pub setup_success: String,
    pub setup_success_footer: String,
    pub content_filtered: String,
    pub no_votes_yet: String,
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

    // AI System formatting methods
    pub fn format_ai_embed_title(&self, emoji: &str) -> String {
        self.messages.ai.embeds.title_format.replace("{emoji}", emoji)
    }

    pub fn format_ai_embed_footer(&self, username: &str, emotion_emoji: &str) -> String {
        self.messages.ai.embeds.footer_format
            .replace("{username}", username)
            .replace("{emotion_emoji}", emotion_emoji)
    }

    pub fn format_ai_prompt_system_intro(&self, owner_name: &str) -> String {
        self.messages.ai.prompt.system_intro.replace("{owner_name}", owner_name)
    }

    pub fn format_ai_prompt_assistant_instruction(&self, owner_name: &str) -> String {
        self.messages.ai.prompt.assistant_instruction.replace("{owner_name}", owner_name)
    }

    pub fn format_ai_prompt_user_message(&self, message: &str) -> String {
        self.messages.ai.prompt.user_message_format.replace("{message}", message)
    }

    pub fn format_ai_context_saved(&self, user_id: &str) -> String {
        self.messages.ai.messages.context_saved.replace("{user_id}", user_id)
    }

    pub fn format_ai_response_generated(&self, username: &str, channel_id: &str) -> String {
        self.messages.ai.messages.response_generated
            .replace("{username}", username)
            .replace("{channel_id}", channel_id)
    }

    pub fn format_ai_error_generating(&self, error: &str) -> String {
        self.messages.ai.messages.error_generating.replace("{error}", error)
    }

    pub fn format_ai_error_saving_context(&self, error: &str) -> String {
        self.messages.ai.messages.error_saving_context.replace("{error}", error)
    }

    pub fn format_ai_data_stats(&self, button_count: usize, conversation_count: usize, total_messages: usize, timestamp: i64) -> String {
        format!(
            "{}\n{}\n{}\n{}\n{}",
            &self.messages.ai.data.stats_header,
            &self.messages.ai.data.button_messages_label.replace("{count}", &button_count.to_string()),
            &self.messages.ai.data.conversations_label.replace("{count}", &conversation_count.to_string()),
            &self.messages.ai.data.total_messages_label.replace("{count}", &total_messages.to_string()),
            &self.messages.ai.data.last_updated_label.replace("{timestamp}", &timestamp.to_string())
        )
    }

    // Purge command formatting methods
    pub fn format_purge_success(&self, count: u64) -> String {
        self.messages.embeds.purge.success_message.replace("{count}", &count.to_string())
    }

    pub fn format_purge_footer(&self, count: u64) -> String {
        self.messages.embeds.purge.footer.replace("{count}", &count.to_string())
    }

    pub fn format_purge_error_failed(&self, error: &str) -> String {
        self.messages.embeds.purge.error_failed.replace("{error}", error)
    }

    // Reminder command formatting methods
    pub fn format_reminder_success(&self, time: &str) -> String {
        self.messages.embeds.reminder.success_message.replace("{time}", time)
    }

    pub fn format_reminder_footer(&self, id: &str) -> String {
        self.messages.embeds.reminder.footer.replace("{id}", id)
    }

    pub fn format_reminder_error_failed(&self, error: &str) -> String {
        self.messages.embeds.reminder.error_failed.replace("{error}", error)
    }

    // Commission system formatting methods
    pub fn format_commission_setup_success(&self, channel: &str) -> String {
        self.messages.commission.messages.setup_success.replace("{channel}", channel)
    }

    pub fn format_commission_channel_created(&self, username: &str) -> String {
        self.messages.commission.messages.channel_created.replace("{username}", username)
    }

    pub fn format_commission_channel_creation_failed(&self, error: &str) -> String {
        self.messages.commission.messages.channel_creation_failed.replace("{error}", error)
    }

    pub fn format_commission_close_success(&self) -> String {
        self.messages.commission.messages.close_success.clone()
    }

    pub fn format_commission_close_error_failed(&self, error: &str) -> String {
        self.messages.commission.messages.close_error_failed.replace("{error}", error)
    }

    pub fn format_commission_already_exists(&self, channel: &str) -> String {
        self.messages.commission.messages.already_has_commission.replace("{channel}", channel)
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