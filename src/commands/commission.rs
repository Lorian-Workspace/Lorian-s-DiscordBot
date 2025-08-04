use serenity::all::{
    CreateEmbed, CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage, 
    CommandInteraction, Context, Permissions, Color, CreateButton, CreateActionRow, ButtonStyle,
    ComponentInteraction, ChannelType, CreateChannel, PermissionOverwrite, PermissionOverwriteType
};
use serenity::model::prelude::*;
use crate::data::{DataManager, ButtonMessageData};
use crate::data::message_data::{MessageType, ButtonAction};
use crate::lang::{LanguageManager, ImageManager, EmojiManager};
use std::env;

/// Handle the /commission_setup command
pub async fn handle_commission_setup_command(
    ctx: &Context,
    command: &CommandInteraction,
    data_manager: &DataManager,
    lang: &LanguageManager,
    images: &ImageManager,
    emojis: &EmojiManager,
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
                    .title(&lang_msgs.embeds.commission.title)
                    .description(&lang_msgs.commission.messages.setup_error_permission)
                    .color(Color::RED);
                
                let response = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new().embed(embed)
                );
                command.create_response(&ctx.http, response).await?;
                return Ok(());
            }
        }
    }

    // Get the commission channel ID from environment or use the one from new.md
    let commission_channel_id = env::var("COMMISSION_CHANNEL_ID")
        .unwrap_or_else(|_| "1400493436993278043".to_string());
    
    // Verify the channel exists
    let channel_id = match commission_channel_id.parse::<u64>() {
        Ok(id) => ChannelId::new(id),
        Err(_) => {
            let embed = CreateEmbed::new()
                .title(&lang_msgs.embeds.commission.title)
                .description(&lang_msgs.commission.messages.setup_error_channel)
                .color(Color::RED);
            
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(embed)
            );
            command.create_response(&ctx.http, response).await?;
            return Ok(());
        }
    };

    // Create the commission embed
    let commission_embed = create_commission_embed(&lang_msgs, images, emojis);
    
    // Create the commission button
    let commission_button = CreateButton::new("commission_create")
        .style(ButtonStyle::Primary)
        .label(&lang_msgs.embeds.commission.button_text);
    
    let action_row = CreateActionRow::Buttons(vec![commission_button]);
    
    // Send the message to the commission channel
    let message_builder = serenity::builder::CreateMessage::new()
        .embed(commission_embed)
        .components(vec![action_row]);
    
    match channel_id.send_message(&ctx.http, message_builder).await {
        Ok(sent_message) => {
            // Store button message data
            let mut button_data = ButtonMessageData::new(
                sent_message.id.to_string(),
                channel_id.to_string(),
                MessageType::Commission,
            );
            
            button_data.add_button_action(
                "commission_create".to_string(),
                ButtonAction::CreateCommission {
                    channel_id: channel_id.to_string(),
                    user_id: String::new(), // Will be filled when button is clicked
                }
            );
            
            if let Err(e) = data_manager.add_button_message(sent_message.id.to_string(), button_data) {
                eprintln!("Error storing button message data: {}", e);
            }
            
            // Send success response to the user
            let success_embed = CreateEmbed::new()
                .title("✅ Commission Setup Complete")
                .description(&lang.format_commission_setup_success(&format!("<#{}>", channel_id)))
                .color(Color::from_rgb(0, 255, 127))
                .footer(CreateEmbedFooter::new("Commission System"));
            
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .embed(success_embed)
                    .ephemeral(true)
            );
            command.create_response(&ctx.http, response).await?;
        }
        Err(e) => {
            let error_embed = CreateEmbed::new()
                .title(&lang_msgs.embeds.commission.title)
                .description(&format!("Failed to setup commission message: {}", e))
                .color(Color::RED);
            
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new().embed(error_embed)
            );
            command.create_response(&ctx.http, response).await?;
        }
    }
    
    Ok(())
}

