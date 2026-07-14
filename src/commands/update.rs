//! /update command handler

use serenity::all::{
    CommandInteraction, Context, CreateInteractionResponse, CreateInteractionResponseMessage,
    EditInteractionResponse,
};

use crate::config;
use crate::updater;

/// Handle the /update command
pub async fn handle_update_command(
    ctx: &Context,
    command: &CommandInteraction,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Owner gate
    if command.user.id.get() != config::OWNER_ID {
        let response = CreateInteractionResponse::Message(
            CreateInteractionResponseMessage::new()
                .content("❌ Unauthorized. This command is owner-only.")
                .ephemeral(true),
        );
        command.create_response(&ctx.http, response).await?;
        return Ok(());
    }

    // Immediate ephemeral ACK
    let defer_response = CreateInteractionResponse::Defer(
        CreateInteractionResponseMessage::new().ephemeral(true),
    );
    command.create_response(&ctx.http, defer_response).await?;

    // Try to acquire lock (non-blocking)
    let _lock = match updater::try_acquire_lock() {
        Some(lock) => lock,
        None => {
            command
                .edit_response(&ctx.http, EditInteractionResponse::new().content("⏳ Update already in progress."))
                .await?;
            return Ok(());
        }
    };

    // Check for update
    let update = match updater::check_for_update().await {
        Ok(Some(update)) => update,
        Ok(None) => {
            command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().content(format!(
                        "✅ Already up to date (v{})",
                        updater::current_version()
                    )),
                )
                .await?;
            return Ok(());
        }
        Err(updater::UpdaterError::NoReleaseAvailable) => {
            command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().content("❌ No release available."),
                )
                .await?;
            return Ok(());
        }
        Err(e) => {
            command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().content(format!("❌ Error: {}", e)),
                )
                .await?;
            return Ok(());
        }
    };

    // Apply update
    // Edit message before apply_update since exec will replace the process
    command
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new().content(format!(
                "✅ Verified v{} — applying update and restarting...",
                update.version
            )),
        )
        .await?;

    match updater::apply_update(&update).await {
        Ok(()) => {
            // This line should never be reached if exec succeeds
            // But if it does, inform the user
            command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().content(format!(
                        "✅ Updated to v{} — restart pending",
                        update.version
                    )),
                )
                .await?;
        }
        Err(e) => {
            command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().content(format!("❌ Update failed: {}", e)),
                )
                .await?;
        }
    }

    Ok(())
}
