// Módulo de comandos
pub mod help;
pub mod commission;
pub mod ticket;
pub mod feedback;

use serenity::all::{
    CreateEmbed, CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage, 
    CommandInteraction, Context, CommandDataOptionValue, Permissions, Color
};
use serenity::model::prelude::*;
use serenity::builder::GetMessages;
use crate::data::{DataManager, Reminder};
use crate::lang::LanguageManager;
use chrono::{Utc, Duration};
use uuid::Uuid;

// Re-export help functions
pub use help::{handle_help_command, handle_help_selection, handle_help_back};

// Re-export commission functions
pub use commission::{
    handle_commission_setup_command, handle_commission_create, handle_commission_close,
    handle_commission_close_command
};

// Re-export ticket functions
pub use ticket::{
    handle_ticket_setup_command, handle_ticket_create, handle_ticket_close,
    handle_ticket_close_command
};

// Re-export feedback functions
pub use feedback::{
    handle_feedback_setup_command, handle_feedback_message, handle_feedback_reaction_add,
    handle_feedback_reaction_remove, is_feedback_channel
};

/// Handle the /stats command
pub async fn handle_stats_command(
    ctx: &Context,
    command: &CommandInteraction,
    data_manager: &DataManager,
    _lang: &LanguageManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Get bot statistics
    let stats = data_manager.get_stats();
    
    // Create embed with stats
    let embed = CreateEmbed::new()
        .title("📊 Bot Statistics")
        .color(0x695acd) // Using our purple base color
        .field("💬 Total Conversations", format!("{}", stats.conversations_count), true)
        .field("📝 Total Messages", format!("{}", stats.total_messages), true)
        .field("🎯 Button Messages", format!("{}", stats.button_messages_count), true)
        .field("🕒 Last Updated", format!("<t:{}:R>", stats.last_updated.timestamp()), false)
        .footer(CreateEmbedFooter::new("TheLorian's Discord Bot"));

    let response = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .embed(embed)
    );

    command.create_response(&ctx.http, response).await?;
    
    Ok(())
}

/// Handle the /purge command
pub async fn handle_purge_command(
    ctx: &Context,
    command: &CommandInteraction,
    lang: &LanguageManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let lang_msgs = lang.get();
    
    // Get the amount parameter
    let amount = if let Some(option) = command.data.options.get(0) {
        if let CommandDataOptionValue::Integer(amount) = &option.value {
            *amount as u64
        } else {
            // Invalid parameter type
            let embed = CreateEmbed::new()
                .title(&lang_msgs.embeds.purge.title)
                .description(&lang_msgs.embeds.purge.error_invalid_amount)
                .color(Color::RED);
            
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            );
            command.create_response(&ctx.http, response).await?;
            return Ok(());
        }
    } else {
        // No parameter provided
        let embed = CreateEmbed::new()
            .title(&lang_msgs.embeds.purge.title)
            .description(&lang_msgs.embeds.purge.error_invalid_amount)
            .color(Color::RED);
        
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed)
        );
        command.create_response(&ctx.http, response).await?;
        return Ok(());
    };

    // Validate amount (1-100)
    if amount < 1 || amount > 100 {
        let embed = CreateEmbed::new()
            .title(&lang_msgs.embeds.purge.title)
            .description(&lang_msgs.embeds.purge.error_invalid_amount)
            .color(Color::RED);
        
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed)
        );
        command.create_response(&ctx.http, response).await?;
        return Ok(());
    }

    // Check if user has permission to manage messages
    if let Some(guild_id) = command.guild_id {
        if let Ok(member) = guild_id.member(&ctx.http, command.user.id).await {
            let has_permission = {
                if let Some(guild) = ctx.cache.guild(guild_id) {
                    let permissions = guild.member_permissions(&member);
                    permissions.contains(Permissions::MANAGE_MESSAGES)
                } else {
                    false
                }
            };
            
            if !has_permission {
                let embed = CreateEmbed::new()
                    .title(&lang_msgs.embeds.purge.title)
                    .description(&lang_msgs.embeds.purge.error_permission)
                    .color(Color::RED);
                
                let response = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new().embed(embed)
                );
                command.create_response(&ctx.http, response).await?;
                return Ok(());
            }
        }
    }

    // Try to delete messages
    match command.channel_id.messages(&ctx.http, GetMessages::new().limit(amount as u8)).await {
        Ok(messages) => {
            let message_ids: Vec<MessageId> = messages.iter().map(|m| m.id).collect();
            
            if message_ids.is_empty() {
                let embed = CreateEmbed::new()
                    .title(&lang_msgs.embeds.purge.title)
                    .description("No messages found to delete")
                    .color(Color::ORANGE);
                
                let response = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new().embed(embed)
                );
                command.create_response(&ctx.http, response).await?;
                return Ok(());
            }

            // Delete messages
            match command.channel_id.delete_messages(&ctx.http, &message_ids).await {
                Ok(_) => {
                    let deleted_count = message_ids.len() as u64;
                    let embed = CreateEmbed::new()
                        .title(&lang_msgs.embeds.purge.title)
                        .description(&lang.format_purge_success(deleted_count))
                        .color(Color::from_rgb(0, 255, 127))
                        .footer(CreateEmbedFooter::new(&lang.format_purge_footer(deleted_count)));
                    
                    let response = CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .embed(embed)
                            .ephemeral(true) // Make it ephemeral so only the user can see it
                    );
                    command.create_response(&ctx.http, response).await?;
                }
                Err(e) => {
                    let error_msg = format!("Failed to delete messages: {}", e);
                    let embed = CreateEmbed::new()
                        .title(&lang_msgs.embeds.purge.title)
                        .description(&error_msg)
                        .color(Color::RED);
                    
                    let response = CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new().embed(embed)
                    );
                    command.create_response(&ctx.http, response).await?;
                }
            }
        }
        Err(e) => {
            let error_msg = format!("Failed to fetch messages: {}", e);
            let embed = CreateEmbed::new()
                .title(&lang_msgs.embeds.purge.title)
                .description(&error_msg)
                .color(Color::RED);
            
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            );
            command.create_response(&ctx.http, response).await?;
        }
    }

    Ok(())
}

