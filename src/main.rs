use serenity::async_trait;
use serenity::model::gateway::Ready;
use serenity::model::prelude::*;
use serenity::model::application::CommandType;
use serenity::model::colour::Color;
use serenity::prelude::*;
use serenity::builder::{CreateCommand, CreateInteractionResponse, CreateInteractionResponseMessage, CreateEmbed};
use serenity::Client;
use std::env;
use std::time::Instant;
use chrono::Utc;

mod lang;
use lang::{LanguageManager, ImageManager, EmojiManager};

struct Handler {
    lang: LanguageManager,
    images: ImageManager,
    emojis: EmojiManager,
    start_time: Instant,
}

impl Handler {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let lang = LanguageManager::new()?;
        let images = ImageManager::new()?;
        let emojis = EmojiManager::new()?;
        Ok(Handler { 
            lang,
            images,
            emojis,
            start_time: Instant::now(),
        })
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
            CreateCommand::new(&lang_msgs.commands.userinfo.name)
                .kind(CommandType::User),
        ];

        let _ = Command::set_global_commands(&ctx.http, commands).await;
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
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
                        .color(Color::from_rgb(88, 101, 242)) // Discord blurple
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
                _ => {
                    let content = match command.data.name.as_str() {
                        "info" => lang_msgs.responses.info.clone(),
                        "hello" => self.lang.format_hello(&command.user.name),
                        "help" => lang_msgs.responses.help.clone(),
                        _ => lang_msgs.responses.unknown_command.clone(),
                    };
                    
                    let data = CreateInteractionResponseMessage::new().content(content);
                    let builder = CreateInteractionResponse::Message(data);
                    let _ = command.create_response(&ctx.http, builder).await;
                }
            }
        }
    }
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
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    
    let mut client = Client::builder(&token, intents)
        .event_handler(handler)
        .await
        .expect("Error creating client");
        
    if let Err(why) = client.start().await {
        println!("Error starting bot: {:?}", why);
    }
} 