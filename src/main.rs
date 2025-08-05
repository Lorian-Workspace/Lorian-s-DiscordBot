use serenity::async_trait;
use serenity::model::gateway::Ready;
use serenity::model::prelude::*;
use serenity::all::ComponentInteractionDataKind;
use serenity::model::application::CommandType;
use serenity::model::colour::Color;
use serenity::prelude::*;
use serenity::builder::{CreateCommand, CreateInteractionResponse, CreateInteractionResponseMessage, CreateEmbed, CreateMessage, CreateSelectMenu, CreateSelectMenuOption, CreateActionRow, CreateSelectMenuKind};
use serenity::Client;
use std::env;
use std::time::Instant;
use std::sync::Arc;
use chrono::Utc;
use tokio::time::{interval, Duration};

mod lang;
mod data;
mod ai;
mod commands;

use lang::{LanguageManager, ImageManager, EmojiManager};
use data::{DataManager, AIMessage, MessageRole};
use ai::{AIManager, AIConfig};

// Wrapper para Arc<Handler> que implementa EventHandler
struct HandlerWrapper(Arc<Handler>);

#[async_trait]
impl EventHandler for HandlerWrapper {
    async fn ready(&self, ctx: Context, ready: Ready) {
        self.0.ready(ctx, ready).await;
    }
    
    async fn message(&self, ctx: Context, msg: Message) {
        self.0.message(ctx, msg).await;
    }
    
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        self.0.interaction_create(ctx, interaction).await;
    }
    
    async fn reaction_add(&self, ctx: Context, reaction: Reaction) {
        self.0.reaction_add(ctx, reaction).await;
    }
    
    async fn reaction_remove(&self, ctx: Context, reaction: Reaction) {
        self.0.reaction_remove(ctx, reaction).await;
    }
}

struct Handler {
    lang: LanguageManager,
    images: ImageManager,
    emojis: EmojiManager,
    data_manager: DataManager,
    ai_manager: AIManager,
    start_time: Instant,
}

impl Handler {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let lang = LanguageManager::new()?;
        let images = ImageManager::new()?;
        let emojis = EmojiManager::new()?;
        let data_manager = DataManager::new()?;
        
        // Migrate data if needed
        data_manager.migrate_data_if_needed()?;
        
