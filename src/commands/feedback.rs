use serenity::all::{
    CreateEmbed, CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage, 
    CommandInteraction, Context, Color, CreateMessage, ReactionType, ChannelId,
    Message
};
use crate::data::{DataManager, FeedbackMessage};
use crate::lang::{LanguageManager, ImageManager, EmojiManager};
use chrono::Utc;

// Feedback channel ID from the requirements
const FEEDBACK_CHANNEL_ID: &str = "1400466972293992498";

/// Handle the /feedback_setup command
pub async fn handle_feedback_setup_command(
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
            if !member.permissions(&ctx.cache)?.administrator() {
                let embed = CreateEmbed::new()
                    .title(&lang_msgs.feedback.messages.setup_error_permission_title)
                    .description(&lang_msgs.feedback.messages.setup_error_permission)
                    .color(Color::RED);
                
                let response = CreateInteractionResponse::Message(
                    CreateInteractionResponseMessage::new().embed(embed)
                );
                command.create_response(&ctx.http, response).await?;
                return Ok(());
            }
        }
    }

    // Get the feedback channel
    let channel_id = ChannelId::new(FEEDBACK_CHANNEL_ID.parse::<u64>()?);
    
    // Create the feedback setup embed
    let embed = CreateEmbed::new()
        .title(&format!("{} {}", 
            emojis.get_emoji("interface", "star").unwrap_or(&"⭐".to_string()),
            &lang_msgs.feedback.embeds.setup.title
        ))
        .description(&lang_msgs.feedback.embeds.setup.description)
        .color(Color::from_rgb(147, 112, 219)) // Light purple
        .field(
            &lang_msgs.feedback.embeds.setup.how_it_works_title,
            &format!("{}\n{}\n{}\n{}",
                &lang_msgs.feedback.embeds.setup.how_it_works_step1,
                &lang_msgs.feedback.embeds.setup.how_it_works_step2,
                &lang_msgs.feedback.embeds.setup.how_it_works_step3,
                &lang_msgs.feedback.embeds.setup.how_it_works_step4
            ),
            false
        )
        .field(
            &lang_msgs.feedback.embeds.setup.rating_system_title,
            &format!("{}\n{}\n{}",
                &lang_msgs.feedback.embeds.setup.rating_upvote,
                &lang_msgs.feedback.embeds.setup.rating_downvote,
                &lang_msgs.feedback.embeds.setup.rating_stars
            ),
            false
        )
        .thumbnail(
            images.get_image("reactions", "wow_alert")
                .or_else(|| images.get_default_image("feedback"))
                .unwrap_or(&lang_msgs.feedback.embeds.setup.thumbnail)
        )
        .footer(CreateEmbedFooter::new(&lang_msgs.feedback.embeds.setup.footer))
        .timestamp(Utc::now());

    // Send the setup message to the feedback channel
    let message = CreateMessage::new().embed(embed);
    let sent_message = channel_id.send_message(&ctx.http, message).await?;

    // Confirm to the admin
    let success_embed = CreateEmbed::new()
        .title(&format!("{} {}", 
            emojis.success(),
            &lang_msgs.feedback.messages.setup_success_title
        ))
        .description(&format!("{} <#{}>!", &lang_msgs.feedback.messages.setup_success, FEEDBACK_CHANNEL_ID))
        .color(Color::from_rgb(0, 255, 127))
        .footer(CreateEmbedFooter::new(&lang_msgs.feedback.messages.setup_success_footer))
        .timestamp(Utc::now());

    let response = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new().embed(success_embed)
    );
    command.create_response(&ctx.http, response).await?;

    println!("Feedback system setup completed by {}", command.user.name);
    Ok(())
}

/// Check if a message contains inappropriate content using basic word filtering
fn contains_inappropriate_content(content: &str) -> bool {
    // Basic profanity filter - in production you'd want a more sophisticated solution
    let inappropriate_words = [
        // Add inappropriate words here - keeping it basic for now
        "spam", "scam", "abuse", // Basic examples
    ];
    
    let content_lower = content.to_lowercase();
    inappropriate_words.iter().any(|word| content_lower.contains(word))
}