/// Handle commission button click
pub async fn handle_commission_create(
    ctx: &Context,
    component: &ComponentInteraction,
    data_manager: &DataManager,
    lang: &LanguageManager,
    images: &ImageManager,
    emojis: &EmojiManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let lang_msgs = lang.get();
    let user = &component.user;
    let guild_id = match component.guild_id {
        Some(id) => id,
        None => {
            component.create_response(&ctx.http, 
                CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new()
                        .content("This command can only be used in a server.")
                        .ephemeral(true)
                )
            ).await?;
            return Ok(());
        }
    };

    // Check if user already has an active commission channel
    if let Some(existing_channel) = get_user_commission_channel(ctx, guild_id, user.id).await? {
        let embed = CreateEmbed::new()
            .title(&lang_msgs.embeds.commission.title)
            .description(&lang.format_commission_already_exists(&format!("<#{}>", existing_channel)))
            .color(Color::ORANGE)
            .footer(CreateEmbedFooter::new("Commission System"));
        
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .embed(embed)
                .ephemeral(true)
        );
        component.create_response(&ctx.http, response).await?;
        return Ok(());
    }

    // Get owner ID for permissions
    let owner_id = env::var("DISCORD_OWNER_ID")
        .unwrap_or_else(|_| "1400464001133056111".to_string())
        .parse::<u64>()
        .unwrap_or(1400464001133056111);

    // Create private commission channel
    let channel_name = format!("commission-{}-{}", user.name.to_lowercase(), user.id);
    
    let permission_overwrites = vec![
        // Deny everyone
        PermissionOverwrite {
            allow: Permissions::empty(),
            deny: Permissions::VIEW_CHANNEL,
            kind: PermissionOverwriteType::Role(guild_id.everyone_role()),
        },
        // Allow the user who clicked the button
        PermissionOverwrite {
            allow: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::READ_MESSAGE_HISTORY,
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Member(user.id),
        },
        // Allow the bot
        PermissionOverwrite {
            allow: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::READ_MESSAGE_HISTORY,
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Member(ctx.cache.current_user().id),
        },
        // Allow the owner
        PermissionOverwrite {
            allow: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::READ_MESSAGE_HISTORY,
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Member(UserId::new(owner_id)),
        },
    ];

    let channel_builder = CreateChannel::new(&channel_name)
        .kind(ChannelType::Text)
        .permissions(permission_overwrites);

    match guild_id.create_channel(&ctx.http, channel_builder).await {
        Ok(created_channel) => {
            // Create welcome message in the new channel
            let welcome_embed = create_commission_welcome_embed(&lang_msgs, &user.name, images, emojis);
            
            // Create close button
            let close_button = CreateButton::new(format!("commission_close_{}", user.id))
                .style(ButtonStyle::Danger)
                .label(&lang_msgs.embeds.commission_created.close_button_text);
            
            let action_row = CreateActionRow::Buttons(vec![close_button]);
            
            let welcome_message = serenity::builder::CreateMessage::new()
                .embed(welcome_embed)
                .components(vec![action_row])
                .content(&format!("<@{}>", user.id)); // Mention the user
            
            if let Ok(sent_message) = created_channel.send_message(&ctx.http, welcome_message).await {
                // Store button message data for the close button
                let mut button_data = ButtonMessageData::new(
                    sent_message.id.to_string(),
                    created_channel.id.to_string(),
                    MessageType::Commission,
                );
                
                button_data.add_button_action(
                    format!("commission_close_{}", user.id),
                    ButtonAction::CloseCommission {
                        commission_channel_id: created_channel.id.to_string(),
                        creator_id: user.id.to_string(),
                    }
                );
                
                // Add metadata to identify this as a commission channel
                button_data.add_metadata("commission_creator".to_string(), user.id.to_string());
                button_data.add_metadata("commission_creator_name".to_string(), user.name.clone());
                
                if let Err(e) = data_manager.add_button_message(sent_message.id.to_string(), button_data) {
                    eprintln!("Error storing button message data: {}", e);
                }
            }

            // Send confirmation response
            let confirmation_embed = CreateEmbed::new()
                .title("✅ Commission Channel Created!")
                .description(&format!("Your private commission channel has been created: <#{}>", created_channel.id))
                .color(Color::from_rgb(0, 255, 127))
                .footer(CreateEmbedFooter::new("Commission System"));
            
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .embed(confirmation_embed)
                    .ephemeral(true)
            );
            component.create_response(&ctx.http, response).await?;
            
            println!("{}", lang.format_commission_channel_created(&user.name));
        }
        Err(e) => {
            eprintln!("Failed to create commission channel: {}", e);
            
            let error_embed = CreateEmbed::new()
                .title(&lang_msgs.embeds.commission.title)
                .description(&lang.format_commission_channel_creation_failed(&e.to_string()))
                .color(Color::RED);
            
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .embed(error_embed)
                    .ephemeral(true)
            );
            component.create_response(&ctx.http, response).await?;
        }
    }
    
    Ok(())
}