        let ai_config = AIConfig::default();
        let ai_manager = AIManager::new(ai_config)?;
        Ok(Handler { 
            lang,
            images,
            emojis,
            data_manager,
            ai_manager,
            start_time: Instant::now(),
        })
    }
    
    /// Check if user summary should be analyzed and update if needed
    async fn check_and_analyze_user_summary(&self, user_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Check if summary analysis should be triggered (every 20 messages)
        let should_analyze = self.data_manager.increment_message_counter_and_check(user_id)?;

        if should_analyze {
            println!("Triggering summary analysis for user: {}", user_id);
            
            // Get fresh context for analysis
            if let Some(context) = self.data_manager.get_conversation_context(user_id) {
                // Analyze the summary
                match self.ai_manager.analyze_user_summary(&context, &self.lang).await {
                    Ok(Some(new_summary)) => {
                        println!("Updating user summary for: {}", user_id);
                        // Update the summary in the data manager
                        self.data_manager.update_data(|data| {
                            if let Some(user_context) = data.get_conversation_context_mut(user_id) {
                                user_context.update_summary(new_summary);
                            }
                        })?;
                    }
                    Ok(None) => {
                        println!("No summary update needed for user: {}", user_id);
                    }
                    Err(e) => {
                        eprintln!("Error analyzing summary for user {}: {}", user_id, e);
                    }
                }
            }
        }

        Ok(())
    }
    
    fn get_uptime(&self) -> String {
        let uptime = self.start_time.elapsed();
        let days = uptime.as_secs() / 86400;
        let hours = (uptime.as_secs() % 86400) / 3600;
        let minutes = (uptime.as_secs() % 3600) / 60;
        let seconds = uptime.as_secs() % 60;
        
        if days > 0 {
            format!("{}d {}h {}m {}s", days, hours, minutes, seconds)
        } else if hours > 0 {
            format!("{}h {}m {}s", hours, minutes, seconds)
        } else if minutes > 0 {
            format!("{}m {}s", minutes, seconds)
        } else {
            format!("{}s", seconds)
        }
    }
    
    fn get_memory_usage() -> f64 {
        // Get basic memory usage (this is a simplified version)
        // In a real scenario, you might want to use a more sophisticated method
        let _pid = std::process::id();
        // For now, return a placeholder. You could integrate with system monitoring libraries
        42.5 // MB placeholder
    }

    /// Get data manager statistics
    fn get_data_stats(&self) -> String {
        let stats = self.data_manager.get_stats();
        self.lang.format_ai_data_stats(
            stats.button_messages_count,
            stats.conversations_count,
            stats.total_messages,
            stats.last_updated.timestamp()
        )
    }

    /// Save current data to disk
    async fn save_data(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.data_manager.save()
    }

    /// Check and send pending reminders
    async fn check_pending_reminders(&self, ctx: &Context) -> Result<(), Box<dyn std::error::Error>> {
        let pending_reminders = self.data_manager.get_pending_reminders();
        
        for reminder in pending_reminders {
            // Create reminder notification embed
            let lang_msgs = self.lang.get();
            let embed = CreateEmbed::new()
                .title(&lang_msgs.embeds.reminder_notification.title)
                .description(&format!("{}\n\n**{}**", &lang_msgs.embeds.reminder_notification.description, reminder.message))
                .color(Color::from_rgb(255, 165, 0)) // Orange color for notifications
                .field(&lang_msgs.embeds.reminder_notification.user_field, &format!("<@{}>", reminder.user_id), true)
                .field(&lang_msgs.embeds.reminder_notification.created_field, &format!("<t:{}:R>", reminder.created_at.timestamp()), true)
                .footer(serenity::builder::CreateEmbedFooter::new(&lang_msgs.embeds.reminder_notification.footer))
                .timestamp(Utc::now());

            // Send to the reminder channel (using the channel where it was created for now)
            if let Ok(channel_id) = reminder.channel_id.parse::<u64>() {
                let channel = ChannelId::new(channel_id);
                let message = CreateMessage::new().embed(embed);
                
                match channel.send_message(&ctx.http, message).await {
                    Ok(_) => {
                        // Mark reminder as sent
                        if let Err(e) = self.data_manager.mark_reminder_sent(&reminder.id) {
                            eprintln!("Error marking reminder as sent: {}", e);
                        } else {
                            println!("Sent reminder {} to user {}", reminder.id, reminder.user_name);
                        }
                    }
                    Err(e) => {
                        eprintln!("Error sending reminder {}: {}", reminder.id, e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle AI conversation in the designated channel or ticket channels
    async fn handle_ai_message(&self, ctx: &Context, msg: &Message) -> Result<(), Box<dyn std::error::Error>> {
        // Get or create conversation context for this user
        let user_id = msg.author.id.to_string();
        let context = self.data_manager.get_conversation_context(&user_id);

        // Add user message to conversation context
        let user_message = AIMessage::new(MessageRole::User, msg.content.clone())
            .with_channel(msg.channel_id.to_string())
            .with_discord_message_id(msg.id.to_string());

        self.data_manager.add_conversation_message_with_name(&user_id, &msg.author.name, user_message)?;

        // Generate AI response
        let response_builder = self.ai_manager.generate_response(
            &msg.content,
            &user_id,
            context.as_ref(),
            &self.emojis,
            &self.lang,
            &self.images,
        ).await?;

        // Build the embed and components
        let embed = response_builder.build_embed(&self.images, &self.emojis, &msg.author.name, &self.lang);
        let action_rows = response_builder.build_action_rows();

        // Send the response
        let mut message_builder = CreateMessage::new().embed(embed);
        
        for row in action_rows {
            message_builder = message_builder.components(vec![row]);
        }

        let sent_message = msg.channel_id.send_message(&ctx.http, message_builder).await?;

        // Save AI response to conversation context
        let ai_message = AIMessage::new(MessageRole::Assistant, response_builder.get_content().to_string())
            .with_channel(msg.channel_id.to_string())
            .with_discord_message_id(sent_message.id.to_string());

        self.data_manager.add_conversation_message_with_name(&user_id, &msg.author.name, ai_message)?;

        // Check if we should analyze user summary (every 20 messages)
        if let Err(e) = self.check_and_analyze_user_summary(&user_id).await {
            eprintln!("Error analyzing user summary: {}", e);
        }

        // Ya no agregamos botones automáticamente, el sistema es más simple ahora

        println!("{}", self.lang.format_ai_response_generated(&msg.author.name, &msg.channel_id.to_string()));
        Ok(())
    }

    /// Handle reaction added to a message
    async fn reaction_add(&self, ctx: Context, reaction: Reaction) {
        // Handle feedback system reactions
        if let Err(e) = commands::handle_feedback_reaction_add(&ctx, &reaction, &self.data_manager, &self.lang, &self.emojis).await {
            eprintln!("Error handling feedback reaction add: {}", e);
        }
    }

    /// Handle reaction removed from a message
    async fn reaction_remove(&self, ctx: Context, reaction: Reaction) {
        // Handle feedback system reactions
        if let Err(e) = commands::handle_feedback_reaction_remove(&ctx, &reaction, &self.data_manager, &self.lang, &self.emojis).await {
            eprintln!("Error handling feedback reaction remove: {}", e);
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{}", self.lang.format_bot_connected(&ready.user.name));
        
        let lang_msgs = self.lang.get();
        let commands = vec![
            CreateCommand::new(&lang_msgs.commands.ping.name)
                .description(&lang_msgs.commands.ping.description),
            CreateCommand::new(&lang_msgs.commands.info.name)
                .description(&lang_msgs.commands.info.description),
            CreateCommand::new(&lang_msgs.commands.hello.name)
                .description(&lang_msgs.commands.hello.description),
            CreateCommand::new(&lang_msgs.commands.help.name)
                .description(&lang_msgs.commands.help.description),
            CreateCommand::new(&lang_msgs.commands.images.name)
                .description(&lang_msgs.commands.images.description),
            CreateCommand::new(&lang_msgs.commands.stats.name)
                .description(&lang_msgs.commands.stats.description),
            CreateCommand::new(&lang_msgs.commands.userinfo.name)
                .kind(CommandType::User),
            CreateCommand::new(&lang_msgs.commands.purge.name)
                .description(&lang_msgs.commands.purge.description)
                .add_option(serenity::builder::CreateCommandOption::new(
                    serenity::model::application::CommandOptionType::Integer,
                    "amount",
                    "Number of messages to delete (1-100)",
                ).required(true)),
            CreateCommand::new(&lang_msgs.commands.reminder.name)
                .description(&lang_msgs.commands.reminder.description)
                .add_option(serenity::builder::CreateCommandOption::new(
                    serenity::model::application::CommandOptionType::String,
                    "time",
                    "Time until reminder (e.g., 5m, 2h, 1d)",
                ).required(true))
                .add_option(serenity::builder::CreateCommandOption::new(
                    serenity::model::application::CommandOptionType::String,
                    "message",
                    "Reminder message",
                ).required(true))
                .add_option(serenity::builder::CreateCommandOption::new(
                    serenity::model::application::CommandOptionType::String,
                    "visibility",
                    "Who can see the reminder notification",
                ).required(false)
                .add_string_choice("public", "public")
                .add_string_choice("private", "private"))
                .add_option(serenity::builder::CreateCommandOption::new(
                    serenity::model::application::CommandOptionType::String,
                    "mention_type",
                    "Who to mention when the reminder is sent",
                ).required(false)
                .add_string_choice("none", "none")
                .add_string_choice("creator", "creator")
                .add_string_choice("everyone", "everyone"))
                .add_option(serenity::builder::CreateCommandOption::new(
                    serenity::model::application::CommandOptionType::Boolean,
                    "has_status",
                    "Enable status tracking with dropdown menu",
                ).required(false)),
            CreateCommand::new(&lang_msgs.commands.commission_setup.name)
                .description(&lang_msgs.commands.commission_setup.description),
            CreateCommand::new(&lang_msgs.commands.commission_close.name)
                .description(&lang_msgs.commands.commission_close.description),
            CreateCommand::new(&lang_msgs.commands.ticket_setup.name)
                .description(&lang_msgs.commands.ticket_setup.description),
            CreateCommand::new(&lang_msgs.commands.ticket_close.name)
                .description(&lang_msgs.commands.ticket_close.description),
            CreateCommand::new(&lang_msgs.commands.feedback_setup.name)
                .description(&lang_msgs.commands.feedback_setup.description),
        ];

        let _ = Command::set_global_commands(&ctx.http, commands).await;
    }

    async fn message(&self, ctx: Context, msg: Message) {
        // Ignore messages from bots
        if msg.author.bot {
            return;
        }

        // Check if this is a message in the feedback channel
        if commands::is_feedback_channel(&msg.channel_id.to_string()) {
            if let Err(e) = commands::handle_feedback_message(&ctx, &msg, &self.data_manager, &self.lang, &self.images, &self.emojis).await {
                eprintln!("Error handling feedback message: {}", e);
            }
            return;
        }

        // Check if this is a message in the AI channel (tickets excluded)
        let should_process = self.ai_manager.should_process_message(&msg.channel_id.to_string(), &msg.author.id.to_string());
            
        if should_process {
            if let Err(e) = self.handle_ai_message(&ctx, &msg).await {
                eprintln!("{}", self.lang.format_ai_error_generating(&e.to_string()));
            }
        }

        // Handle ticket channel notifications separately (without AI responses)
        if self.data_manager.is_ticket_channel(&msg.channel_id.to_string()) {
            const OWNER_ID: &str = "1400464001133056111";
            
            // Only mention if the message author is not the owner
            if msg.author.id.to_string() != OWNER_ID {
                // Send a simple mention to notify the owner (optional, can be removed if too spammy)
                // let mention_content = format!("<@{}>", OWNER_ID);
                // let _ = msg.channel_id.say(&ctx.http, &mention_content).await;
            }
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match interaction {
            Interaction::Command(command) => {
            let start_time = Instant::now();
            let lang_msgs = self.lang.get();
            
            match command.data.name.as_str() {
                "ping" => {
                    // Calculate latency
                    let latency = start_time.elapsed().as_millis() as u64;
                    let uptime = self.get_uptime();
                    let memory = Self::get_memory_usage();
                    
                    let embed = CreateEmbed::new()
                        .title(&format!("{} {}", self.emojis.success(), lang_msgs.embeds.ping.title))
                        .description(&lang_msgs.embeds.ping.description)
                        .color(Color::from_rgb(0, 255, 127)) // Spring green
                        .field(
                            &format!("{} {}", self.emojis.get_emoji("status", "up").unwrap_or(&"📶".to_string()), lang_msgs.embeds.ping.latency_field),
                            self.lang.format_ping_embed_latency(latency),
                            true,
                        )
                        .field(
                            &format!("{} {}", self.emojis.get_emoji("status", "on").unwrap_or(&"🟢".to_string()), lang_msgs.embeds.ping.uptime_field),
                            self.lang.format_ping_embed_uptime(&uptime),
                            true,
                        )
                        .field(
                            &format!("{} {}", self.emojis.get_emoji("technology", "console").unwrap_or(&"💻".to_string()), lang_msgs.embeds.ping.memory_field),
                            self.lang.format_ping_embed_memory(memory),
                            true,
                        )
                        .thumbnail(
                            self.images.get_default_image("ping_embed")
                                .unwrap_or(&lang_msgs.embeds.ping.thumbnail)
                        )
                        .footer(serenity::builder::CreateEmbedFooter::new(&lang_msgs.embeds.ping.footer))
                        .timestamp(Utc::now());
                    
                    let data = CreateInteractionResponseMessage::new().embed(embed);
                    let builder = CreateInteractionResponse::Message(data);
                    let _ = command.create_response(&ctx.http, builder).await;
                },
                "images" => {
                    // Create images gallery embed
                    let categories = self.images.list_categories();
                    let mut total_images = 0;
                    let mut description = lang_msgs.embeds.images.description.clone();
                    description.push_str("\n\n");
                    
                    for category in &categories {
                        let images_in_category = self.images.list_images_in_category(category);
                        total_images += images_in_category.len();
                        description.push_str(&format!("**{}**: {} images\n", 
                            category.to_uppercase(), images_in_category.len()));
                    }
                    
                    // Use a random avatar image for thumbnail
                    let thumbnail_url = self.images.get_random_avatar()
                        .or_else(|| self.images.get_image("avatar", "pointing"))
                        .unwrap_or(&lang_msgs.embeds.ping.thumbnail);
                    
                    let embed = CreateEmbed::new()
                        .title(&lang_msgs.embeds.images.title)
                        .description(description)
                        .color(Color::from_rgb(138, 43, 226)) // Blue violet
                        .thumbnail(thumbnail_url)
                        .footer(serenity::builder::CreateEmbedFooter::new(
                            &self.lang.format_images_embed_footer(total_images)
                        ))
                        .timestamp(Utc::now());
                    
                    let data = CreateInteractionResponseMessage::new().embed(embed);
                    let builder = CreateInteractionResponse::Message(data);
                    let _ = command.create_response(&ctx.http, builder).await;
                },
                "User Info" => {
                    // Get target user from context menu
                    let target_user = if let Some(target_id) = command.data.target_id {
                        // For user context menu commands, convert target_id to UserId first
                        let user_id = target_id.to_user_id();
                        user_id.to_user(&ctx.http).await.unwrap_or_else(|_| command.user.clone())
                    } else {
                        command.user.clone()
                    };

                    // Get member info if in a guild
                    let member_info = if let Some(guild_id) = command.guild_id {
                        guild_id.member(&ctx.http, target_user.id).await.ok()
                    } else {
                        None
                    };

                    // Format roles
                    let roles_text = if let Some(ref member) = member_info {
                        if member.roles.is_empty() {
                            lang_msgs.embeds.userinfo.no_roles.clone()
                        } else {
                            member.roles.iter()
                                .take(10) // Limit roles to prevent embed overflow
                                .map(|role_id| format!("<@&{}>", role_id))
                                .collect::<Vec<_>>()
                                .join(", ")
                        }
                    } else {
                        lang_msgs.embeds.userinfo.no_roles.clone()
                    };

                    // Format account creation date
                    let account_created = format!("<t:{}:F>", target_user.created_at().timestamp());
                    
                    // Format server join date
                    let server_joined = if let Some(ref member) = member_info {
                        if let Some(joined_at) = member.joined_at {
                            format!("<t:{}:F>", joined_at.timestamp())
                        } else {
                            "Unknown".to_string()
                        }
                    } else {
                        "Not in server".to_string()
                    };

                    // Get user avatar or default
                    let user_avatar = target_user.avatar_url().unwrap_or_else(|| target_user.default_avatar_url());

                    // Get thumbnail from bot images
                    let thumbnail_url = self.images.get_default_image("userinfo_embed")
                        .or_else(|| {
                            // Try to resolve the thumbnail reference from lang file
                            self.images.get_image("talking", &lang_msgs.embeds.userinfo.thumbnail)
                        })
                        .unwrap_or(&lang_msgs.embeds.userinfo.thumbnail);

                    let embed = CreateEmbed::new()
                        .title(&format!("{} {}", self.emojis.get_emoji("interface", "stats").unwrap_or(&"📊".to_string()), lang_msgs.embeds.userinfo.title))
                        .description(&self.lang.format_userinfo_embed_description(&target_user.name))
                        .color(Color::from_rgb(105, 90, 205)) // Our purple theme
                        .field(
                            &format!("{} {}", self.emojis.get_emoji("interface", "list").unwrap_or(&"🆔".to_string()), lang_msgs.embeds.userinfo.user_id_field),
                            target_user.id.to_string(),
                            true,
                        )
                        .field(
                            &format!("{} {}", self.emojis.get_emoji("interface", "star").unwrap_or(&"⭐".to_string()), lang_msgs.embeds.userinfo.account_created_field),
                            account_created,
                            true,
                        )
                        .field(
                            &format!("{} {}", self.emojis.get_emoji("emotions", "heart").unwrap_or(&"💖".to_string()), lang_msgs.embeds.userinfo.server_joined_field),
                            server_joined,
                            true,
                        )
                        .field(
                            &format!("{} {}", self.emojis.get_emoji("interface", "staff").unwrap_or(&"👥".to_string()), lang_msgs.embeds.userinfo.roles_field),
                            roles_text,
                            false,
                        );

                    // Add AI summary if available
                    let ai_summary_text = if let Some(context) = self.data_manager.get_conversation_context(&target_user.id.to_string()) {
                        if context.has_summary() {
                            context.get_summary().to_string()
                        } else {
                            lang_msgs.embeds.userinfo.no_ai_summary.clone()
                        }
                    } else {
                        lang_msgs.embeds.userinfo.no_ai_summary.clone()
                    };

                    let embed = embed
                        .field(
                            &format!("{} {}", self.emojis.get_emoji("emotions", "happy").unwrap_or(&"🤖".to_string()), lang_msgs.embeds.userinfo.ai_summary_field),
                            ai_summary_text,
                            false,
                        )
                        .thumbnail(thumbnail_url)
                        .image(user_avatar)
                        .footer(serenity::builder::CreateEmbedFooter::new(
                            &self.lang.format_userinfo_embed_footer(&command.user.name)
                        ))
                        .timestamp(Utc::now());

                    let data = CreateInteractionResponseMessage::new().embed(embed);
                    let builder = CreateInteractionResponse::Message(data);
                    let _ = command.create_response(&ctx.http, builder).await;
                },
                "stats" => {
                    // Handle stats command
                    if let Err(e) = commands::handle_stats_command(&ctx, &command, &self.data_manager, &self.lang).await {
                        eprintln!("Error handling stats command: {}", e);
                        let data = CreateInteractionResponseMessage::new()
                            .content("Error retrieving bot statistics.");
                        let builder = CreateInteractionResponse::Message(data);
                        let _ = command.create_response(&ctx.http, builder).await;
                    }
                },
                "purge" => {
                    // Handle purge command
                    if let Err(e) = commands::handle_purge_command(&ctx, &command, &self.lang).await {
                        eprintln!("Error handling purge command: {}", e);
                        let data = CreateInteractionResponseMessage::new()
                            .content("Error executing purge command.");
                        let builder = CreateInteractionResponse::Message(data);
                        let _ = command.create_response(&ctx.http, builder).await;
                    }
                },
                "reminder" => {
                    // Handle reminder command
                    if let Err(e) = commands::handle_reminder_command(&ctx, &command, &self.data_manager, &self.lang, &self.images, &self.emojis).await {
                        eprintln!("Error handling reminder command: {}", e);
                        let data = CreateInteractionResponseMessage::new()
                            .content("Error creating reminder.");
                        let builder = CreateInteractionResponse::Message(data);
                        let _ = command.create_response(&ctx.http, builder).await;
                    }
                },
                "commission_setup" => {
                    // Handle commission setup command
                    if let Err(e) = commands::handle_commission_setup_command(&ctx, &command, &self.data_manager, &self.lang, &self.images, &self.emojis).await {
                        eprintln!("Error handling commission setup command: {}", e);
                        let data = CreateInteractionResponseMessage::new()
                            .content("Error setting up commission system.");
                        let builder = CreateInteractionResponse::Message(data);
                        let _ = command.create_response(&ctx.http, builder).await;
                    }
                },
                "commission_close" => {
                    // Handle commission close command
                    if let Err(e) = commands::handle_commission_close_command(&ctx, &command, &self.data_manager, &self.lang).await {
                        eprintln!("Error handling commission close command: {}", e);
                        let data = CreateInteractionResponseMessage::new()
                            .content("Error closing commission.");
                        let builder = CreateInteractionResponse::Message(data);
                        let _ = command.create_response(&ctx.http, builder).await;
                    }
                },
                "ticket_setup" => {
                    // Handle ticket setup command
                    if let Err(e) = commands::handle_ticket_setup_command(&ctx, &command, &self.data_manager, &self.lang, &self.images, &self.emojis).await {
                        eprintln!("Error handling ticket setup command: {}", e);
                        let data = CreateInteractionResponseMessage::new()
                            .content("Error setting up ticket system.");
                        let builder = CreateInteractionResponse::Message(data);
                        let _ = command.create_response(&ctx.http, builder).await;
                    }
                },
                "ticket_close" => {
                    // Handle ticket close command
                    if let Err(e) = commands::handle_ticket_close_command(&ctx, &command, &self.data_manager, &self.lang).await {
                        eprintln!("Error handling ticket close command: {}", e);
                        let data = CreateInteractionResponseMessage::new()
                            .content("Error closing ticket.");
                        let builder = CreateInteractionResponse::Message(data);
                        let _ = command.create_response(&ctx.http, builder).await;
                    }
                },
                "feedback_setup" => {
                    // Handle feedback setup command
                    if let Err(e) = commands::handle_feedback_setup_command(&ctx, &command, &self.data_manager, &self.lang, &self.images, &self.emojis).await {
                        eprintln!("Error handling feedback setup command: {}", e);
                        let data = CreateInteractionResponseMessage::new()
                            .content("Error setting up feedback system.");
                        let builder = CreateInteractionResponse::Message(data);
                        let _ = command.create_response(&ctx.http, builder).await;
                    }
                },
                _ => {
                    let content = match command.data.name.as_str() {
                        "info" => lang_msgs.responses.info.clone(),
                        "hello" => self.lang.format_hello(&command.user.name),
                        "help" => {
                            // Handle new help command with dropdown
                            if let Err(e) = commands::handle_help_command(&ctx, &command, &self.lang).await {
                                eprintln!("Error handling help command: {}", e);
                                lang_msgs.responses.help.clone()
                            } else {
                                return; // Successfully handled
                            }
                        },
                        _ => lang_msgs.responses.unknown_command.clone(),
                    };
                    
                    let data = CreateInteractionResponseMessage::new().content(content);
                    let builder = CreateInteractionResponse::Message(data);
                    let _ = command.create_response(&ctx.http, builder).await;
                }
            }
            },
            Interaction::Component(component) => {
                // Handle dropdown menu and button interactions
                match component.data.custom_id.as_str() {
                    "help_select" => {
                        if let Err(e) = commands::handle_help_selection(&ctx, &component, &self.lang).await {
                            eprintln!("Error handling help selection: {}", e);
                        }
                    },
                    "help_back" => {
                        if let Err(e) = commands::handle_help_back(&ctx, &component, &self.lang).await {
                            eprintln!("Error handling help back: {}", e);
                        }
                    },
                    "commission_create" => {
                        // Handle commission creation button
                        if let Err(e) = commands::handle_commission_create(&ctx, &component, &self.data_manager, &self.lang, &self.images, &self.emojis).await {
                            eprintln!("Error handling commission creation: {}", e);
                        }
                    },
                    custom_id if custom_id.starts_with("commission_close_") => {
                        // Handle commission close button
                        if let Err(e) = commands::handle_commission_close(&ctx, &component, &self.data_manager, &self.lang, &self.images, &self.emojis).await {
                            eprintln!("Error handling commission close: {}", e);
                        }
                    },
                    "ticket_create" => {
                        // Handle ticket creation button
                        if let Err(e) = commands::handle_ticket_create(&ctx, &component, &self.data_manager, &self.lang, &self.images, &self.emojis).await {
                            eprintln!("Error handling ticket creation: {}", e);
                        }
                    },
                    custom_id if custom_id.starts_with("ticket_close_") => {
                        // Handle ticket close button
                        if let Err(e) = commands::handle_ticket_close(&ctx, &component, &self.data_manager, &self.lang, &self.images, &self.emojis).await {
                            eprintln!("Error handling ticket close: {}", e);
                        }
                    },
                    custom_id if custom_id.starts_with("reminder_status_") => {
                        // Handle reminder status selection
                        let reminder_id = custom_id.strip_prefix("reminder_status_").unwrap();
                        
                        if let ComponentInteractionDataKind::StringSelect { values } = &component.data.kind {
                            if let Some(selected_value) = values.first() {
                            // Create response based on selected status using Unicode emojis
                            let status_text = match selected_value.as_str() {
                                "confirmed" => "✅ **Confirmed** - Task has been confirmed and will be done".to_string(),
                                "created" => "⭐ **Created** - Task has been created/started".to_string(),
                                "completed" => "🎉 **Completed** - Task has been finished successfully".to_string(),
                                "cancelled" => "❌ **Cancelled** - Task has been cancelled".to_string(),
                                "failed" => "💥 **Failed** - Task failed to complete".to_string(),
                                _ => "Status updated".to_string(),
                            };
                            
                            // Get reminder details to preserve them
                            if let Some(reminder_data) = self.data_manager.get_reminder(reminder_id) {
                                // Get bell emoji and thumbnail
                                let bell_emoji = self.emojis.get_emoji("interface", "bell").map_or("🔔", |v| v);
                                let thumbnail_url = self.images.get_image("reactions", "wow_alert")
                                    .or_else(|| self.images.get_default_image("success"))
                                    .unwrap_or(&"https://cdn.discordapp.com/embed/avatars/0.png".to_string())
                                    .clone();
                                
                                let lang_msgs = self.lang.get();
                                let title_with_emoji = format!("{} {}", bell_emoji, &lang_msgs.embeds.reminder_notification.title);
                                
                                // Update the message to remove dropdown and show status while preserving original content
                                let embed = CreateEmbed::new()
                                    .title(title_with_emoji)
                                    .description(&format!("{}\n\n**{}**\n\n{}", 
                                        &lang_msgs.embeds.reminder_notification.description,
                                        reminder_data.message,
                                        status_text
                                    ))
                                    .color(Color::from_rgb(0, 255, 127)) // Green
                                    .thumbnail(thumbnail_url)
                                    .field(&lang_msgs.embeds.reminder_notification.created_field, &format!("<t:{}:R>", reminder_data.created_at.timestamp()), true)
                                    .footer(serenity::builder::CreateEmbedFooter::new("Reminder completed • Thank you!"))
                                    .timestamp(Utc::now());
                                
                                // Add user field only if it's not a private reminder
                                let embed = if !reminder_data.is_private {
                                    embed.field(&lang_msgs.embeds.reminder_notification.user_field, &format!("<@{}>", reminder_data.user_id), true)
                                } else {
                                    embed
                                };
                                
                                let edit_message = CreateInteractionResponseMessage::new()
                                    .embed(embed)
                                    .components(vec![]); // Remove all components
                                    
                                let response = CreateInteractionResponse::UpdateMessage(edit_message);
                                
                                if component.create_response(&ctx.http, response).await.is_ok() {
                                    // Delete the reminder from data since it has been completed
                                    if let Err(e) = self.data_manager.remove_reminder(reminder_id) {
                                        eprintln!("Error deleting completed reminder {}: {}", reminder_id, e);
                                    } else {
                                        println!("✅ Deleted completed reminder {} with status: {}", reminder_id, selected_value);
                                    }
                                }
                            } else {
                                // Fallback if reminder data not found
                                let embed = CreateEmbed::new()
                                    .title("📝 Reminder Status Updated")
                                    .description(&format!("Status has been set to:\n\n{}", status_text))
                                    .color(Color::from_rgb(0, 255, 127)) // Green
                                    .footer(serenity::builder::CreateEmbedFooter::new("Reminder completed • Thank you!"))
                                    .timestamp(Utc::now());
                                    
                                let edit_message = CreateInteractionResponseMessage::new()
                                    .embed(embed)
                                    .components(vec![]); // Remove all components
                                    
                                let response = CreateInteractionResponse::UpdateMessage(edit_message);
                                
                                if component.create_response(&ctx.http, response).await.is_ok() {
                                    // Delete the reminder from data since it has been completed
                                    if let Err(e) = self.data_manager.remove_reminder(reminder_id) {
                                        eprintln!("Error deleting completed reminder {}: {}", reminder_id, e);
                                    } else {
                                        println!("✅ Deleted completed reminder {} with status: {}", reminder_id, selected_value);
                                    }
                                }
                            }
                        }
                        }
                    },
                    _ => {
                        // Unknown component interaction
                        eprintln!("Unknown component interaction: {}", component.data.custom_id);
                    }
                }
            },
            _ => {} // Other interaction types
        }
    }
}



/// Check and send pending reminders (standalone function for background task)
async fn check_and_send_reminders(handler: &Handler, http: &Arc<serenity::http::Http>) -> Result<(), Box<dyn std::error::Error>> {
    let pending_reminders = handler.data_manager.get_pending_reminders();
    
    for reminder in pending_reminders {
        // Create reminder notification embed
        let lang_msgs = handler.lang.get();
        
        // Get bell emoji and wow_alert image
        let bell_emoji = handler.emojis.get_emoji("interface", "bell").map_or("🔔", |v| v);
        let thumbnail_url = handler.images.get_image("reactions", "wow_alert")
            .or_else(|| handler.images.get_default_image("success"))
            .unwrap_or(&"https://cdn.discordapp.com/embed/avatars/0.png".to_string())
            .clone();
        
        let title_with_emoji = format!("{} {}", bell_emoji, &lang_msgs.embeds.reminder_notification.title);
        
        let mut embed = CreateEmbed::new()
            .title(title_with_emoji)
            .description(&format!("{}\n\n**{}**", &lang_msgs.embeds.reminder_notification.description, reminder.message))
            .color(Color::from_rgb(255, 165, 0)) // Orange color for notifications
            .thumbnail(thumbnail_url)
            .field(&lang_msgs.embeds.reminder_notification.created_field, &format!("<t:{}:R>", reminder.created_at.timestamp()), true)
            .footer(serenity::builder::CreateEmbedFooter::new(&lang_msgs.embeds.reminder_notification.footer))
            .timestamp(Utc::now());

        // Add user field only if it's not a private reminder
        if !reminder.is_private {
            embed = embed.field(&lang_msgs.embeds.reminder_notification.user_field, &format!("<@{}>", reminder.user_id), true);
        }

        // Send to the reminder channel
        if let Ok(channel_id) = reminder.channel_id.parse::<u64>() {
            let channel = ChannelId::new(channel_id);
            let mut message_builder = CreateMessage::new().embed(embed);
            
            // Handle mentions based on mention_type and visibility
            let mention_content = if reminder.is_private {
                // For private reminders, always mention only the creator
                format!("<@{}>", reminder.user_id)
            } else {
                // For public reminders, check mention_type
                match reminder.mention_type.as_str() {
                    "creator" => format!("<@{}>", reminder.user_id),
                    "everyone" => "@everyone".to_string(),
                    _ => String::new(), // "none" or any other value
                }
            };
            
            if !mention_content.is_empty() {
                message_builder = message_builder.content(&mention_content);
            }
            
            // Add status dropdown if has_status is true
            if reminder.has_status {
                // Use standard Unicode emojis for dropdown (Discord doesn't support custom emojis in dropdowns)
                let options = vec![
                    CreateSelectMenuOption::new(
                        "✅ Confirmed",
                        "confirmed"
                    ).description("Task has been confirmed and will be done"),
                    CreateSelectMenuOption::new(
                        "⭐ Created",
                        "created"
                    ).description("Task has been created/started"),
                    CreateSelectMenuOption::new(
                        "🎉 Completed",
                        "completed"
                    ).description("Task has been finished successfully"),
                    CreateSelectMenuOption::new(
                        "❌ Cancelled",
                        "cancelled"
                    ).description("Task has been cancelled"),
                    CreateSelectMenuOption::new(
                        "💥 Failed",
                        "failed"
                    ).description("Task failed to complete"),
                ];
                
                let select_menu = CreateSelectMenu::new(
                    format!("reminder_status_{}", reminder.id),
                    CreateSelectMenuKind::String { options }
                ).placeholder("Select the status of this reminder...");
                
                let action_row = CreateActionRow::SelectMenu(select_menu);
                message_builder = message_builder.components(vec![action_row]);
            }
            
            match channel.send_message(http, message_builder).await {
                Ok(_) => {
                    // Mark reminder as sent
                    if let Err(e) = handler.data_manager.mark_reminder_sent(&reminder.id) {
                        eprintln!("Error marking reminder as sent: {}", e);
                    } else {
                        println!("✅ Sent reminder {} to user {}", reminder.id, reminder.user_name);
                    }
                }
                Err(e) => {
                    eprintln!("❌ Error sending reminder {}: {}", reminder.id, e);
                }
            }
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    
    let handler = match Handler::new() {
        Ok(h) => h,
        Err(e) => {
            eprintln!("Failed to initialize language system: {}", e);
            return;
        }
    };
    
    let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not set");
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT | GatewayIntents::GUILDS | GatewayIntents::GUILD_MESSAGE_REACTIONS;
    
    // Create Arc for sharing between client and background task
    let handler_arc = Arc::new(handler);
    let handler_for_task = Arc::clone(&handler_arc);
    
    let mut client = Client::builder(&token, intents)
        .event_handler(HandlerWrapper(Arc::clone(&handler_arc)))
        .await
        .expect("Error creating client");

    // Start reminder checking task
    let http_clone = client.http.clone();
    
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(60)); // Check every minute
        
        loop {
            interval.tick().await;
            
            // Create a simple context for sending messages
            if let Err(e) = check_and_send_reminders(&handler_for_task, &http_clone).await {
                eprintln!("Error checking reminders: {}", e);
            }
        }
    });
        
    if let Err(why) = client.start().await {
        println!("Error starting bot: {:?}", why);
    }
} 