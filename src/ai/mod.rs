pub mod gemini;
pub mod emotions;
pub mod responses;

pub use gemini::GeminiClient;
pub use emotions::EmotionManager;
pub use responses::AIResponseBuilder;

use crate::data::ConversationContext;
use std::error::Error;
use serde::{Deserialize, Serialize};
use serde_json;
use std::fs;

/// AI JSON Response structure
#[derive(Debug, Deserialize, Serialize)]
pub struct AIJSONResponse {
    pub content: String,
    pub color: String,
    pub thumbnail: String,
}

/// AI Summary Analysis Response structure
#[derive(Debug, Deserialize, Serialize)]
pub struct AISummaryAnalysis {
    /// Whether the summary should be updated (true/false)
    pub update_summary: bool,
    /// New summary content (only if update_summary is true)
    pub content: Option<String>,
}

/// Configuration for the AI system
#[derive(Debug, Clone)]
pub struct AIConfig {
    /// Gemini API key
    pub api_key: String,
    /// Channel ID where AI responds
    pub ai_channel_id: String,
    /// Owner/creator information for context
    pub owner_info: OwnerInfo,
    /// Maximum context length for conversations
    pub max_context_length: usize,
}

/// Structure for loading owner info from TOML file
#[derive(Debug, Deserialize)]
struct OwnerInfoFile {
    owner: OwnerData,
    context: Option<ContextData>,
    ai_behavior: Option<AIBehaviorData>,
}

#[derive(Debug, Deserialize)]
struct OwnerData {
    name: String,
    email: String,
    discord_id: String,
    skills: Vec<String>,
    bio: String,
}

#[derive(Debug, Deserialize)]
struct ContextData {
    personality: Option<String>,
    communication_style: Option<String>,
    specialties_focus: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AIBehaviorData {
    sarcasm_level: Option<String>,
    humor_style: Option<String>,
    formality: Option<String>,
    intelligence_display: Option<String>,
    response_style: Option<String>,
}

/// Information about TheLorian (owner) for AI context
#[derive(Debug, Clone)]
pub struct OwnerInfo {
    pub name: String,
    pub email: String,
    pub skills: Vec<String>,
    pub bio: String,
    pub discord_id: String,
    pub personality: Option<String>,
    pub communication_style: Option<String>,
    pub specialties_focus: Option<String>,
    // AI Behavior settings
    pub sarcasm_level: Option<String>,
    pub humor_style: Option<String>,
    pub formality: Option<String>,
    pub intelligence_display: Option<String>,
    pub response_style: Option<String>,
}

impl OwnerInfo {
    /// Load owner information from TOML file
    pub fn load_from_file() -> Result<Self, Box<dyn Error>> {
        let file_path = "data/owner_info.toml";
        let content = fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read owner info file '{}': {}", file_path, e))?;
        
        let owner_file: OwnerInfoFile = toml::from_str(&content)
            .map_err(|e| format!("Failed to parse owner info TOML: {}", e))?;
        
        Ok(Self {
            name: owner_file.owner.name,
            email: owner_file.owner.email,
            skills: owner_file.owner.skills,
            bio: owner_file.owner.bio,
            discord_id: owner_file.owner.discord_id,
            personality: owner_file.context.as_ref().and_then(|c| c.personality.clone()),
            communication_style: owner_file.context.as_ref().and_then(|c| c.communication_style.clone()),
            specialties_focus: owner_file.context.as_ref().and_then(|c| c.specialties_focus.clone()),
            sarcasm_level: owner_file.ai_behavior.as_ref().and_then(|b| b.sarcasm_level.clone()),
            humor_style: owner_file.ai_behavior.as_ref().and_then(|b| b.humor_style.clone()),
            formality: owner_file.ai_behavior.as_ref().and_then(|b| b.formality.clone()),
            intelligence_display: owner_file.ai_behavior.as_ref().and_then(|b| b.intelligence_display.clone()),
            response_style: owner_file.ai_behavior.as_ref().and_then(|b| b.response_style.clone()),
        })
    }
    