/// Handle a message posted in the feedback channel
pub async fn handle_feedback_message(
    ctx: &Context,
    msg: &Message,
    data_manager: &DataManager,
    lang: &LanguageManager,
    _images: &ImageManager,
    emojis: &EmojiManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Skip if it's a bot message
    if msg.author.bot {
        return Ok(());
    }

    // Check for inappropriate content
    if contains_inappropriate_content(&msg.content) {
        // Delete the original message
        if let Err(e) = msg.delete(&ctx.http).await {
            eprintln!("Failed to delete inappropriate message: {}", e);
        }
        
        // Send a warning to the user (optional)
        let lang_msgs = lang.get();
        let warning = format!("<@{}>, {}", msg.author.id, &lang_msgs.feedback.messages.content_filtered);
        let warning_msg = msg.channel_id.say(&ctx.http, warning).await?;
        
        // Delete the warning after 10 seconds
        let http = ctx.http.clone();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            let _ = warning_msg.delete(&http).await;
        });
        
        return Ok(());
    }

    // Delete the original message
    if let Err(e) = msg.delete(&ctx.http).await {
        eprintln!("Failed to delete original feedback message: {}", e);
    }

    // Create the feedback embed
    let lang_msgs = lang.get();
    let embed = CreateEmbed::new()
        .title(&format!("{} {}", 
            emojis.get_emoji("interface", "star").unwrap_or(&"⭐".to_string()),
            &lang_msgs.feedback.embeds.message.title
        ))
        .description(&msg.content)
        .color(Color::from_rgb(147, 112, 219)) // Light purple
        .author(serenity::builder::CreateEmbedAuthor::new(&msg.author.name)
            .icon_url(msg.author.avatar_url().unwrap_or_else(|| msg.author.default_avatar_url())))
        .field(
            &lang_msgs.feedback.embeds.message.rating_field,
            &generate_star_display(0, 0, emojis), // Start with 0 upvotes, 0 downvotes
            false
        )
        .footer(CreateEmbedFooter::new(&format!("{} • ID: {}", &lang_msgs.feedback.embeds.message.footer, msg.id)))
        .timestamp(Utc::now());

    // Send the feedback embed
    let message = CreateMessage::new().embed(embed);
    let sent_message = msg.channel_id.send_message(&ctx.http, message).await?;

    // Add reactions for voting using custom bot emojis
    let upvote_emoji = if let Some(up_emoji_str) = emojis.get_emoji("status", "up") {
        // Parse custom emoji: <:up:1400579660408029235>
        if let Some(emoji_id_str) = up_emoji_str.split(':').nth(2) {
            if let Some(emoji_id_str) = emoji_id_str.strip_suffix('>') {
                if let Ok(emoji_id) = emoji_id_str.parse::<u64>() {
                    ReactionType::Custom {
                        animated: false,
                        id: serenity::model::id::EmojiId::new(emoji_id),
                        name: Some("up".to_string()),
                    }
                } else {
                    ReactionType::Unicode("⬆️".to_string())
                }
            } else {
                ReactionType::Unicode("⬆️".to_string())
            }
        } else {
            ReactionType::Unicode("⬆️".to_string())
        }
    } else {
        ReactionType::Unicode("⬆️".to_string())
    };

    let downvote_emoji = if let Some(down_emoji_str) = emojis.get_emoji("status", "down") {
        // Parse custom emoji: <:down:1400579662161383594>
        if let Some(emoji_id_str) = down_emoji_str.split(':').nth(2) {
            if let Some(emoji_id_str) = emoji_id_str.strip_suffix('>') {
                if let Ok(emoji_id) = emoji_id_str.parse::<u64>() {
                    ReactionType::Custom {
                        animated: false,
                        id: serenity::model::id::EmojiId::new(emoji_id),
                        name: Some("down".to_string()),
                    }
                } else {
                    ReactionType::Unicode("⬇️".to_string())
                }
            } else {
                ReactionType::Unicode("⬇️".to_string())
            }
        } else {
            ReactionType::Unicode("⬇️".to_string())
        }
    } else {
        ReactionType::Unicode("⬇️".to_string())
    };
    
    sent_message.react(&ctx.http, upvote_emoji).await?;
    sent_message.react(&ctx.http, downvote_emoji).await?;

    // Store feedback message data
    let feedback_data = FeedbackMessage {
        message_id: sent_message.id.to_string(),
        original_author_id: msg.author.id.to_string(),
        original_author_name: msg.author.name.clone(),
        original_author_avatar: msg.author.avatar_url().unwrap_or_else(|| msg.author.default_avatar_url()),
        content: msg.content.clone(),
        channel_id: msg.channel_id.to_string(),
        upvotes: 0,
        downvotes: 0,
        created_at: Utc::now(),
    };

    // Clean old feedback messages if we have more than 30
    data_manager.clean_old_feedback_messages(30);
    
    if let Err(e) = data_manager.add_feedback_message(feedback_data) {
        eprintln!("Error saving feedback message: {}", e);
    }

    println!("Feedback message created for user: {}", msg.author.name);
    Ok(())
}