/// Parse time string like "5m", "2h", "1d" into Duration
fn parse_time_string(time_str: &str) -> Option<Duration> {
    if time_str.is_empty() {
        return None;
    }

    let time_str = time_str.trim();
    let (number_part, unit_part) = time_str.split_at(time_str.len() - 1);
    
    if let Ok(number) = number_part.parse::<i64>() {
        match unit_part.to_lowercase().as_str() {
            "s" => Some(Duration::seconds(number)),
            "m" => Some(Duration::minutes(number)),
            "h" => Some(Duration::hours(number)),
            "d" => Some(Duration::days(number)),
            _ => None,
        }
    } else {
        None
    }
}

/// Handle the /reminder command
pub async fn handle_reminder_command(
    ctx: &Context,
    command: &CommandInteraction,
    data_manager: &DataManager,
    lang: &LanguageManager,
    images: &crate::lang::ImageManager,
    emojis: &crate::lang::EmojiManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let lang_msgs = lang.get();
    
    // Check if user has administrator permission
    if let Some(guild_id) = command.guild_id {
        if let Ok(member) = guild_id.member(&ctx.http, command.user.id).await {
            let has_permission = {
                if let Some(guild) = ctx.cache.guild(guild_id) {
                    let permissions = guild.member_permissions(&member);
                    permissions.contains(Permissions::ADMINISTRATOR)
                } else {
                    false
                }
            };
            
            if !has_permission {
                let embed = CreateEmbed::new()
                    .title(&lang_msgs.embeds.reminder.title)
                    .description(&lang_msgs.embeds.reminder.error_permission)
                    .color(Color::RED);
                
                let response = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new().embed(embed)
                );
                command.create_response(&ctx.http, response).await?;
                return Ok(());
            }
        }
    }
    
    // Get the time, message, visibility, mention_type and has_status parameters
    let mut time_str = String::new();
    let mut reminder_message = String::new();
    let mut visibility = String::from("public"); // Default to public
    let mut mention_type = String::from("none"); // Default to no mention
    let mut has_status = false; // Default to no status tracking
    
    for option in &command.data.options {
        match option.name.as_str() {
            "time" => {
                if let CommandDataOptionValue::String(time) = &option.value {
                    time_str = time.clone();
                }
            }
            "message" => {
                if let CommandDataOptionValue::String(message) = &option.value {
                    reminder_message = message.clone();
                }
            }
            "visibility" => {
                if let CommandDataOptionValue::String(vis) = &option.value {
                    visibility = vis.clone();
                }
            }
            "mention_type" => {
                if let CommandDataOptionValue::String(mention) = &option.value {
                    mention_type = mention.clone();
                }
            }
            "has_status" => {
                if let CommandDataOptionValue::Boolean(status) = &option.value {
                    has_status = *status;
                }
            }
            _ => {}
        }
    }

    // Validate inputs
    if time_str.is_empty() || reminder_message.is_empty() {
        let embed = CreateEmbed::new()
            .title(&lang_msgs.embeds.reminder.title)
            .description(&lang_msgs.embeds.reminder.error_invalid_time)
            .color(Color::RED);
        
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed)
        );
        command.create_response(&ctx.http, response).await?;
        return Ok(());
    }

    // Check if reminder channel is configured
    if std::env::var("REMINDER_CHANNEL_ID").is_err() {
        let embed = CreateEmbed::new()
            .title(&lang_msgs.embeds.reminder.title)
            .description(&lang_msgs.embeds.reminder.error_no_channel)
            .color(Color::RED);
        
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed)
        );
        command.create_response(&ctx.http, response).await?;
        return Ok(());
    }

    // Parse the time duration
    let duration = match parse_time_string(&time_str) {
        Some(d) => d,
        None => {
            let embed = CreateEmbed::new()
                .title(&lang_msgs.embeds.reminder.title)
                .description(&lang_msgs.embeds.reminder.error_invalid_time)
                .color(Color::RED);
            
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            );
            command.create_response(&ctx.http, response).await?;
            return Ok(());
        }
    };

    // Calculate reminder time
    let reminder_time = Utc::now() + duration;
    
    // Create reminder
    let reminder_id = Uuid::new_v4().to_string();
    let is_private = visibility == "private";
    let reminder = Reminder {
        id: reminder_id.clone(),
        user_id: command.user.id.to_string(),
        user_name: command.user.name.clone(),
        message: reminder_message.clone(),
        channel_id: std::env::var("REMINDER_CHANNEL_ID").unwrap_or_else(|_| command.channel_id.to_string()),
        reminder_time,
        created_at: Utc::now(),
        is_sent: false,
        is_private,
        mention_type: mention_type.clone(),
        has_status,
    };

    // Save reminder to database
    let reminder_success = data_manager.add_reminder(reminder).is_ok();
    
    if reminder_success {
        let formatted_time = format!("<t:{}:F>", reminder_time.timestamp());
        let visibility_text = if is_private {
            &lang_msgs.embeds.reminder.visibility_private
        } else {
            &lang_msgs.embeds.reminder.visibility_public
        };
        
        let mention_text = match mention_type.as_str() {
            "creator" => &lang_msgs.embeds.reminder.mention_creator,
            "everyone" => &lang_msgs.embeds.reminder.mention_everyone,
            _ => &lang_msgs.embeds.reminder.mention_none,
        };
        
        let status_text = if has_status {
            &lang_msgs.embeds.reminder.status_enabled
        } else {
            &lang_msgs.embeds.reminder.status_disabled
        };
        
        // Get bell emoji and wow_alert image
        let bell_emoji = emojis.get_emoji("interface", "bell").map_or("🔔", |v| v);
        let thumbnail_url = images.get_image("reactions", "wow_alert")
            .or_else(|| images.get_default_image("success"))
            .unwrap_or(&"https://cdn.discordapp.com/embed/avatars/0.png".to_string())
            .clone();
        
        let title_with_emoji = format!("{} {}", bell_emoji, &lang_msgs.embeds.reminder.title);
        
        let embed = CreateEmbed::new()
            .title(title_with_emoji)
            .description(&lang.format_reminder_success(&formatted_time))
            .color(Color::from_rgb(138, 43, 226))
            .thumbnail(thumbnail_url)
            .field(&lang_msgs.embeds.reminder.time_field, &formatted_time, true)
            .field(&lang_msgs.embeds.reminder.message_field, &reminder_message, true)
            .field(&lang_msgs.embeds.reminder.channel_field, &format!("<#{}>", std::env::var("REMINDER_CHANNEL_ID").unwrap_or_else(|_| command.channel_id.to_string())), true)
            .field(&lang_msgs.embeds.reminder.visibility_field, visibility_text, true)
            .field(&lang_msgs.embeds.reminder.mention_field, mention_text, true)
            .field(&lang_msgs.embeds.reminder.status_field, status_text, true)
            .footer(CreateEmbedFooter::new(&lang.format_reminder_footer(&reminder_id[..8])))
            .timestamp(Utc::now());
        
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed)
        );
        command.create_response(&ctx.http, response).await?;
    } else {
        let embed = CreateEmbed::new()
            .title(&lang_msgs.embeds.reminder.title)
            .description("Failed to create reminder. Please try again later.")
            .color(Color::RED);
        
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed)
        );
        command.create_response(&ctx.http, response).await?;
    }

    Ok(())
} 