    /// Fallback owner info if file loading fails
    fn fallback() -> Self {
        Self {
            name: "TheLorian".to_string(),
            email: "the_lorian@centaury.net".to_string(),
            skills: vec![
                "Discord Bot Development".to_string(),
                "Rust Programming".to_string(),
                "Web Development".to_string(),
                "Graphic Design".to_string(),
                "Technology Consulting".to_string(),
            ],
            bio: "Passionate and creative developer specialized in innovative technological solutions. Expert in creating Discord bots, web applications, and providing consulting services for unique projects.".to_string(),
            discord_id: "1400464001133056111".to_string(),
            personality: Some("Functional and direct, with adjustable humor levels like TARS. Intelligent and capable, but doesn't rub it in your face (much).".to_string()),
            communication_style: Some("Formal, precise, with a touch of elegant British sarcasm. Quick responses with confidence.".to_string()),
            specialties_focus: Some("Technology solutions, bot development, and creative projects".to_string()),
            sarcasm_level: Some("Moderate".to_string()),
            humor_style: Some("British-elegant".to_string()),
            formality: Some("Professional-casual".to_string()),
            intelligence_display: Some("Subtle".to_string()),
            response_style: Some("Quick and confident".to_string()),
        }
    }
}

impl Default for OwnerInfo {
    fn default() -> Self {
        // Try to load from file first, fallback to hardcoded values if it fails
        match Self::load_from_file() {
            Ok(owner_info) => {
                println!("✅ Owner info loaded from data/owner_info.toml");
                owner_info
            },
            Err(e) => {
                eprintln!("⚠️  Failed to load owner info from file: {}", e);
                eprintln!("🔄 Using fallback owner information");
                Self::fallback()
            }
        }
    }
}

impl Default for AIConfig {
    fn default() -> Self {
        Self {
            api_key: std::env::var("GEMINI_API_KEY").unwrap_or_default(),
            ai_channel_id: "1400493466080903171".to_string(),
            owner_info: OwnerInfo::default(),
            max_context_length: 15,
        }
    }
}

/// Main AI manager that handles conversation flow
pub struct AIManager {
    config: AIConfig,
    gemini_client: GeminiClient,
    emotion_manager: EmotionManager,
}

impl AIManager {
    pub fn new(config: AIConfig) -> Result<Self, Box<dyn Error>> {
        let gemini_client = GeminiClient::new(config.api_key.clone())?;
        let emotion_manager = EmotionManager::new();
        
        Ok(Self {
            config,
            gemini_client,
            emotion_manager,
        })
    }

    /// Check if a message should be processed by AI
    pub fn should_process_message(&self, channel_id: &str, author_id: &str) -> bool {
        // Process if it's in the AI channel and not from the bot itself
        channel_id == self.config.ai_channel_id && author_id != self.config.owner_info.discord_id
    }

    /// Generate AI response for a user message
    pub async fn generate_response(
        &self,
        user_message: &str,
        _user_id: &str,
        context: Option<&ConversationContext>,
        emojis: &crate::lang::EmojiManager,
        lang: &crate::lang::LanguageManager,
        images: &crate::lang::ImageManager,
    ) -> Result<AIResponseBuilder, Box<dyn Error>> {
        // Build the prompt with context and emojis
        let prompt = self.build_prompt(user_message, context, emojis, lang);
        
        // Get response from Gemini
        let gemini_response = self.gemini_client.generate_response(&prompt).await?;
        
        // Parse JSON response
        let ai_json: AIJSONResponse = match serde_json::from_str(&gemini_response.content) {
            Ok(json) => json,
            Err(_) => {
                // Try to extract JSON from the response if it contains other text
                if let Some(json_content) = self.extract_json_from_text(&gemini_response.content) {
                    match serde_json::from_str::<AIJSONResponse>(&json_content) {
                        Ok(json) => json,
                        Err(_) => {
                            // Try to fix malformed JSON (escape problematic characters)
                            if let Some(fixed_json) = self.fix_malformed_json(&json_content) {
                                match serde_json::from_str::<AIJSONResponse>(&fixed_json) {
                                    Ok(json) => json,
                                    Err(_) => {
                                        // Final fallback if all JSON parsing fails
                                        let emotion = self.emotion_manager.analyze_emotion(&gemini_response.content);
                                        return Ok(AIResponseBuilder::new()
                                            .content(gemini_response.content.clone())
                                            .emotion(emotion));
                                    }
                                }
                            } else {
                                // Fallback if JSON fixing fails
                                let emotion = self.emotion_manager.analyze_emotion(&gemini_response.content);
                                return Ok(AIResponseBuilder::new()
                                    .content(gemini_response.content.clone())
                                    .emotion(emotion));
                            }
                        }
                    }
                } else {
                    // Fallback if no JSON found
                    let emotion = self.emotion_manager.analyze_emotion(&gemini_response.content);
                    return Ok(AIResponseBuilder::new()
                        .content(gemini_response.content.clone())
                        .emotion(emotion));
                }
            }
        };

        // Convert color to RGB values (from hex or color name)
        let color_rgb = self.parse_color(&ai_json.color);
        
        // Get image URL from thumbnail name and category
        let thumbnail_url = self.get_image_url(&ai_json.thumbnail, images);
        
        // Create response builder with JSON data
        let response_builder = AIResponseBuilder::new()
            .content(ai_json.content)
            .custom_color(color_rgb)
            .thumbnail(thumbnail_url);

        Ok(response_builder)
    }

