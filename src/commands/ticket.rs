// Ticket system implementation
use serenity::all::{
    CreateEmbed, CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage, 
    CommandInteraction, Context, CreateButton, ButtonStyle, CreateActionRow,
    Color, ComponentInteraction, CreateChannel, ChannelType, PermissionOverwrite, PermissionOverwriteType,
    Permissions, CreateMessage, ChannelId
};
use serenity::model::prelude::*;
use crate::data::{DataManager, ButtonMessageData};
use crate::data::message_data::{MessageType, ButtonAction};
use crate::lang::{LanguageManager, ImageManager, EmojiManager};
use chrono::Utc;
use uuid::Uuid;

const TICKET_CHANNEL_ID: &str = "1400493422036648088";
const OWNER_ID: &str = "1400464001133056111";

/// Handle the /ticket_setup command
pub async fn handle_ticket_setup_command(
    ctx: &Context,
    command: &CommandInteraction,
    data_manager: &DataManager,
    lang: &LanguageManager,
    images: &ImageManager,
    emojis: &EmojiManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
                    .title("🎫 Ticket System Setup")
                    .description("You don't have permission to setup ticket messages. Only administrators can use this command.")
                    .color(Color::RED);
                
                let response = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new().embed(embed)
                );
                command.create_response(&ctx.http, response).await?;
                return Ok(());
            }
        }
    }

    // Get the ticket channel
    let ticket_channel_id = ChannelId::new(TICKET_CHANNEL_ID.parse::<u64>()?);
    
    // Create the ticket setup embed
    let thumbnail_url = images.get_image("avatar", "what_pointing")
        .or_else(|| images.get_image("avatar", "pointing"))
        .unwrap_or(&"https://cdn.discordapp.com/embed/avatars/0.png".to_string())
        .clone();

    let embed = CreateEmbed::new()
        .title("🎫 Support Ticket System")
        .description("Need help or have questions? Click the button below to create a private support ticket. Our team will assist you as soon as possible!")
        .color(Color::from_rgb(138, 43, 226)) // Purple theme
        .thumbnail(thumbnail_url)
        .footer(CreateEmbedFooter::new("TheLorian's Support • Click the button to get help"))
        .timestamp(Utc::now());

    // Create the button
    let button = CreateButton::new("ticket_create")
        .label("🎫 Create Ticket")
        .style(ButtonStyle::Primary);

    let action_row = CreateActionRow::Buttons(vec![button]);

    // Send the message
    let message_builder = CreateMessage::new()
        .embed(embed)
        .components(vec![action_row]);

    let sent_message = ticket_channel_id.send_message(&ctx.http, message_builder).await?;

    // Store button message data
    let mut button_data = ButtonMessageData::new(
        sent_message.id.to_string(),
        ticket_channel_id.to_string(),
        MessageType::Ticket,
    );

    button_data.add_button_action(
        "ticket_create".to_string(),
        ButtonAction::CreateTicket {
            channel_id: ticket_channel_id.to_string(),
            user_id: "".to_string(), // Will be filled when button is clicked
        },
    );

    if let Err(e) = data_manager.add_button_message(sent_message.id.to_string(), button_data) {
        eprintln!("Error storing button message data: {}", e);
    }

    // Respond to the command
    let success_embed = CreateEmbed::new()
        .title("✅ Ticket System Setup Complete")
        .description(&format!("Ticket system message created successfully in <#{}>!", TICKET_CHANNEL_ID))
        .color(Color::from_rgb(0, 255, 127))
        .timestamp(Utc::now());

    let response = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .embed(success_embed)
            .ephemeral(true)
    );
    command.create_response(&ctx.http, response).await?;

    Ok(())
}

