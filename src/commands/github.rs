//! /github_channel command and GitHub activity feed
//!
//! Polls the public events of the configured GitHub account and announces
//! them as embeds in the owner-configured channel.

use std::sync::Arc;

use serenity::all::{
    ChannelId, Color, CommandDataOptionValue, CommandInteraction, Context, CreateEmbed,
    CreateEmbedAuthor, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateMessage, Timestamp,
};

use crate::config;
use crate::data::DataManager;

/// Handle the /github_channel command (owner only)
pub async fn handle_github_channel_command(
    ctx: &Context,
    command: &CommandInteraction,
    data_manager: &DataManager,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if command.user.id.get() != config::OWNER_ID {
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content("❌ Unauthorized. This command is owner-only.")
                .ephemeral(true),
        );
        command.create_response(&ctx.http, response).await?;
        return Ok(());
    }

    let channel_id = match command.data.options.first().map(|o| &o.value) {
        Some(CommandDataOptionValue::Channel(id)) => id.get(),
        _ => {
            let response = CreateInteractionResponse::Message(
                CreateInteractionResponseMessage::new()
                    .content("❌ Missing channel option.")
                    .ephemeral(true),
            );
            command.create_response(&ctx.http, response).await?;
            return Ok(());
        }
    };

    data_manager
        .set_github_channel(channel_id)
        .map_err(|e| e.to_string())?;

    let embed = CreateEmbed::new()
        .title("✅ GitHub Announcements Channel Set")
        .description(format!(
            "Public activity of [**{user}**](https://github.com/{user}) will be announced in <#{channel_id}>.",
            user = config::GITHUB_USER
        ))
        .color(Color::from_rgb(46, 164, 79))
        .thumbnail(format!("https://github.com/{}.png", config::GITHUB_USER));

    let response = CreateInteractionResponse::Message(
        CreateInteractionResponseMessage::new()
            .add_embed(embed)
            .ephemeral(true),
    );
    command.create_response(&ctx.http, response).await?;
    Ok(())
}