/// Handle commission close button click
pub async fn handle_commission_close(
    ctx: &Context,
    component: &ComponentInteraction,
    data_manager: &DataManager,
    lang: &LanguageManager,
    _images: &ImageManager,
    _emojis: &EmojiManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let lang_msgs = lang.get();
    let user = &component.user;
    
    // Extract creator ID from custom_id
    let creator_id = if let Some(stripped) = component.data.custom_id.strip_prefix("commission_close_") {
        stripped.parse::<u64>().unwrap_or(0)
    } else {
        0
    };
    
    // Check if user has permission to close (creator or admin)
    let can_close = user.id.get() == creator_id || {
        if let Some(guild_id) = component.guild_id {
            if let Ok(member) = guild_id.member(&ctx.http, user.id).await {
                if let Some(guild) = ctx.cache.guild(guild_id) {
                    let permissions = guild.member_permissions(&member);
                    permissions.contains(Permissions::ADMINISTRATOR)
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        }
    };
    
    if !can_close {
        let embed = CreateEmbed::new()
            .title(&lang_msgs.embeds.commission_closed.title)
            .description(&lang_msgs.commission.messages.close_error_permission)
            .color(Color::RED);
        
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .embed(embed)
                .ephemeral(true)
        );
        component.create_response(&ctx.http, response).await?;
        return Ok(());
    }

    // Create closing confirmation embed
    let close_embed = CreateEmbed::new()
        .title(&lang_msgs.embeds.commission_closed.title)
        .description(&lang_msgs.embeds.commission_closed.description)
        .color(Color::from_rgb(105, 90, 205))
        .field(&lang_msgs.embeds.commission_closed.closed_by_field, &format!("<@{}>", user.id), true)
        .field(&lang_msgs.embeds.commission_closed.closed_at_field, &format!("<t:{}:F>", chrono::Utc::now().timestamp()), true)
        .field("Contact", &lang_msgs.embeds.commission_closed.contact_reminder, false)
        .footer(CreateEmbedFooter::new(&lang_msgs.embeds.commission_closed.footer));

    // Update the message to show it's closed
    let edit_message = CreateInteractionResponseMessage::new()
        .embed(close_embed)
        .components(vec![]); // Remove all components
        
    let response = CreateInteractionResponse::UpdateMessage(edit_message);
    component.create_response(&ctx.http, response).await?;
    
    // Clean up button data from JSON storage to avoid garbage data
    let message_id = component.message.id.to_string();
    if let Err(e) = data_manager.remove_button_message(&message_id) {
        eprintln!("Warning: Could not remove button data for message {}: {}", message_id, e);
    } else {
        println!("✅ Cleaned up button data for closed commission message: {}", message_id);
    }
    
    // Delete the channel after a delay
    let channel_id = component.channel_id;
    let http_clone = ctx.http.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        if let Err(e) = channel_id.delete(&http_clone).await {
            eprintln!("Failed to delete commission channel: {}", e);
        } else {
            println!("✅ Successfully deleted commission channel: {}", channel_id);
        }
    });
    
    println!("{}", lang.format_commission_close_success());
    Ok(())
}