/// Handle ticket creation when button is clicked
pub async fn handle_ticket_create(
    ctx: &Context,
    component: &ComponentInteraction,
    data_manager: &DataManager,
    lang: &LanguageManager,
    images: &ImageManager,
    emojis: &EmojiManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let user_id = component.user.id;
    let guild_id = component.guild_id.ok_or("No guild found")?;

    // Check if user already has an active ticket
    let active_tickets = data_manager.get_user_active_tickets(&user_id.to_string());
    if !active_tickets.is_empty() {
        let embed = CreateEmbed::new()
            .title("🎫 Ticket Already Exists")
            .description(&format!("You already have an active ticket channel: <#{}>", active_tickets[0]))
            .color(Color::ORANGE);

        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .embed(embed)
                .ephemeral(true)
        );
        component.create_response(&ctx.http, response).await?;
        return Ok(());
    }

    // Generate ticket ID
    let ticket_id = format!("ticket-{}-{}", user_id, Uuid::new_v4().to_string()[..8].to_lowercase());

    // Create permission overwrites
    let mut permission_overwrites = vec![
        // Deny everyone
        PermissionOverwrite {
            allow: Permissions::empty(),
            deny: Permissions::VIEW_CHANNEL,
            kind: PermissionOverwriteType::Role(guild_id.everyone_role()),
        },
        // Allow the user
        PermissionOverwrite {
            allow: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::READ_MESSAGE_HISTORY,
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Member(user_id),
        },
    ];

    // Add bot permissions
    if let Ok(current_user) = ctx.http.get_current_user().await {
        permission_overwrites.push(PermissionOverwrite {
            allow: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::READ_MESSAGE_HISTORY | Permissions::MANAGE_MESSAGES,
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Member(current_user.id),
        });
    }

    // Add owner permissions
    if let Ok(owner_id) = OWNER_ID.parse::<u64>() {
        permission_overwrites.push(PermissionOverwrite {
            allow: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES | Permissions::READ_MESSAGE_HISTORY | Permissions::MANAGE_MESSAGES,
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Member(UserId::new(owner_id)),
        });
    }

    // Create the ticket channel
    let channel_builder = CreateChannel::new(&ticket_id)
        .kind(ChannelType::Text)
        .topic(&format!("Support ticket for {}", component.user.name))
        .permissions(permission_overwrites);

    match guild_id.create_channel(&ctx.http, channel_builder).await {
        Ok(ticket_channel) => {
            // Create welcome message in the ticket channel
            let thumbnail_url = images.get_image("reactions", "happy")
                .or_else(|| images.get_default_image("success"))
                .unwrap_or(&"https://cdn.discordapp.com/embed/avatars/0.png".to_string())
                .clone();

            let welcome_embed = CreateEmbed::new()
                .title("🎫 Ticket Created Successfully")
                .description(&format!(
                    "Welcome to your support ticket, {}! Please describe your issue or question in detail. Our team has been notified and will respond shortly.\n\n<@{}>", 
                    component.user.name,
                    OWNER_ID
                ))
                .color(Color::from_rgb(138, 43, 226))
                .thumbnail(thumbnail_url)
                .field("👤 Ticket Creator", format!("<@{}>", user_id), true)
                .field("🆔 Ticket ID", &ticket_id, true)
                .field("🕒 Created", format!("<t:{}:F>", Utc::now().timestamp()), true)
                .footer(CreateEmbedFooter::new(&format!("Support Ticket • {}", &ticket_id[..16])))
                .timestamp(Utc::now());

            // Create close button
            let close_button = CreateButton::new(&format!("ticket_close_{}", ticket_id))
                .label("🗑️ Close Ticket")
                .style(ButtonStyle::Danger);

            let action_row = CreateActionRow::Buttons(vec![close_button]);

            let welcome_message = CreateMessage::new()
                .embed(welcome_embed)
                .components(vec![action_row]);

            let sent_message = ticket_channel.send_message(&ctx.http, welcome_message).await?;

            // Store ticket button data
            let mut ticket_button_data = ButtonMessageData::new(
                sent_message.id.to_string(),
                ticket_channel.id.to_string(),
                MessageType::Ticket,
            );

            ticket_button_data.add_button_action(
                format!("ticket_close_{}", ticket_id),
                ButtonAction::CloseTicket {
                    ticket_channel_id: ticket_channel.id.to_string(),
                    creator_id: user_id.to_string(),
                },
            );

            ticket_button_data.add_metadata("ticket_id".to_string(), ticket_id.clone());
            ticket_button_data.add_metadata("creator_id".to_string(), user_id.to_string());

            if let Err(e) = data_manager.add_button_message(sent_message.id.to_string(), ticket_button_data) {
                eprintln!("Error storing ticket button message data: {}", e);
            }

            // Respond to the component interaction
            let success_embed = CreateEmbed::new()
                .title("✅ Ticket Created")
                .description(&format!("Your ticket has been created: <#{}>", ticket_channel.id))
                .color(Color::from_rgb(0, 255, 127));

            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .embed(success_embed)
                    .ephemeral(true)
            );
            component.create_response(&ctx.http, response).await?;

            println!("✅ Created ticket channel {} for user {}", ticket_id, component.user.name);
        }
        Err(e) => {
            let error_embed = CreateEmbed::new()
                .title("❌ Ticket Creation Failed")
                .description(&format!("Failed to create ticket channel: {}", e))
                .color(Color::RED);

            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .embed(error_embed)
                    .ephemeral(true)
            );
            component.create_response(&ctx.http, response).await?;

            eprintln!("❌ Failed to create ticket channel for user {}: {}", component.user.name, e);
        }
    }

    Ok(())
}

