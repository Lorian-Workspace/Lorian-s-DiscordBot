// Help command with dropdown menu functionality
use serenity::all::{
    CreateEmbed, CreateEmbedFooter, CreateInteractionResponse, CreateInteractionResponseMessage, 
    CommandInteraction, Context, CreateSelectMenu, CreateSelectMenuOption, CreateActionRow, 
    Color, CreateButton, ButtonStyle, ComponentInteraction, CreateSelectMenuKind
};
use crate::lang::LanguageManager;
use chrono::Utc;

/// Handle the /help command with dropdown menu
pub async fn handle_help_command(
    ctx: &Context,
    command: &CommandInteraction,
    lang: &LanguageManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let lang_msgs = lang.get();
    
    // Create the main help embed
    let embed = CreateEmbed::new()
        .title(&lang_msgs.embeds.help.title)
        .description(&lang_msgs.embeds.help.description)
        .color(Color::from_rgb(105, 90, 205))
        .footer(CreateEmbedFooter::new(&lang_msgs.embeds.help.footer))
        .timestamp(Utc::now());

    // Create dropdown menu with all commands
    let select_menu = CreateSelectMenu::new(
        "help_select", 
        CreateSelectMenuKind::String {
            options: vec![
                CreateSelectMenuOption::new("🏓 Ping Command", "ping")
                    .description("Check bot latency and status"),
                CreateSelectMenuOption::new("ℹ️ Info Command", "info")
                    .description("Get basic bot information"),
                CreateSelectMenuOption::new("👋 Hello Command", "hello")
                    .description("Get a personalized greeting"),
                CreateSelectMenuOption::new("📊 Stats Command", "stats")
                    .description("View bot usage statistics"),
                CreateSelectMenuOption::new("🖼️ Images Command", "images")
                    .description("Browse bot image gallery"),
                CreateSelectMenuOption::new("👤 User Info Command", "userinfo")
                    .description("Get detailed user information"),
                CreateSelectMenuOption::new("🧹 Purge Command", "purge")
                    .description("Delete multiple messages"),
                CreateSelectMenuOption::new("⏰ Reminder Command", "reminder")
                    .description("Set reminders for the future"),
            ]
        }
    )
    .placeholder(&lang_msgs.embeds.help.select_placeholder);

    let action_row = CreateActionRow::SelectMenu(select_menu);

    let response = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .embed(embed)
            .components(vec![action_row])
    );

    command.create_response(&ctx.http, response).await?;
    Ok(())
}