/// Create the main commission embed
fn create_commission_embed(
    lang_msgs: &crate::lang::Messages,
    images: &ImageManager,
    emojis: &EmojiManager,
) -> CreateEmbed {
    // Get business emoji
    let business_emoji = emojis.get_emoji("interface", "business")
        .or_else(|| emojis.get_emoji("interface", "star"))
        .map_or("💼", |v| v);
    
    // Get thumbnail - using showing1 (perfect for displaying services)
    let thumbnail_url = images.get_image("showing", "showing1")
        .or_else(|| images.get_image("avatar", "pointing"))
        .or_else(|| images.get_default_image("info"))
        .unwrap_or(&images.get_image("avatar", "what_pointing").unwrap_or(&"https://cdn.discordapp.com/embed/avatars/1.png".to_string()).clone())
        .clone();

    let mut description = lang_msgs.embeds.commission.description.clone();
    description.push_str("\n\n");
    description.push_str(&format!("**{}**\n", &lang_msgs.embeds.commission.how_it_works_title));
    description.push_str(&format!("{}\n", &lang_msgs.embeds.commission.how_it_works_step1));
    description.push_str(&format!("{}\n", &lang_msgs.embeds.commission.how_it_works_step2));
    description.push_str(&format!("{}\n", &lang_msgs.embeds.commission.how_it_works_step3));
    description.push_str(&format!("{}\n\n", &lang_msgs.embeds.commission.how_it_works_step4));
    
    description.push_str(&format!("**{}**\n", &lang_msgs.embeds.commission.services_title));
    description.push_str(&format!("{}\n", &lang_msgs.embeds.commission.services_web));
    description.push_str(&format!("{}\n", &lang_msgs.embeds.commission.services_design));
    description.push_str(&format!("{}\n", &lang_msgs.embeds.commission.services_programming));
    description.push_str(&format!("{}\n\n", &lang_msgs.embeds.commission.services_consulting));
    
    description.push_str(&format!("📧 {}", &lang_msgs.embeds.commission.contact_info));

    CreateEmbed::new()
        .title(&format!("{} {}", business_emoji, &lang_msgs.embeds.commission.title))
        .description(description)
        .color(Color::from_rgb(105, 90, 205)) // Purple theme
        .thumbnail(thumbnail_url)
        .footer(CreateEmbedFooter::new(&lang_msgs.embeds.commission.footer))
        .timestamp(chrono::Utc::now())
}

/// Create welcome embed for new commission channel
fn create_commission_welcome_embed(
    lang_msgs: &crate::lang::Messages,
    username: &str,
    images: &ImageManager,
    emojis: &EmojiManager,
) -> CreateEmbed {
    let welcome_emoji = emojis.get_emoji("emotions", "happy")
        .or_else(|| emojis.get_emoji("interface", "success"))
        .map_or("✅", |v| v);
    
    let thumbnail_url = images.get_image("emotions", "thanks")
        .or_else(|| images.get_image("emotions", "hand_on_heart"))
        .or_else(|| images.get_default_image("success"))
        .unwrap_or(&images.get_image("emotions", "love").unwrap_or(&"https://cdn.discordapp.com/embed/avatars/2.png".to_string()).clone())
        .clone();

    let welcome_msg = lang_msgs.embeds.commission_created.welcome_message
        .replace("{username}", username);

    let mut description = lang_msgs.embeds.commission_created.description.clone();
    description.push_str("\n\n");
    description.push_str(&welcome_msg);
    description.push_str("\n\n");
    description.push_str(&format!("**{}**\n", &lang_msgs.embeds.commission_created.next_steps_title));
    description.push_str(&format!("{}\n", &lang_msgs.embeds.commission_created.next_steps_1));
    description.push_str(&format!("{}\n", &lang_msgs.embeds.commission_created.next_steps_2));
    description.push_str(&format!("{}\n", &lang_msgs.embeds.commission_created.next_steps_3));
    description.push_str(&format!("{}", &lang_msgs.embeds.commission_created.next_steps_4));

    CreateEmbed::new()
        .title(&format!("{} {}", welcome_emoji, &lang_msgs.embeds.commission_created.title))
        .description(description)
        .color(Color::from_rgb(0, 255, 127)) // Green for success
        .thumbnail(thumbnail_url)
        .field(
            &lang_msgs.embeds.commission_created.contact_info_field,
            &lang_msgs.embeds.commission_created.contact_info_value,
            false
        )
        .footer(CreateEmbedFooter::new(
            &lang_msgs.embeds.commission_created.footer.replace("{username}", username)
        ))
        .timestamp(chrono::Utc::now())
}