/// Generate the star display based on upvotes and downvotes
fn generate_star_display(upvotes: i32, downvotes: i32, emojis: &EmojiManager) -> String {
    let total_votes = upvotes + downvotes;
    
    // Create fallback strings that live long enough
    let star_fallback = "⭐".to_string();
    let x_fallback = "❌".to_string();
    let up_fallback = "⬆️".to_string();
    let down_fallback = "⬇️".to_string();
    
    let star_emoji = emojis.get_emoji("interface", "star").unwrap_or(&star_fallback);
    let x_emoji = emojis.get_emoji("confirmations", "x_").unwrap_or(&x_fallback);
    let up_emoji = emojis.get_emoji("status", "up").unwrap_or(&up_fallback);
    let down_emoji = emojis.get_emoji("status", "down").unwrap_or(&down_fallback);
    
    if total_votes == 0 {
        // No votes yet - show 5 empty stars
        return format!("{}{}{}{}{} (No votes yet)", x_emoji, x_emoji, x_emoji, x_emoji, x_emoji);
    }
    
    // Calculate average rating (0-5 scale)
    let rating = if total_votes > 0 {
        ((upvotes as f64 / total_votes as f64) * 5.0).floor() as i32
    } else {
        0
    };
    
    let mut display = String::new();
    
    // Add filled stars
    for _ in 0..rating {
        display.push_str(star_emoji);
    }
    
    // Add empty stars
    for _ in rating..5 {
        display.push_str(x_emoji);
    }
    
    // Add vote counts
    display.push_str(&format!(" ({} {} | {} {})", up_emoji, upvotes, down_emoji, downvotes));
    
    display
}

/// Handle reaction added to a feedback message
pub async fn handle_feedback_reaction_add(
    ctx: &Context,
    reaction: &serenity::model::channel::Reaction,
    data_manager: &DataManager,
    lang: &LanguageManager,
    emojis: &EmojiManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Skip if reaction is from a bot
    if let Ok(user) = reaction.user(&ctx.http).await {
        if user.bot {
            return Ok(());
        }
    }

    // Check if this is a feedback message
    if let Some(mut feedback_msg) = data_manager.get_feedback_message(&reaction.message_id.to_string()) {
        let mut updated = false;

        // Check if this reaction is relevant (up or down vote)
        let is_relevant_reaction = match &reaction.emoji {
            ReactionType::Custom { name: Some(name), .. } => name == "up" || name == "down",
            ReactionType::Unicode(emoji) => emoji == "⬆️" || emoji == "⬇️",
            _ => false,
        };

        if is_relevant_reaction {
            // Get the current message to read actual reaction counts
            if let Ok(message) = ctx.http.get_message(reaction.channel_id, reaction.message_id).await {
                // Count upvotes and downvotes from actual reactions
                let mut upvotes = 0;
                let mut downvotes = 0;

                for reaction_info in &message.reactions {
                    match &reaction_info.reaction_type {
                        ReactionType::Custom { name: Some(name), .. } if name == "up" => {
                            upvotes = reaction_info.count as i32;
                        },
                        ReactionType::Custom { name: Some(name), .. } if name == "down" => {
                            downvotes = reaction_info.count as i32;
                        },
                        ReactionType::Unicode(emoji) if emoji == "⬆️" => {
                            upvotes = reaction_info.count as i32;
                        },
                        ReactionType::Unicode(emoji) if emoji == "⬇️" => {
                            downvotes = reaction_info.count as i32;
                        },
                        _ => {}
                    }
                }

                // Update feedback message with real counts
                feedback_msg.upvotes = upvotes;
                feedback_msg.downvotes = downvotes;
                updated = true;
            }
        }

        if updated {
            // Update the message embed with new rating
            let lang_msgs = lang.get();
            let new_star_display = generate_star_display(feedback_msg.upvotes, feedback_msg.downvotes, emojis);
            
            let embed = CreateEmbed::new()
                .title(&format!("{} {}", 
                    emojis.get_emoji("interface", "star").unwrap_or(&"⭐".to_string()),
                    &lang_msgs.feedback.embeds.message.title
                ))
                .description(&feedback_msg.content)
                .color(Color::from_rgb(147, 112, 219))
                .author(serenity::builder::CreateEmbedAuthor::new(&feedback_msg.original_author_name)
                    .icon_url(&feedback_msg.original_author_avatar))
                .field(
                    &lang_msgs.feedback.embeds.message.rating_field,
                    &new_star_display,
                    false
                )
                .footer(CreateEmbedFooter::new(&format!("{} • ID: {}", 
                    &lang_msgs.feedback.embeds.message.footer, 
                    reaction.message_id
                )))
                .timestamp(feedback_msg.created_at);

            // Update the message
            if let Ok(mut message) = ctx.http.get_message(reaction.channel_id, reaction.message_id).await {
                let _ = message.edit(&ctx.http, 
                    serenity::builder::EditMessage::new().embed(embed)
                ).await;
            }

            // Save updated feedback data
            if let Err(e) = data_manager.update_feedback_message(feedback_msg) {
                eprintln!("Error updating feedback message: {}", e);
            }
        }
    }

    Ok(())
}

