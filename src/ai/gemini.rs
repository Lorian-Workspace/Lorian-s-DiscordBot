use serde::{Deserialize, Serialize};
use std::error::Error;
use reqwest::Client;

/// Gemini API client
pub struct GeminiClient {
    client: Client,
    api_key: String,
    api_url: String,
}

/// Response from Gemini API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiResponse {
    pub content: String,
    pub finish_reason: Option<String>,
    pub safety_ratings: Option<Vec<SafetyRating>>,
}

/// Safety rating from Gemini
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyRating {
    pub category: String,
    pub probability: String,
}

/// Request structure for Gemini API
#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
    generation_config: GenerationConfig,
    safety_settings: Vec<SafetySetting>,
}

#[derive(Debug, Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
struct Part {
    text: String,
}

#[derive(Debug, Serialize)]
struct GenerationConfig {
    temperature: f32,
    top_k: u32,
    top_p: f32,
    max_output_tokens: u32,
}

#[derive(Debug, Serialize)]
struct SafetySetting {
    category: String,
    threshold: String,
}

/// Response structure from Gemini API
#[derive(Debug, Deserialize)]
struct GeminiApiResponse {
    candidates: Option<Vec<Candidate>>,
    prompt_feedback: Option<PromptFeedback>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: ContentResponse,
    finish_reason: Option<String>,
    safety_ratings: Option<Vec<SafetyRating>>,
}

#[derive(Debug, Deserialize)]
struct ContentResponse {
    parts: Vec<PartResponse>,
}

#[derive(Debug, Deserialize)]
struct PartResponse {
    text: String,
}

#[derive(Debug, Deserialize)]
struct PromptFeedback {
    safety_ratings: Option<Vec<SafetyRating>>,
}

impl GeminiClient {
    pub fn new(api_key: String) -> Result<Self, Box<dyn Error>> {
        if api_key.is_empty() {
            return Err("Gemini API key not provided".into());
        }

        Ok(Self {
            client: Client::new(),
            api_key,
            api_url: "https://generativelanguage.googleapis.com/v1beta/models/gemini-1.5-flash:generateContent".to_string(),
        })
    }

    pub async fn generate_response(&self, prompt: &str) -> Result<GeminiResponse, Box<dyn Error>> {
        let request = GeminiRequest {
            contents: vec![Content {
                parts: vec![Part {
                    text: prompt.to_string(),
                }],
            }],
            generation_config: GenerationConfig {
                temperature: 0.7,
                top_k: 40,
                top_p: 0.8,
                max_output_tokens: 1000,
            },
            safety_settings: vec![
                SafetySetting {
                    category: "HARM_CATEGORY_HARASSMENT".to_string(),
                    threshold: "BLOCK_MEDIUM_AND_ABOVE".to_string(),
                },
                SafetySetting {
                    category: "HARM_CATEGORY_HATE_SPEECH".to_string(),
                    threshold: "BLOCK_MEDIUM_AND_ABOVE".to_string(),
                },
                SafetySetting {
                    category: "HARM_CATEGORY_SEXUALLY_EXPLICIT".to_string(),
                    threshold: "BLOCK_MEDIUM_AND_ABOVE".to_string(),
                },
                SafetySetting {
                    category: "HARM_CATEGORY_DANGEROUS_CONTENT".to_string(),
                    threshold: "BLOCK_MEDIUM_AND_ABOVE".to_string(),
                },
            ],
        };

        let url = format!("{}?key={}", self.api_url, self.api_key);
        
        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Gemini API error: {}", error_text).into());
        }

        let api_response: GeminiApiResponse = response.json().await?;

        // Extract the response content
        if let Some(candidates) = api_response.candidates {
            if let Some(candidate) = candidates.first() {
                if let Some(part) = candidate.content.parts.first() {
                    return Ok(GeminiResponse {
                        content: part.text.clone(),
                        finish_reason: candidate.finish_reason.clone(),
                        safety_ratings: candidate.safety_ratings.clone(),
                    });
                }
            }
        }

        Err("No valid response from Gemini API".into())
    }

    /// Test connection to Gemini API
    pub async fn test_connection(&self) -> Result<bool, Box<dyn Error>> {
        match self.generate_response("Hello, respond with 'OK' if you can hear me.").await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}