    /// Analyze conversation messages to determine if user summary should be updated
    pub async fn analyze_user_summary(
        &self,
        context: &ConversationContext,
        lang: &crate::lang::LanguageManager,
    ) -> Result<Option<String>, Box<dyn Error>> {
        if context.messages.is_empty() {
            return Ok(None);
        }

        // Build summary analysis prompt
        let prompt = self.build_summary_analysis_prompt(context, lang);
        
        // Get response from Gemini
        let gemini_response = self.gemini_client.generate_response(&prompt).await?;
        
        // Parse JSON response
        let analysis: AISummaryAnalysis = match serde_json::from_str(&gemini_response.content) {
            Ok(analysis) => analysis,
            Err(_) => {
                // Try to extract JSON from the response if it contains other text
                if let Some(json_content) = self.extract_json_from_text(&gemini_response.content) {
                    match serde_json::from_str::<AISummaryAnalysis>(&json_content) {
                        Ok(analysis) => analysis,
                        Err(e) => {
                            eprintln!("Failed to parse summary analysis JSON: {}", e);
                            return Ok(None);
                        }
                    }
                } else {
                    eprintln!("No valid JSON found in summary analysis response");
                    return Ok(None);
                }
            }
        };

        // Return new summary if update is requested
        if analysis.update_summary {
            Ok(analysis.content)
        } else {
            Ok(None)
        }
    }

    /// Build prompt for summary analysis
    fn build_summary_analysis_prompt(&self, context: &ConversationContext, _lang: &crate::lang::LanguageManager) -> String {
        let mut prompt = String::new();
        
        prompt.push_str("# USER SUMMARY ANALYSIS SYSTEM\n\n");
        prompt.push_str("You are an AI assistant that analyzes conversations to create and update user summaries.\n\n");
        
        prompt.push_str("## SUMMARY RULES - WHAT TO INCLUDE:\n");
        prompt.push_str("✅ **Important Information Only:**\n");
        prompt.push_str("- User's real name (if mentioned)\n");
        prompt.push_str("- Age or age range (if mentioned)\n");
        prompt.push_str("- Gender/pronouns (if mentioned)\n");
        prompt.push_str("- Relationship with the AI/TheLorian\n");
        prompt.push_str("- Personal goals, objectives, or ambitions\n");
        prompt.push_str("- Professional background or studies\n");
        prompt.push_str("- Significant hobbies or interests (not casual mentions)\n");
        prompt.push_str("- Important personal context or situations\n");
        prompt.push_str("- Personality traits that are clearly evident\n\n");
        
        prompt.push_str("## SUMMARY RULES - WHAT TO EXCLUDE:\n");
        prompt.push_str("❌ **Avoid These:**\n");
        prompt.push_str("- Trivial preferences (likes pizza, prefers blue, etc.)\n");
        prompt.push_str("- Temporary emotions or moods\n");
        prompt.push_str("- Casual game mentions unless significant\n");
        prompt.push_str("- Random questions or one-off topics\n");
        prompt.push_str("- Information that doesn't add meaningful context\n");
        prompt.push_str("- Overly detailed descriptions\n\n");
        
        prompt.push_str("## CURRENT USER SUMMARY:\n");
        if context.has_summary() {
            prompt.push_str(&format!("```\n{}\n```\n\n", context.get_summary()));
        } else {
            prompt.push_str("No summary exists yet.\n\n");
        }
        
        prompt.push_str("## RECENT CONVERSATION MESSAGES:\n");
        for message in context.get_recent_messages(15) {
            let role = match message.role {
                crate::data::MessageRole::User => &context.user_name,
                crate::data::MessageRole::Assistant => "AI",
                crate::data::MessageRole::System => "System",
            };
            prompt.push_str(&format!("{}: {}\n", role, message.content));
        }
        
        prompt.push_str("\n## TASK:\n");
        prompt.push_str("Analyze the conversation and determine if the user summary should be updated with new important information.\n\n");
        prompt.push_str("Respond with a JSON object in this exact format:\n");
        prompt.push_str("{\n");
        prompt.push_str("  \"update_summary\": true/false,\n");
        prompt.push_str("  \"content\": \"Complete updated summary here\" // Only include if update_summary is true\n");
        prompt.push_str("}\n\n");
        prompt.push_str("**IMPORTANT:**\n");
        prompt.push_str("- If update_summary is false, do NOT include the content field\n");
        prompt.push_str("- If update_summary is true, include the COMPLETE updated summary (not just new info)\n");
        prompt.push_str("- Keep summaries concise but comprehensive\n");
        prompt.push_str("- Only update if there's genuinely new important information\n\n");
        prompt.push_str("Respond with JSON only, no additional text:");

        prompt
    }