/// Handle reaction removed from a feedback message
pub async fn handle_feedback_reaction_remove(
    ctx: &Context,
    reaction: &serenity::model::channel::Reaction,
    data_manager: &DataManager,
    lang: &LanguageManager,
    emojis: &EmojiManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Skip if reaction is from a bot
    if let Ok(user) = reaction.user(&ctx.http).await {
        if user.bot {
            return Ok(());
        }
    }

    // Check if this is a feedback message
    if let Some(mut feedback_msg) = data_manager.get_feedback_message(&reaction.message_id.to_string()) {
        let mut updated = false;

        // Check if this reaction is relevant (up or down vote)
        let is_relevant_reaction = match &reaction.emoji {
            ReactionType::Custom { name: Some(name), .. } => name == "up" || name == "down",
            ReactionType::Unicode(emoji) => emoji == "⬆️" || emoji == "⬇️",
            _ => false,
        };

        if is_relevant_reaction {
            // Get the current message to read actual reaction counts
            if let Ok(message) = ctx.http.get_message(reaction.channel_id, reaction.message_id).await {
                // Count upvotes and downvotes from actual reactions
                let mut upvotes = 0;
                let mut downvotes = 0;

                for reaction_info in &message.reactions {
                    match &reaction_info.reaction_type {
                        ReactionType::Custom { name: Some(name), .. } if name == "up" => {
                            upvotes = reaction_info.count as i32;
                        },
                        ReactionType::Custom { name: Some(name), .. } if name == "down" => {
                            downvotes = reaction_info.count as i32;
                        },
                        ReactionType::Unicode(emoji) if emoji == "⬆️" => {
                            upvotes = reaction_info.count as i32;
                        },
                        ReactionType::Unicode(emoji) if emoji == "⬇️" => {
                            downvotes = reaction_info.count as i32;
                        },
                        _ => {}
                    }
                }

                // Update feedback message with real counts
                feedback_msg.upvotes = upvotes;
                feedback_msg.downvotes = downvotes;
                updated = true;
            }
        }

        if updated {
            // Update the message embed with new rating
            let lang_msgs = lang.get();
            let new_star_display = generate_star_display(feedback_msg.upvotes, feedback_msg.downvotes, emojis);
            
            let embed = CreateEmbed::new()
                .title(&format!("{} {}", 
                    emojis.get_emoji("interface", "star").unwrap_or(&"⭐".to_string()),
                    &lang_msgs.feedback.embeds.message.title
                ))
                .description(&feedback_msg.content)
                .color(Color::from_rgb(147, 112, 219))
                .author(serenity::builder::CreateEmbedAuthor::new(&feedback_msg.original_author_name)
                    .icon_url(&feedback_msg.original_author_avatar))
                .field(
                    &lang_msgs.feedback.embeds.message.rating_field,
                    &new_star_display,
                    false
                )
                .footer(CreateEmbedFooter::new(&format!("{} • ID: {}", 
                    &lang_msgs.feedback.embeds.message.footer, 
                    reaction.message_id
                )))
                .timestamp(feedback_msg.created_at);

            // Update the message
            if let Ok(mut message) = ctx.http.get_message(reaction.channel_id, reaction.message_id).await {
                let _ = message.edit(&ctx.http, 
                    serenity::builder::EditMessage::new().embed(embed)
                ).await;
            }

            // Save updated feedback data
            if let Err(e) = data_manager.update_feedback_message(feedback_msg) {
                eprintln!("Error updating feedback message: {}", e);
            }
        }
    }

    Ok(())
}