/// Handle ticket closing when button is clicked
pub async fn handle_ticket_close(
    ctx: &Context,
    component: &ComponentInteraction,
    data_manager: &DataManager,
    _lang: &LanguageManager,
    _images: &ImageManager,
    _emojis: &EmojiManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let user_id = component.user.id;
    let channel_id = component.channel_id;

    // Get button message data to check permissions
    if let Some(button_data) = data_manager.get_button_message(&component.message.id.to_string()) {
        if let Some(creator_id) = button_data.get_metadata("creator_id") {
            let creator_id = creator_id.parse::<u64>()?;
            let owner_id = OWNER_ID.parse::<u64>()?;

            // Check if user has permission to close (creator or owner)
            if user_id.get() != creator_id && user_id.get() != owner_id {
                let embed = CreateEmbed::new()
                    .title("❌ Permission Denied")
                    .description("You don't have permission to close this ticket. Only the ticket creator or server owner can close tickets.")
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
            let closing_embed = CreateEmbed::new()
                .title("🗑️ Closing Ticket...")
                .description("This ticket is being closed. Thank you for using our support system!")
                .color(Color::from_rgb(255, 165, 0))
                .footer(CreateEmbedFooter::new("Ticket will be deleted in a few seconds..."))
                .timestamp(Utc::now());

            let response = CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::new()
                    .embed(closing_embed)
                    .components(vec![]) // Remove all buttons
            );
            component.create_response(&ctx.http, response).await?;

            // Wait a bit then delete the channel
            tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

            // Remove button message data
            data_manager.remove_button_message(&component.message.id.to_string());

            // Delete the channel
            if let Err(e) = channel_id.delete(&ctx.http).await {
                eprintln!("❌ Failed to delete ticket channel {}: {}", channel_id, e);
            } else {
                println!("✅ Deleted ticket channel {}", channel_id);
            }
        }
    }

    Ok(())
}

/// Handle the /ticket_close command
pub async fn handle_ticket_close_command(
    ctx: &Context,
    command: &CommandInteraction,
    data_manager: &DataManager,
    _lang: &LanguageManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let channel_id = command.channel_id;
    let user_id = command.user.id;

    // Check if this is a ticket channel
    let is_ticket_channel = channel_id.to_string().starts_with("ticket-") || 
        data_manager.is_ticket_channel(&channel_id.to_string());

    if !is_ticket_channel {
        let embed = CreateEmbed::new()
            .title("❌ Not a Ticket Channel")
            .description("This command can only be used in ticket channels.")
            .color(Color::RED);

        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed)
        );
        command.create_response(&ctx.http, response).await?;
        return Ok(());
    }

    // Check permissions (creator or owner)
    let owner_id = OWNER_ID.parse::<u64>()?;
    let has_permission = user_id.get() == owner_id || 
        data_manager.is_ticket_creator(&channel_id.to_string(), &user_id.to_string());

    if !has_permission {
        let embed = CreateEmbed::new()
            .title("❌ Permission Denied")
            .description("You don't have permission to close this ticket. Only the ticket creator or server owner can close tickets.")
            .color(Color::RED);

        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new().embed(embed)
        );
        command.create_response(&ctx.http, response).await?;
        return Ok(());
    }

    // Close the ticket
    let closing_embed = CreateEmbed::new()
        .title("🗑️ Closing Ticket...")
        .description("This ticket is being closed. Thank you for using our support system!")
        .color(Color::from_rgb(255, 165, 0))
        .footer(CreateEmbedFooter::new("Ticket will be deleted in a few seconds..."))
        .timestamp(Utc::now());

    let response = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().embed(closing_embed)
    );
    command.create_response(&ctx.http, response).await?;

    // Wait a bit then delete the channel
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Clean up data
    if let Err(e) = data_manager.cleanup_ticket_data(&channel_id.to_string()) {
        eprintln!("Error cleaning up ticket data: {}", e);
    }

    // Delete the channel
    if let Err(e) = channel_id.delete(&ctx.http).await {
        eprintln!("❌ Failed to delete ticket channel {}: {}", channel_id, e);
    } else {
        println!("✅ Deleted ticket channel {}", channel_id);
    }

    Ok(())
}