/// Handle help dropdown menu selection
pub async fn handle_help_selection(
    ctx: &Context,
    interaction: &ComponentInteraction,
    lang: &LanguageManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let lang_msgs = lang.get();
    let command_name = match &interaction.data.kind {
        serenity::model::application::ComponentInteractionDataKind::StringSelect { values } => &values[0],
        _ => return Ok(()),
    };

    let (embed_title, embed_description, usage, details) = match command_name.as_str() {
        "ping" => (
            &lang_msgs.embeds.help.commands.ping.title,
            &lang_msgs.embeds.help.commands.ping.description,
            &lang_msgs.embeds.help.commands.ping.usage,
            &lang_msgs.embeds.help.commands.ping.details,
        ),
        "info" => (
            &lang_msgs.embeds.help.commands.info.title,
            &lang_msgs.embeds.help.commands.info.description,
            &lang_msgs.embeds.help.commands.info.usage,
            &lang_msgs.embeds.help.commands.info.details,
        ),
        "hello" => (
            &lang_msgs.embeds.help.commands.hello.title,
            &lang_msgs.embeds.help.commands.hello.description,
            &lang_msgs.embeds.help.commands.hello.usage,
            &lang_msgs.embeds.help.commands.hello.details,
        ),
        "stats" => (
            &lang_msgs.embeds.help.commands.stats.title,
            &lang_msgs.embeds.help.commands.stats.description,
            &lang_msgs.embeds.help.commands.stats.usage,
            &lang_msgs.embeds.help.commands.stats.details,
        ),
        "images" => (
            &lang_msgs.embeds.help.commands.images.title,
            &lang_msgs.embeds.help.commands.images.description,
            &lang_msgs.embeds.help.commands.images.usage,
            &lang_msgs.embeds.help.commands.images.details,
        ),
        "userinfo" => (
            &lang_msgs.embeds.help.commands.userinfo.title,
            &lang_msgs.embeds.help.commands.userinfo.description,
            &lang_msgs.embeds.help.commands.userinfo.usage,
            &lang_msgs.embeds.help.commands.userinfo.details,
        ),
        "purge" => (
            &lang_msgs.embeds.help.commands.purge.title,
            &lang_msgs.embeds.help.commands.purge.description,
            &lang_msgs.embeds.help.commands.purge.usage,
            &lang_msgs.embeds.help.commands.purge.details,
        ),
        "reminder" => (
            &lang_msgs.embeds.help.commands.reminder.title,
            &lang_msgs.embeds.help.commands.reminder.description,
            &lang_msgs.embeds.help.commands.reminder.usage,
            &lang_msgs.embeds.help.commands.reminder.details,
        ),
        _ => {
            // Default case - shouldn't happen but good to have
            let embed = CreateEmbed::new()
                .title("❌ Unknown Command")
                .description("The selected command was not found.")
                .color(Color::RED);
            
            let response = CreateInteractionResponse::UpdateMessage(
                CreateInteractionResponseMessage::new().embed(embed)
            );
            interaction.create_response(&ctx.http, response).await?;
            return Ok(());
        }
    };

    let embed = CreateEmbed::new()
        .title(embed_title)
        .description(embed_description)
        .color(Color::from_rgb(105, 90, 205))
        .field("📝 Usage", usage, false)
        .field("📖 Details", details, false)
        .footer(CreateEmbedFooter::new("TheLorian's Bot Help System"))
        .timestamp(Utc::now());

    // Create back button
    let back_button = CreateButton::new("help_back")
        .label("← Back to Help Menu")
        .style(ButtonStyle::Secondary);

    let action_row = CreateActionRow::Buttons(vec![back_button]);

    let response = CreateInteractionResponse::UpdateMessage(
        CreateInteractionResponseMessage::new()
            .embed(embed)
            .components(vec![action_row])
    );

    interaction.create_response(&ctx.http, response).await?;
    Ok(())
}

/// Handle help back button
pub async fn handle_help_back(
    ctx: &Context,
    interaction: &ComponentInteraction,
    lang: &LanguageManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let lang_msgs = lang.get();
    
    // Recreate the main help embed and dropdown
    let embed = CreateEmbed::new()
        .title(&lang_msgs.embeds.help.title)
        .description(&lang_msgs.embeds.help.description)
        .color(Color::from_rgb(105, 90, 205))
        .footer(CreateEmbedFooter::new(&lang_msgs.embeds.help.footer))
        .timestamp(Utc::now());

    let select_menu = CreateSelectMenu::new(
        "help_select", 
        CreateSelectMenuKind::String {
            options: vec![
                CreateSelectMenuOption::new("🏓 Ping Command", "ping")
                    .description("Check bot latency and status"),
                CreateSelectMenuOption::new("ℹ️ Info Command", "info")
                    .description("Get basic bot information"),
                CreateSelectMenuOption::new("👋 Hello Command", "hello")
                    .description("Get a personalized greeting"),
                CreateSelectMenuOption::new("📊 Stats Command", "stats")
                    .description("View bot usage statistics"),
                CreateSelectMenuOption::new("🖼️ Images Command", "images")
                    .description("Browse bot image gallery"),
                CreateSelectMenuOption::new("👤 User Info Command", "userinfo")
                    .description("Get detailed user information"),
                CreateSelectMenuOption::new("🧹 Purge Command", "purge")
                    .description("Delete multiple messages"),
                CreateSelectMenuOption::new("⏰ Reminder Command", "reminder")
                    .description("Set reminders for the future"),
            ]
        }
    )
    .placeholder(&lang_msgs.embeds.help.select_placeholder);

    let action_row = CreateActionRow::SelectMenu(select_menu);

    let response = CreateInteractionResponse::UpdateMessage(
        CreateInteractionResponseMessage::new()
            .embed(embed)
            .components(vec![action_row])
    );

    interaction.create_response(&ctx.http, response).await?;
    Ok(())
}