/// Update feedback message with current reaction counts
async fn update_feedback_reactions(
    http: &serenity::http::Http,
    channel_id: serenity::model::id::ChannelId,
    message_id: serenity::model::id::MessageId,
    data_manager: &DataManager,
    lang: &LanguageManager,
    emojis: &EmojiManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Get the feedback message data
    if let Some(mut feedback_msg) = data_manager.get_feedback_message(&message_id.to_string()) {
        // Get the current message to read actual reaction counts
        if let Ok(message) = http.get_message(channel_id, message_id).await {
            // Count upvotes and downvotes from actual reactions
            let mut upvotes = 0;
            let mut downvotes = 0;

            for reaction_info in &message.reactions {
                match &reaction_info.reaction_type {
                    ReactionType::Custom { name: Some(name), .. } if name == "up" => {
                        upvotes = reaction_info.count as i32;
                    },
                    ReactionType::Custom { name: Some(name), .. } if name == "down" => {
                        downvotes = reaction_info.count as i32;
                    },
                    ReactionType::Unicode(emoji) if emoji == "⬆️" => {
                        upvotes = reaction_info.count as i32;
                    },
                    ReactionType::Unicode(emoji) if emoji == "⬇️" => {
                        downvotes = reaction_info.count as i32;
                    },
                    _ => {}
                }
            }

            // Only update if counts have changed
            if feedback_msg.upvotes != upvotes || feedback_msg.downvotes != downvotes {
                // Update feedback message with real counts
                feedback_msg.upvotes = upvotes;
                feedback_msg.downvotes = downvotes;

                // Update the message embed with new rating
                let lang_msgs = lang.get();
                let new_star_display = generate_star_display(feedback_msg.upvotes, feedback_msg.downvotes, emojis);
                
                let embed = CreateEmbed::new()
                    .title(&format!("{} {}", 
                        emojis.get_emoji("interface", "star").unwrap_or(&"⭐".to_string()),
                        &lang_msgs.feedback.embeds.message.title
                    ))
                    .description(&feedback_msg.content)
                    .color(Color::from_rgb(147, 112, 219))
                    .author(serenity::builder::CreateEmbedAuthor::new(&feedback_msg.original_author_name)
                        .icon_url(&feedback_msg.original_author_avatar))
                    .field(
                        &lang_msgs.feedback.embeds.message.rating_field,
                        &new_star_display,
                        false
                    )
                    .footer(CreateEmbedFooter::new(&format!("{} • ID: {}", 
                        &lang_msgs.feedback.embeds.message.footer, 
                        message_id
                    )))
                    .timestamp(feedback_msg.created_at);

                // Update the message
                if let Ok(mut msg) = http.get_message(channel_id, message_id).await {
                    let _ = msg.edit(http, 
                        serenity::builder::EditMessage::new().embed(embed)
                    ).await;
                }

                // Save updated feedback data
                if let Err(e) = data_manager.update_feedback_message(feedback_msg) {
                    eprintln!("Error updating feedback message: {}", e);
                }
            }
        }
    }
    
    Ok(())
}

/// Check if a channel is the feedback channel
pub fn is_feedback_channel(channel_id: &str) -> bool {
    channel_id == FEEDBACK_CHANNEL_ID
}