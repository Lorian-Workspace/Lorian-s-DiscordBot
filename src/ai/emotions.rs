use rand::{self, Rng};

/// Types of emotions that can be detected and displayed
#[derive(Debug, Clone, PartialEq)]
pub enum EmotionType {
    Happy,
    Excited,
    Helpful,
    Thoughtful,
    Curious,
    Friendly,
    Professional,
    Creative,
    Encouraging,
    Neutral,
}

impl EmotionType {
    /// Get emotion keywords for detection
    pub fn keywords(&self) -> Vec<&'static str> {
        match self {
            EmotionType::Happy => vec!["feliz", "genial", "excelente", "perfecto", "increíble", "😊", "🎉"],
            EmotionType::Excited => vec!["emocionante", "fantástico", "asombroso", "wow", "guau", "🚀", "⭐"],
            EmotionType::Helpful => vec!["ayuda", "puedo ayudar", "aquí tienes", "te explico", "💡", "🤝"],
            EmotionType::Thoughtful => vec!["considera", "piensa", "reflexiona", "analiza", "🤔", "💭"],
            EmotionType::Curious => vec!["interesante", "dime más", "cuéntame", "explícame", "❓", "🔍"],
            EmotionType::Friendly => vec!["hola", "saludos", "encantado", "un placer", "👋", "😄"],
            EmotionType::Professional => vec!["servicio", "trabajo", "proyecto", "empresa", "negocio", "💼", "👔"],
            EmotionType::Creative => vec!["diseño", "arte", "creatividad", "idea", "innovador", "🎨", "✨"],
            EmotionType::Encouraging => vec!["puedes", "lograrás", "adelante", "ánimo", "éxito", "💪", "🌟"],
            EmotionType::Neutral => vec![],
        }
    }

    /// Get image category for this emotion
    pub fn image_category(&self) -> &'static str {
        match self {
            EmotionType::Happy => "emotions",
            EmotionType::Excited => "emotions", 
            EmotionType::Helpful => "talking",
            EmotionType::Thoughtful => "talking",
            EmotionType::Curious => "talking",
            EmotionType::Friendly => "emotions",
            EmotionType::Professional => "avatar",
            EmotionType::Creative => "emotions",
            EmotionType::Encouraging => "emotions",
            EmotionType::Neutral => "avatar",
        }
    }

    /// Get specific image name for this emotion
    pub fn image_name(&self) -> &'static str {
        match self {
            EmotionType::Happy => "happy",
            EmotionType::Excited => "excited",
            EmotionType::Helpful => "explaining",
            EmotionType::Thoughtful => "thinking",
            EmotionType::Curious => "curious",
            EmotionType::Friendly => "friendly",
            EmotionType::Professional => "professional",
            EmotionType::Creative => "creative",
            EmotionType::Encouraging => "thumbs_up",
            EmotionType::Neutral => "pointing",
        }
    }

    /// Get emoji representation
    pub fn emoji(&self) -> &'static str {
        match self {
            EmotionType::Happy => "😊",
            EmotionType::Excited => "🎉",
            EmotionType::Helpful => "🤝",
            EmotionType::Thoughtful => "🤔",
            EmotionType::Curious => "🔍",
            EmotionType::Friendly => "👋",
            EmotionType::Professional => "💼",
            EmotionType::Creative => "🎨",
            EmotionType::Encouraging => "💪",
            EmotionType::Neutral => "🤖",
        }
    }

    /// Get color associated with this emotion (RGB)
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            EmotionType::Happy => (255, 215, 0),       // Gold
            EmotionType::Excited => (255, 69, 0),      // Red-Orange
            EmotionType::Helpful => (0, 191, 255),     // Deep Sky Blue
            EmotionType::Thoughtful => (138, 43, 226), // Blue Violet
            EmotionType::Curious => (255, 20, 147),    // Deep Pink
            EmotionType::Friendly => (50, 205, 50),    // Lime Green
            EmotionType::Professional => (25, 25, 112), // Midnight Blue
            EmotionType::Creative => (186, 85, 211),   // Medium Orchid
            EmotionType::Encouraging => (34, 139, 34), // Forest Green
            EmotionType::Neutral => (128, 128, 128),   // Gray
        }
    }
}

/// Manages emotion detection and image selection
pub struct EmotionManager {
    /// Available emotion types
    emotions: Vec<EmotionType>,
}

impl EmotionManager {
    pub fn new() -> Self {
        Self {
            emotions: vec![
                EmotionType::Happy,
                EmotionType::Excited,
                EmotionType::Helpful,
                EmotionType::Thoughtful,
                EmotionType::Curious,
                EmotionType::Friendly,
                EmotionType::Professional,
                EmotionType::Creative,
                EmotionType::Encouraging,
            ],
        }
    }

    /// Analyze text to determine the most likely emotion
    pub fn analyze_emotion(&self, text: &str) -> EmotionType {
        let text_lower = text.to_lowercase();
        let mut emotion_scores: Vec<(EmotionType, usize)> = Vec::new();

        // Score each emotion based on keyword matches
        for emotion in &self.emotions {
            let mut score = 0;
            for keyword in emotion.keywords() {
                if text_lower.contains(keyword) {
                    score += 1;
                }
            }
            if score > 0 {
                emotion_scores.push((emotion.clone(), score));
            }
        }

        // Sort by score and return the highest scoring emotion
        emotion_scores.sort_by(|a, b| b.1.cmp(&a.1));
        
        if let Some((emotion, _)) = emotion_scores.first() {
            emotion.clone()
        } else {
            // If no emotion detected, pick a random one from common ones or neutral
            let mut rng = rand::thread_rng();
            let common_emotions = vec![
                EmotionType::Helpful,
                EmotionType::Friendly,
                EmotionType::Professional,
                EmotionType::Neutral,
            ];
            common_emotions[rng.gen_range(0..common_emotions.len())].clone()
        }
    }

    /// Get a random emotion of a specific type
    pub fn get_random_emotion(&self) -> EmotionType {
        let mut rng = rand::thread_rng();
        self.emotions[rng.gen_range(0..self.emotions.len())].clone()
    }

    /// Check if an emotion should show encouragement buttons
    pub fn should_show_encouragement_buttons(&self, emotion: &EmotionType) -> bool {
        matches!(emotion, EmotionType::Helpful | EmotionType::Professional | EmotionType::Encouraging)
    }
}

impl Default for EmotionManager {
    fn default() -> Self {
        Self::new()
    }
}