/// Check if user already has an active commission channel
async fn get_user_commission_channel(
    ctx: &Context,
    guild_id: GuildId,
    user_id: UserId,
) -> Result<Option<ChannelId>, Box<dyn std::error::Error + Send + Sync>> {
    let channels = guild_id.channels(&ctx.http).await?;
    
    for (channel_id, channel) in channels {
        if channel.name.starts_with("commission-") && channel.name.contains(&user_id.to_string()) {
            // Simple check: if the channel name contains the user ID, they probably have access
            return Ok(Some(channel_id));
        }
    }
    
    Ok(None)
}

/// Handle the /commission_close command
pub async fn handle_commission_close_command(
    ctx: &Context,
    command: &CommandInteraction,
    data_manager: &DataManager,
    lang: &LanguageManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let lang_msgs = lang.get();
    
    // Check if this is a commission channel
    let channel_name = if let Ok(channel) = command.channel_id.to_channel(&ctx.http).await {
        if let Some(guild_channel) = channel.guild() {
            guild_channel.name.clone()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    if !channel_name.starts_with("commission-") {
        let embed = CreateEmbed::new()
            .title(&lang_msgs.embeds.commission_closed.title)
            .description(&lang_msgs.commission.messages.close_error_not_commission)
            .color(Color::RED);
        
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed)
        );
        command.create_response(&ctx.http, response).await?;
        return Ok(());
    }

    // Check permissions (creator or admin)
    let can_close = if let Some(guild_id) = command.guild_id {
        if let Ok(member) = guild_id.member(&ctx.http, command.user.id).await {
            if let Some(guild) = ctx.cache.guild(guild_id) {
                let permissions = guild.member_permissions(&member);
                permissions.contains(Permissions::ADMINISTRATOR) ||
                channel_name.contains(&command.user.id.to_string())
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };

    if !can_close {
        let embed = CreateEmbed::new()
            .title(&lang_msgs.embeds.commission_closed.title)
            .description(&lang_msgs.commission.messages.close_error_permission)
            .color(Color::RED);
        
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed)
        );
        command.create_response(&ctx.http, response).await?;
        return Ok(());
    }

    // Create closing confirmation embed
    let close_embed = CreateEmbed::new()
        .title(&lang_msgs.embeds.commission_closed.title)
        .description(&lang_msgs.embeds.commission_closed.description)
        .color(Color::from_rgb(105, 90, 205))
        .field(&lang_msgs.embeds.commission_closed.closed_by_field, &format!("<@{}>", command.user.id), true)
        .field(&lang_msgs.embeds.commission_closed.closed_at_field, &format!("<t:{}:F>", chrono::Utc::now().timestamp()), true)
        .field("Contact", &lang_msgs.embeds.commission_closed.contact_reminder, false)
        .footer(CreateEmbedFooter::new(&lang_msgs.embeds.commission_closed.footer));

    let response = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().embed(close_embed)
    );
    command.create_response(&ctx.http, response).await?;
    
    // Clean up any button data related to this commission channel
    // We need to find and remove button message data for this channel
    if let Ok(message_history) = command.channel_id.messages(&ctx.http, serenity::builder::GetMessages::new().limit(50)).await {
        for message in message_history {
            if message.author.bot {
                // Check if this message has commission button data
                if data_manager.get_button_message(&message.id.to_string()).is_some() {
                    if let Err(e) = data_manager.remove_button_message(&message.id.to_string()) {
                        eprintln!("Warning: Could not remove button data for message {}: {}", message.id, e);
                    } else {
                        println!("✅ Cleaned up button data for commission message: {}", message.id);
                    }
                }
            }
        }
    }
    
    // Delete the channel after a delay
    let channel_id = command.channel_id;
    let http_clone = ctx.http.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        if let Err(e) = channel_id.delete(&http_clone).await {
            eprintln!("Failed to delete commission channel: {}", e);
        } else {
            println!("✅ Successfully deleted commission channel: {}", channel_id);
        }
    });
    
    Ok(())
}