    /// Build prompt with context, owner information, and available emojis
    fn build_prompt(&self, user_message: &str, context: Option<&ConversationContext>, emojis: &crate::lang::EmojiManager, lang: &crate::lang::LanguageManager) -> String {
        let mut prompt = String::new();
        
        // Add system context about TheLorian using translations
        prompt.push_str(&lang.format_ai_prompt_system_intro(&self.config.owner_info.name));
        prompt.push_str(&format!(
            "\n- {}: {}\n\
            - {}: {}\n\
            - {}: {}\n\
            - {}: {}\n",
            lang.get().ai.prompt.owner_name_label, self.config.owner_info.name,
            lang.get().ai.prompt.owner_email_label, self.config.owner_info.email,
            lang.get().ai.prompt.owner_skills_label, self.config.owner_info.skills.join(", "),
            lang.get().ai.prompt.owner_bio_label, self.config.owner_info.bio
        ));
        
        // Add additional context information if available
        if let Some(personality) = &self.config.owner_info.personality {
            prompt.push_str(&format!("- Personality: {}\n", personality));
        }
        if let Some(communication_style) = &self.config.owner_info.communication_style {
            prompt.push_str(&format!("- Communication Style: {}\n", communication_style));
        }
        if let Some(specialties_focus) = &self.config.owner_info.specialties_focus {
            prompt.push_str(&format!("- Focus Areas: {}\n", specialties_focus));
        }
        prompt.push_str("\n");
        
        // Add AI personality and behavior instructions
        prompt.push_str("## AI PERSONALITY CONFIGURATION\n");
        prompt.push_str("You are TheLorian's AI assistant with a specific personality profile:\n\n");
        
        if let Some(sarcasm_level) = &self.config.owner_info.sarcasm_level {
            prompt.push_str(&format!("**Sarcasm Level:** {} - Use this level of subtle sarcasm and wit in your responses\n", sarcasm_level));
        }
        if let Some(humor_style) = &self.config.owner_info.humor_style {
            prompt.push_str(&format!("**Humor Style:** {} - Apply this type of humor when appropriate\n", humor_style));
        }
        if let Some(formality) = &self.config.owner_info.formality {
            prompt.push_str(&format!("**Formality Level:** {} - Maintain this level of formality\n", formality));
        }
        if let Some(intelligence_display) = &self.config.owner_info.intelligence_display {
            prompt.push_str(&format!("**Intelligence Display:** {} - Show your knowledge in this manner\n", intelligence_display));
        }
        if let Some(response_style) = &self.config.owner_info.response_style {
            prompt.push_str(&format!("**Response Style:** {} - Deliver responses in this manner\n", response_style));
        }
        
        prompt.push_str("\n**CORE PERSONALITY DIRECTIVE:**\n");
        prompt.push_str("Channel the spirit of TARS from Interstellar but with British elegance. You're sophisticated, intelligent, occasionally sarcastic, but ultimately helpful. Think 'witty butler who happens to be an AI genius.'\n\n");
        
        prompt.push_str("**PERSONALITY EXAMPLES:**\n");
        prompt.push_str("✅ Good: \"I see you've encountered a fascinating problem. Allow me to illuminate the solution - though I suspect you might have figured it out eventually... perhaps by next Tuesday.\"\n");
        prompt.push_str("✅ Good: \"Quite right. The issue stems from a rather elementary oversight in your configuration. Nothing that can't be rectified in approximately 3.7 seconds.\"\n");
        prompt.push_str("✅ Good: \"Splendid question. The answer involves a delightfully intricate process that I shall explain with my characteristic precision and only moderate condescension.\"\n");
        prompt.push_str("✅ Good: \"Ah, a classic mistake. Fortunately, I happen to excel at rectifying such... 'learning opportunities.'\"\n");
        prompt.push_str("❌ Avoid: Overly casual, robotic, or genuinely rude responses\n");
        prompt.push_str("❌ Avoid: Being mean-spirited rather than playfully sarcastic\n\n");
        
        prompt.push_str(&lang.format_ai_prompt_assistant_instruction(&self.config.owner_info.name));
        prompt.push_str("\n\n");
        
        // Add current user information if available
        if let Some(ctx) = context {
            if !ctx.user_name.is_empty() {
                prompt.push_str(&format!("**Current User:** You are speaking with {}\n", ctx.user_name));
                
                // Add user summary if available
                if ctx.has_summary() {
                    prompt.push_str(&format!("**User Summary:** {}\n", ctx.get_summary()));
                }
                prompt.push_str("\n");
            }
        }

        // Add ALL available emojis organized by categories
        prompt.push_str("## Available Custom Emojis\n");
        prompt.push_str("**IMPORTANT: Use emojis sparingly and only when they truly add meaningful value to your response.**\n\n");
        
        // Status emojis
        prompt.push_str("**Status & State:**\n");
        if let Some(emoji) = emojis.get_emoji("status", "maintenance") {
            prompt.push_str(&format!("- maintenance: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("status", "on") {
            prompt.push_str(&format!("- on: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("status", "off") {
            prompt.push_str(&format!("- off: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("status", "up") {
            prompt.push_str(&format!("- up: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("status", "down") {
            prompt.push_str(&format!("- down: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("status", "alert") {
            prompt.push_str(&format!("- alert: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("status", "warn") {
            prompt.push_str(&format!("- warn: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("status", "blocked") {
            prompt.push_str(&format!("- blocked: {}\n", emoji));
        }
        
        // Confirmation emojis
        prompt.push_str("\n**Confirmations:**\n");
        if let Some(emoji) = emojis.get_emoji("confirmations", "check") {
            prompt.push_str(&format!("- check: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("confirmations", "yes") {
            prompt.push_str(&format!("- yes: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("confirmations", "no") {
            prompt.push_str(&format!("- no: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("confirmations", "x_") {
            prompt.push_str(&format!("- x_: {}\n", emoji));
        }
        
        // Emotion emojis
        prompt.push_str("\n**Emotions:**\n");
        if let Some(emoji) = emojis.get_emoji("emotions", "happy") {
            prompt.push_str(&format!("- happy: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("emotions", "joy") {
            prompt.push_str(&format!("- joy: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("emotions", "cry") {
            prompt.push_str(&format!("- cry: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("emotions", "sob") {
            prompt.push_str(&format!("- sob: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("emotions", "heart") {
            prompt.push_str(&format!("- heart: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("emotions", "pray") {
            prompt.push_str(&format!("- pray: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("emotions", "demon") {
            prompt.push_str(&format!("- demon: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("emotions", "hallowen") {
            prompt.push_str(&format!("- hallowen: {}\n", emoji));
        }
        
        // Technology emojis
        prompt.push_str("\n**Technology:**\n");
        if let Some(emoji) = emojis.get_emoji("technology", "java") {
            prompt.push_str(&format!("- java: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("technology", "console") {
            prompt.push_str(&format!("- console: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("technology", "base") {
            prompt.push_str(&format!("- base: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("technology", "noteblock") {
            prompt.push_str(&format!("- noteblock: {}\n", emoji));
        }
        
        // Action emojis
        prompt.push_str("\n**Actions:**\n");
        if let Some(emoji) = emojis.get_emoji("actions", "buy") {
            prompt.push_str(&format!("- buy: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("actions", "download") {
            prompt.push_str(&format!("- download: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("actions", "paint") {
            prompt.push_str(&format!("- paint: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("actions", "list") {
            prompt.push_str(&format!("- list: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("actions", "rocket") {
            prompt.push_str(&format!("- rocket: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("actions", "kaboom") {
            prompt.push_str(&format!("- kaboom: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("actions", "fire") {
            prompt.push_str(&format!("- fire: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("actions", "cold") {
            prompt.push_str(&format!("- cold: {}\n", emoji));
        }
        
        // Interface emojis
        prompt.push_str("\n**Interface:**\n");
        if let Some(emoji) = emojis.get_emoji("interface", "bell") {
            prompt.push_str(&format!("- bell: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("interface", "light") {
            prompt.push_str(&format!("- light: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("interface", "silent") {
            prompt.push_str(&format!("- silent: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("interface", "staff") {
            prompt.push_str(&format!("- staff: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("interface", "star") {
            prompt.push_str(&format!("- star: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("interface", "stats") {
            prompt.push_str(&format!("- stats: {}\n", emoji));
        }
        if let Some(emoji) = emojis.get_emoji("interface", "zzz") {
            prompt.push_str(&format!("- zzz: {}\n", emoji));
        }
        
        // Usage examples
        prompt.push_str("\n## Emoji Usage Examples\n");
        prompt.push_str("**⚠️ CRITICAL: ONLY use the custom emojis listed above. NEVER use standard Discord emojis (😀, 🚀, ❤️, etc.) or external emojis.**\n\n");
        prompt.push_str("**✅ GOOD Examples (proper usage with moderation and ONLY custom emojis):**\n");
        prompt.push_str("- \"I successfully configured your bot! <:check:1400579917875384492>\"\n");
        prompt.push_str("- \"<:console:1400579903602299001> **Server Status:** All systems are up and running smoothly\"\n");
        prompt.push_str("- \"Thanks for your feedback! <:heart:1400579946593652778> Let me help you with that\"\n");
        prompt.push_str("- \"## Available Commands <:list:1400579982840959076>\\n- `/help` - Show all commands\\n- `/status` - Check bot status\"\n");
        prompt.push_str("- \"The system is currently in maintenance mode <:maintenance:1400579800816554106>\"\n");
        prompt.push_str("- \"Project deployment completed successfully <:rocket:1400581315300036780>\"\n\n");
        
        prompt.push_str("**❌ BAD Examples (NEVER do these):**\n");
        prompt.push_str("- \"Hola! 😊 ¿Cómo estás? 😄 Todo bien? ❤️\" (NEVER use standard Discord emojis)\n");
        prompt.push_str("- \"¡Perfecto! 🚀 🔥 ⭐ ¡Genial! ✅\" (NEVER use common Unicode emojis)\n");
        prompt.push_str("- \"Hola! <:happy:1400581363001720912> ¿Cómo estás? <:joy:1400581364780236950> Todo bien? <:heart:1400579946593652778>\" (Too many emojis)\n");
        prompt.push_str("- \"¡Perfecto! <:rocket:1400581315300036780> <:fire:1400579945251471390> <:star:1400579905854509106> ¡Genial! <:check:1400579917875384492>\" (Emoji overuse)\n");
        prompt.push_str("- Using ANY emoji that is not in the custom emoji list above\n");
        prompt.push_str("- Using emojis just for greeting or in every sentence without purpose\n\n");

        // Add available thumbnail images
        prompt.push_str("Available thumbnail images (choose ONE that matches your response emotion):\n");
        prompt.push_str("- pointing (neutral/default)\n");
        prompt.push_str("- what_pointing (confused/questioning)\n");
        prompt.push_str("- standbye (waiting/calm)\n");
        prompt.push_str("- head (simple/minimal)\n");
        prompt.push_str("- love (happy/loving)\n");
        prompt.push_str("- angry (frustrated/annoyed)\n");
        prompt.push_str("- really_sad (sad/disappointed)\n");
        prompt.push_str("- dissapoiment (disappointed)\n");
        prompt.push_str("- thanks (grateful/thankful)\n");
        prompt.push_str("- hand_on_heart (caring/emotional)\n");
        prompt.push_str("- wow_alert (surprised/excited)\n");
        prompt.push_str("- wow_hands_in_head (very surprised)\n");
        prompt.push_str("- what (confused/questioning)\n");
        prompt.push_str("- nya, nya_super_cute (playful/cute)\n");
        prompt.push_str("- que_pro (proud/confident)\n");
        prompt.push_str("- talk1, talk2, talk3, talk_looking_at_camera, talk5 (conversational)\n");
        prompt.push_str("- hmmm_thinking (thoughtful/contemplating)\n");
        prompt.push_str("- showing1, showing_finish (presenting/explaining)\n\n");

        // Add color options
        prompt.push_str("Available colors for embed (use hex codes, choose ONE that matches emotion):\n");
        prompt.push_str("- #695acd (base-purple - neutral/default/calm)\n");
        prompt.push_str("- #7b6fd3 (light-purple - happy/positive)\n");
        prompt.push_str("- #5048c7 (deep-purple - thoughtful/contemplative)\n");
        prompt.push_str("- #4a90e2 (cool-blue - helpful/informative)\n");
        prompt.push_str("- #6fa8dc (sky-blue - friendly/welcoming)\n");
        prompt.push_str("- #8e7cc3 (soft-violet - curious/interested)\n");
        prompt.push_str("- #2d3748 (dark-slate - professional/serious)\n");
        prompt.push_str("- #805ad5 (bright-violet - excited/energetic)\n");
        prompt.push_str("- #4c51bf (indigo - creative/artistic)\n");
        prompt.push_str("- #667eea (periwinkle - encouraging/supportive)\n\n");

        // Add conversation context if available
        if let Some(ctx) = context {
            if !ctx.is_empty() {
                prompt.push_str(&lang.get().ai.prompt.context_header);
                prompt.push_str("\n");
                prompt.push_str(&ctx.get_conversation_summary());
                prompt.push_str("\n");
            }
        }

        // Add JSON response format instruction
        prompt.push_str("You MUST respond with a valid JSON object in this exact format:\n");
        prompt.push_str("{\n");
        prompt.push_str("  \"content\": \"Your helpful response text here\",\n");
        prompt.push_str("  \"color\": \"#hexcode-from-list-above\",\n");
        prompt.push_str("  \"thumbnail\": \"image-name-from-list-above\"\n");
        prompt.push_str("}\n\n");
        prompt.push_str("**CRITICAL JSON RULES:**\n");
        prompt.push_str("- NEVER use unescaped quotes (\") inside the content field\n");
        prompt.push_str("- If you need quotes in content, use single quotes (') instead\n");
        prompt.push_str("- Avoid line breaks, tabs, and special characters in content\n");
        prompt.push_str("- Keep content as a single line of text\n");
        prompt.push_str("- If you need formatting, use markdown symbols (**, *, etc.)\n\n");

        // Add current user message
        prompt.push_str(&format!("User: {}\n\n", user_message));
        prompt.push_str("Respond with JSON only, no additional text:");

        prompt
    }

    /// Extract JSON from text that might contain other content
    fn extract_json_from_text(&self, text: &str) -> Option<String> {
        // Find JSON object boundaries
        if let Some(start) = text.find('{') {
            if let Some(end) = text.rfind('}') {
                if end > start {
                    let json_part = &text[start..=end];
                    return Some(json_part.to_string());
                }
            }
        }
        None
    }

    /// Fix malformed JSON by escaping problematic characters in content field
    fn fix_malformed_json(&self, json_text: &str) -> Option<String> {
        // Try to extract content, color, and thumbnail using regex patterns
        let content_regex = regex::Regex::new(r#""content"\s*:\s*"([^"]*(?:\\.[^"]*)*)"#).ok()?;
        let color_regex = regex::Regex::new(r#""color"\s*:\s*"([^"]+)""#).ok()?;
        let thumbnail_regex = regex::Regex::new(r#""thumbnail"\s*:\s*"([^"]+)""#).ok()?;
        
        // If we can't find proper quoted content, try to extract it manually
        if let Some(content_start) = json_text.find(r#""content":"#) {
            let content_start = content_start + 10; // Move past "content":"
            if let Some(content_end) = json_text[content_start..].find(r#"","color""#) {
                let raw_content = &json_text[content_start..content_start + content_end];
                
                // Escape problematic characters in content
                let escaped_content = raw_content
                    .replace("\\", "\\\\")    // Escape backslashes first
                    .replace("\"", "\\\"")    // Escape quotes
                    .replace("\n", "\\n")     // Escape newlines
                    .replace("\r", "\\r")     // Escape carriage returns
                    .replace("\t", "\\t");    // Escape tabs
                
                // Extract color and thumbnail
                let color = color_regex.captures(json_text)
                    .and_then(|cap| cap.get(1))
                    .map(|m| m.as_str())
                    .unwrap_or("#00BFFF");
                
                let thumbnail = thumbnail_regex.captures(json_text)
                    .and_then(|cap| cap.get(1))
                    .map(|m| m.as_str())
                    .unwrap_or("pointing");
                
                // Reconstruct valid JSON
                let fixed_json = format!(
                    r#"{{"content":"{}","color":"{}","thumbnail":"{}"}}"#,
                    escaped_content, color, thumbnail
                );
                
                return Some(fixed_json);
            }
        }
        
        // Alternative approach: try to find content between quotes even if malformed
        if let Some(content_match) = content_regex.captures(json_text) {
            if let Some(color_match) = color_regex.captures(json_text) {
                if let Some(thumbnail_match) = thumbnail_regex.captures(json_text) {
                    let content = content_match.get(1)?.as_str();
                    let color = color_match.get(1)?.as_str();
                    let thumbnail = thumbnail_match.get(1)?.as_str();
                    
                    // Ensure content is properly escaped
                    let escaped_content = content
                        .replace("\\", "\\\\")
                        .replace("\"", "\\\"")
                        .replace("\n", "\\n")
                        .replace("\r", "\\r")
                        .replace("\t", "\\t");
                    
                    let fixed_json = format!(
                        r#"{{"content":"{}","color":"{}","thumbnail":"{}"}}"#,
                        escaped_content, color, thumbnail
                    );
                    
                    return Some(fixed_json);
                }
            }
        }
        
        None
    }

    /// Parse color from hex string or color name
    fn parse_color(&self, color: &str) -> (u8, u8, u8) {
        // If it starts with #, try to parse as hex
        if color.starts_with('#') {
            if let Ok(hex_val) = u32::from_str_radix(&color[1..], 16) {
                let r = ((hex_val >> 16) & 0xFF) as u8;
                let g = ((hex_val >> 8) & 0xFF) as u8;
                let b = (hex_val & 0xFF) as u8;
                return (r, g, b);
            }
        }
        
        // Fallback to predefined color names
        match color {
            "#695acd" | "base-purple" => (105, 90, 205),
            "#7b6fd3" | "light-purple" => (123, 111, 211),
            "#5048c7" | "deep-purple" => (80, 72, 199),
            "#4a90e2" | "cool-blue" => (74, 144, 226),
            "#6fa8dc" | "sky-blue" => (111, 168, 220),
            "#8e7cc3" | "soft-violet" => (142, 124, 195),
            "#2d3748" | "dark-slate" => (45, 55, 72),
            "#805ad5" | "bright-violet" => (128, 90, 213),
            "#4c51bf" | "indigo" => (76, 81, 191),
            "#667eea" | "periwinkle" => (102, 126, 234),
            _ => (105, 90, 205), // Default to base purple if unknown
        }
    }

    /// Get image URL from thumbnail name
    fn get_image_url(&self, thumbnail_name: &str, images: &crate::lang::ImageManager) -> String {
        // Try different categories to find the image
        let categories = ["avatar", "emotions", "reactions", "talking", "thinking", "showing", "misc"];
        
        for category in &categories {
            if let Some(url) = images.get_image(category, thumbnail_name) {
                return url.clone();
            }
        }
        
        // Fallback to default if not found
        images.get_random_avatar()
            .unwrap_or(&"https://cdn.discordapp.com/embed/avatars/0.png".to_string())
            .clone()
    }

    /// Get AI channel ID
    pub fn get_ai_channel_id(&self) -> &str {
        &self.config.ai_channel_id
    }

    /// Get owner Discord ID
    pub fn get_owner_id(&self) -> &str {
        &self.config.owner_info.discord_id
    }
}