/// Poll GitHub public events and announce new ones in the configured channel.
///
/// `etag` is kept across calls so unchanged feeds return 304 (which does not
/// count against the unauthenticated rate limit).
pub async fn poll_github_events(
    http: &Arc<serenity::http::Http>,
    data_manager: &DataManager,
    client: &reqwest::Client,
    etag: &mut Option<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let feed = data_manager.get_github_feed();
    let Some(channel_id) = feed.channel_id else {
        return Ok(());
    };

    let mut request = client
        .get(format!(
            "https://api.github.com/users/{}/events/public?per_page=30",
            config::GITHUB_USER
        ))
        .header("User-Agent", "lorian-discord-bot")
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28");
    if let Some(tag) = etag.as_deref() {
        request = request.header("If-None-Match", tag);
    }

    let response = request.send().await?;
    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(());
    }
    if !response.status().is_success() {
        return Err(format!("GitHub API status {}", response.status()).into());
    }
    *etag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let events: serde_json::Value = response.json().await?;
    let events = events.as_array().cloned().unwrap_or_default();

    // Feed is newest-first
    let newest_id = events.first().and_then(event_id);

    let Some(last_seen) = feed.last_event_id else {
        // First run: baseline only, don't spam history
        if let Some(id) = newest_id {
            data_manager
                .set_github_last_event(id)
                .map_err(|e| e.to_string())?;
        }
        return Ok(());
    };

    let mut new_events: Vec<&serde_json::Value> = events
        .iter()
        .filter(|e| event_id(e).is_some_and(|id| id > last_seen))
        .collect();
    new_events.reverse(); // announce oldest first

    let channel = ChannelId::new(channel_id);
    // ponytail: cap 5 embeds per poll to avoid flooding after long downtime
    for event in new_events.iter().take(5) {
        if let Some(embed) = build_event_embed(event) {
            channel
                .send_message(http, CreateMessage::new().add_embed(embed))
                .await?;
        }
    }

    if let Some(id) = newest_id {
        if id > last_seen {
            data_manager
                .set_github_last_event(id)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

fn event_id(event: &serde_json::Value) -> Option<u64> {
    event["id"].as_str().and_then(|s| s.parse().ok())
}

/// Build a pretty embed for a GitHub event. Returns `None` for event types
/// not worth announcing.
fn build_event_embed(event: &serde_json::Value) -> Option<CreateEmbed> {
    let repo = event["repo"]["name"].as_str().unwrap_or("unknown/repo");
    let repo_url = format!("https://github.com/{}", repo);
    let payload = &event["payload"];

    let (title, url, description, color) = match event["type"].as_str()? {
        "PushEvent" => {
            let branch = payload["ref"]
                .as_str()
                .map(|r| r.trim_start_matches("refs/heads/"))
                .unwrap_or("?");
            let commits = payload["commits"].as_array().cloned().unwrap_or_default();
            let lines: Vec<String> = commits
                .iter()
                .rev()
                .take(5)
                .map(|c| {
                    let sha = c["sha"].as_str().unwrap_or("");
                    let short = &sha[..sha.len().min(7)];
                    let msg = c["message"]
                        .as_str()
                        .unwrap_or("")
                        .lines()
                        .next()
                        .unwrap_or("");
                    format!("[`{}`]({}/commit/{}) {}", short, repo_url, sha, msg)
                })
                .collect();
            (
                format!("📦 {} commit(s) pushed to {}@{}", commits.len(), repo, branch),
                format!("{}/commits/{}", repo_url, branch),
                lines.join("\n"),
                Color::from_rgb(46, 164, 79),
            )
        }
        "CreateEvent" => {
            let ref_type = payload["ref_type"].as_str().unwrap_or("?");
            let name = payload["ref"].as_str().unwrap_or(repo);
            (
                format!("🌱 New {} `{}` in {}", ref_type, name, repo),
                repo_url.clone(),
                String::new(),
                Color::from_rgb(88, 166, 255),
            )
        }
        "DeleteEvent" => {
            let ref_type = payload["ref_type"].as_str().unwrap_or("?");
            let name = payload["ref"].as_str().unwrap_or("?");
            (
                format!("🗑️ Deleted {} `{}` in {}", ref_type, name, repo),
                repo_url.clone(),
                String::new(),
                Color::from_rgb(139, 148, 158),
            )
        }
        "ReleaseEvent" => {
            let release = &payload["release"];
            let tag = release["tag_name"].as_str().unwrap_or("?");
            let body = release["body"].as_str().unwrap_or("");
            let mut desc: String = body.chars().take(300).collect();
            if body.chars().count() > 300 {
                desc.push('…');
            }
            (
                format!("🚀 Release {} published in {}", tag, repo),
                release["html_url"].as_str().unwrap_or(&repo_url).to_string(),
                desc,
                Color::from_rgb(163, 113, 247),
            )
        }
        "PullRequestEvent" => {
            let action = payload["action"].as_str().unwrap_or("?");
            let pr = &payload["pull_request"];
            let action = if action == "closed" && pr["merged"].as_bool() == Some(true) {
                "merged"
            } else {
                action
            };
            (
                format!(
                    "🔀 PR #{} {} in {}: {}",
                    pr["number"].as_u64().unwrap_or(0),
                    action,
                    repo,
                    pr["title"].as_str().unwrap_or("")
                ),
                pr["html_url"].as_str().unwrap_or(&repo_url).to_string(),
                String::new(),
                Color::from_rgb(240, 136, 62),
            )
        }
        "IssuesEvent" => {
            let issue = &payload["issue"];
            (
                format!(
                    "🐛 Issue #{} {} in {}: {}",
                    issue["number"].as_u64().unwrap_or(0),
                    payload["action"].as_str().unwrap_or("?"),
                    repo,
                    issue["title"].as_str().unwrap_or("")
                ),
                issue["html_url"].as_str().unwrap_or(&repo_url).to_string(),
                String::new(),
                Color::from_rgb(218, 54, 51),
            )
        }
        "ForkEvent" => (
            format!("🍴 Forked {}", repo),
            payload["forkee"]["html_url"]
                .as_str()
                .unwrap_or(&repo_url)
                .to_string(),
            String::new(),
            Color::from_rgb(139, 148, 158),
        ),
        "WatchEvent" => (
            format!("⭐ Starred {}", repo),
            repo_url.clone(),
            String::new(),
            Color::from_rgb(227, 179, 65),
        ),
        "PublicEvent" => (
            format!("🌐 {} is now public", repo),
            repo_url.clone(),
            String::new(),
            Color::from_rgb(46, 164, 79),
        ),
        _ => return None, // comments, wiki edits, etc. — too noisy
    };

    let mut embed = CreateEmbed::new()
        .author(
            CreateEmbedAuthor::new(config::GITHUB_USER)
                .icon_url(format!("https://github.com/{}.png", config::GITHUB_USER))
                .url(format!("https://github.com/{}", config::GITHUB_USER)),
        )
        .title(title)
        .url(url)
        .color(color)
        .footer(CreateEmbedFooter::new(repo.to_string()));

    if !description.is_empty() {
        embed = embed.description(description);
    }
    if let Some(ts) = event["created_at"]
        .as_str()
        .and_then(|t| Timestamp::parse(t).ok())
    {
        embed = embed.timestamp(ts);
    }
    Some(embed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn push_event(id: &str) -> serde_json::Value {
        json!({
            "id": id,
            "type": "PushEvent",
            "repo": {"name": "Solar2004/test-repo"},
            "created_at": "2026-07-15T10:00:00Z",
            "payload": {
                "ref": "refs/heads/main",
                "commits": [
                    {"sha": "abcdef1234567890", "message": "feat: something\n\nbody"}
                ]
            }
        })
    }

    #[test]
    fn push_event_builds_embed() {
        assert!(build_event_embed(&push_event("100")).is_some());
    }

    #[test]
    fn unknown_event_is_skipped() {
        let event = json!({"id": "1", "type": "GollumEvent", "repo": {"name": "a/b"}, "payload": {}});
        assert!(build_event_embed(&event).is_none());
    }

    #[test]
    fn event_id_parses_numeric_string() {
        assert_eq!(event_id(&push_event("42")), Some(42));
        assert_eq!(event_id(&json!({"id": 42})), None);
    }
}
