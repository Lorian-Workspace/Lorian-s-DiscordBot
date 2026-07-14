use std::collections::{HashMap, HashSet};
use std::env;

use chrono::{DateTime, Duration, Utc};
use reqwest::Url;
use serenity::all::{
    ButtonStyle, Channel, ChannelId, ComponentInteraction, CreateActionRow, CreateAllowedMentions,
    CreateButton, CreateEmbed, CreateEmbedFooter, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateMessage, EditInteractionResponse, EditMessage,
    GetMessages, GuildChannel, GuildId, Member, Message, MessageId, MessageType,
    PermissionOverwriteType, Permissions, Role, RoleId, UserId,
};
use serenity::http::HttpError;
use serenity::prelude::Context;
use serenity::Error as SerenityError;
use tokio::sync::{Mutex, RwLock};

use crate::data::{
    AnnouncementDelivery, AnnouncementSubscription, DataManager, HoneypotRecovery, HoneypotStage,
    VerificationPending,
};
use crate::lang::{ImageManager, LanguageManager};

pub const ANNOUNCEMENT_CHANNEL_ID: u64 = 1_400_467_682_440_118_333;
pub const HONEYPOT_CHANNEL_ID: u64 = 1_526_610_057_511_567_380;
const SAFETY_VERSION: &str = "v1";
const ANNOUNCEMENT_TERMS_VERSION: &str = "optional-announcement-dm-v1";
const VERIFICATION_MARKER: &str = "safety-verification-v1";
const HONEYPOT_MARKER: &str = "safety-honeypot-v1";
const INCIDENT_RETENTION_DAYS: i64 = 30;
const HONEYPOT_DELETE_DAYS: u8 = 2;

type SafetyResult<T> = Result<T, String>;

#[derive(Clone, Debug)]
pub struct SafetyConfig {
    verification_channel_id: ChannelId,
    unverified_role_id: RoleId,
    verified_role_id: RoleId,
    subscriber_role_id: RoleId,
    owner_id: UserId,
    server_invite_url: String,
}

impl SafetyConfig {
    pub fn from_env() -> SafetyResult<Self> {
        let verification_channel_id = ChannelId::new(env_id("VERIFICATION_CHANNEL_ID")?);
        let unverified_role_id = RoleId::new(env_id("UNVERIFIED_ROLE_ID")?);
        let verified_role_id = RoleId::new(env_id("VERIFIED_ROLE_ID")?);
        let subscriber_role_id = RoleId::new(env_id("SUBSCRIBER_ROLE_ID")?);
        let owner_id = UserId::new(env_id("DISCORD_OWNER_ID")?);
        let server_invite_url = env::var("SERVER_INVITE_URL")
            .map_err(|_| "SERVER_INVITE_URL is required".to_string())?;

        validate_invite_url(&server_invite_url)?;

        if unverified_role_id == verified_role_id
            || unverified_role_id == subscriber_role_id
            || verified_role_id == subscriber_role_id
        {
            return Err(
                "UNVERIFIED_ROLE_ID, VERIFIED_ROLE_ID, and SUBSCRIBER_ROLE_ID must be distinct"
                    .to_string(),
            );
        }

        Ok(Self {
            verification_channel_id,
            unverified_role_id,
            verified_role_id,
            subscriber_role_id,
            owner_id,
            server_invite_url,
        })
    }
}

fn env_id(name: &str) -> SafetyResult<u64> {
    env::var(name)
        .map_err(|_| format!("{name} is required"))?
        .parse::<u64>()
        .map_err(|_| format!("{name} must be a Discord snowflake"))
        .and_then(|id| {
            if id == 0 {
                Err(format!("{name} must be non-zero"))
            } else {
                Ok(id)
            }
        })
}

fn validate_invite_url(raw: &str) -> SafetyResult<()> {
    let url = Url::parse(raw).map_err(|error| format!("SERVER_INVITE_URL is invalid: {error}"))?;
    let valid_host = matches!(
        url.host_str(),
        Some("discord.gg" | "discord.com" | "www.discord.com")
    );
    let has_invite_path = if url.host_str() == Some("discord.gg") {
        url.path_segments()
            .and_then(|mut segments| segments.next())
            .is_some_and(|segment| !segment.is_empty())
    } else {
        let mut segments = url.path_segments().into_iter().flatten();
        segments.next() == Some("invite")
            && segments.next().is_some_and(|segment| !segment.is_empty())
    };

    if url.scheme() != "https" || !valid_host || !has_invite_path {
        return Err(
            "SERVER_INVITE_URL must be a stable https://discord.gg/<code> or https://discord.com/invite/<code> URL"
                .to_string(),
        );
    }

    Ok(())
}

pub struct SafetyService {
    config: SafetyConfig,
    enabled_guild: RwLock<Option<GuildId>>,
    ready_lock: Mutex<()>,
    announcement_lock: Mutex<()>,
    honeypot_lock: Mutex<()>,
}

struct CanonicalContent<'a> {
    marker: &'a str,
    embed: CreateEmbed,
    components: Vec<CreateActionRow>,
}

impl SafetyService {
    pub fn new() -> SafetyResult<Self> {
        Ok(Self {
            config: SafetyConfig::from_env()?,
            enabled_guild: RwLock::new(None),
            ready_lock: Mutex::new(()),
            announcement_lock: Mutex::new(()),
            honeypot_lock: Mutex::new(()),
        })
    }

    pub async fn ready(
        &self,
        ctx: &Context,
        bot_id: UserId,
        data: &DataManager,
        lang: &LanguageManager,
        images: &ImageManager,
    ) {
        let _guard = self.ready_lock.lock().await;
        let validated = self.validate_runtime(ctx, bot_id).await;
        let (guild_id, verification_channel) = match validated {
            Ok(value) => value,
            Err(error) => {
                *self.enabled_guild.write().await = None;
                eprintln!("SAFETY DISABLED (fail closed): {error}");
                return;
            }
        };

        *self.enabled_guild.write().await = None;
        let cutoff_was_present = data
            .get_data()
            .safety
            .verification_started_at
            .contains_key(&guild_key(guild_id));

        if !cutoff_was_present {
            let cutoff_result = data
                .update_data(|bot_data| {
                    bot_data
                        .safety
                        .verification_started_at
                        .insert(guild_key(guild_id), Utc::now());
                })
                .map_err(display_error);
            if let Err(error) = cutoff_result {
                *self.enabled_guild.write().await = None;
                eprintln!("SAFETY DISABLED (cutoff persistence failed): {error}");
                return;
            }
        }

        if let Err(error) = self
            .reconcile_verification_panel(
                ctx,
                bot_id,
                guild_id,
                verification_channel.id,
                data,
                lang,
            )
            .await
        {
            eprintln!("SAFETY DISABLED (verification panel reconcile failed): {error}");
            return;
        }

        *self.enabled_guild.write().await = Some(guild_id);
        {
            let _honeypot_guard = self.honeypot_lock.lock().await;
            if let Err(error) = self
                .reconcile_honeypot_panel(ctx, bot_id, guild_id, data, lang, images)
                .await
            {
                eprintln!("SAFETY honeypot panel reconcile failed: {error}");
            }
            if let Err(error) = self.recover_honeypot_unbans(ctx, guild_id, data).await {
                eprintln!("CRITICAL SAFETY honeypot unban recovery failed: {error}");
            }
        }

        if cutoff_was_present {
            if let Err(error) = self.reconcile_members(ctx, guild_id, data).await {
                eprintln!("SAFETY member recovery failed: {error}");
            }
        }
        if let Err(error) = self.resume_announcements(ctx, guild_id, data, lang).await {
            eprintln!("SAFETY announcement recovery paused: {error}");
        }
        self.prune_completed_incidents(data);
    }

    async fn validate_runtime(
        &self,
        ctx: &Context,
        bot_id: UserId,
    ) -> SafetyResult<(GuildId, GuildChannel)> {
        let verification_channel = guild_channel(ctx, self.config.verification_channel_id).await?;
        let guild_id = verification_channel.guild_id;
        let announcement_channel =
            guild_channel(ctx, ChannelId::new(ANNOUNCEMENT_CHANNEL_ID)).await?;
        let honeypot_channel = guild_channel(ctx, ChannelId::new(HONEYPOT_CHANNEL_ID)).await?;
        if announcement_channel.guild_id != guild_id || honeypot_channel.guild_id != guild_id {
            return Err("all safety channels must belong to the verification guild".to_string());
        }

        let roles = guild_id.roles(&ctx.http).await.map_err(display_error)?;
        let unverified =
            required_role(&roles, self.config.unverified_role_id, "UNVERIFIED_ROLE_ID")?;
        let verified = required_role(&roles, self.config.verified_role_id, "VERIFIED_ROLE_ID")?;
        let subscriber =
            required_role(&roles, self.config.subscriber_role_id, "SUBSCRIBER_ROLE_ID")?;
        if unverified.managed || verified.managed || subscriber.managed {
            return Err("configured safety roles must not be integration-managed".to_string());
        }
        if unverified.permissions.administrator()
            || verified.permissions.administrator()
            || subscriber.permissions.administrator()
        {
            return Err("configured safety roles must not grant Administrator".to_string());
        }

        let bot_member = guild_id
            .member(&ctx.http, bot_id)
            .await
            .map_err(display_error)?;
        let highest_bot_position = bot_member
            .roles
            .iter()
            .filter_map(|role_id| roles.get(role_id))
            .map(|role| role.position)
            .max()
            .unwrap_or_default();
        let highest_managed_position =
            [unverified.position, verified.position, subscriber.position]
                .into_iter()
                .max()
                .unwrap_or_default();
        if highest_bot_position <= highest_managed_position {
            return Err("bot highest role must be above all configured safety roles".to_string());
        }

        let guild_permissions = aggregate_permissions(guild_id, &bot_member, &roles);
        let required_guild_permissions =
            Permissions::MANAGE_ROLES | Permissions::BAN_MEMBERS | Permissions::MANAGE_MESSAGES;
        if !guild_permissions.contains(required_guild_permissions) {
            return Err(format!(
                "bot lacks required guild permissions: {:?}",
                required_guild_permissions - guild_permissions
            ));
        }

        let guild = ctx
            .cache
            .guild(guild_id)
            .ok_or_else(|| "verification guild is unavailable in cache".to_string())?;
        let panel_permissions = guild.user_permissions_in(&verification_channel, &bot_member);
        let announcement_permissions =
            guild.user_permissions_in(&announcement_channel, &bot_member);
        let honeypot_permissions = guild.user_permissions_in(&honeypot_channel, &bot_member);
        let panel_required = Permissions::VIEW_CHANNEL
            | Permissions::SEND_MESSAGES
            | Permissions::EMBED_LINKS
            | Permissions::READ_MESSAGE_HISTORY
            | Permissions::MANAGE_MESSAGES;
        let source_required = Permissions::VIEW_CHANNEL | Permissions::READ_MESSAGE_HISTORY;
        let honeypot_required = panel_required | Permissions::BAN_MEMBERS;
        if !panel_permissions.contains(panel_required)
            || !announcement_permissions.contains(source_required)
            || !honeypot_permissions.contains(honeypot_required)
        {
            return Err("safety channel permission overwrites deny required access".to_string());
        }
        let everyone = roles
            .get(&guild_id.everyone_role())
            .ok_or_else(|| "guild @everyone role is missing".to_string())?;
        let unverified_panel_permissions =
            effective_role_permissions(&verification_channel, guild_id, everyone, unverified);
        if !unverified_panel_permissions
            .contains(Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES)
        {
            return Err(
                "Unverified role cannot view and send in VERIFICATION_CHANNEL_ID; configure channel overwrites manually"
                    .to_string(),
            );
        }

        Ok((guild_id, verification_channel))
    }

    pub async fn member_added(&self, ctx: &Context, member: &Member, data: &DataManager) {
        if member.user.bot || !self.is_enabled_guild(member.guild_id).await {
            return;
        }
        let cutoff = data
            .get_data()
            .safety
            .verification_started_at
            .get(&guild_key(member.guild_id))
            .copied();
        let Some(cutoff) = cutoff else {
            eprintln!("SAFETY member add ignored fail-closed: cutoff missing");
            return;
        };

        let joined_at = member
            .joined_at
            .map(|timestamp| timestamp.unix_timestamp())
            .unwrap_or_else(|| Utc::now().timestamp());
        if joined_at < cutoff.timestamp() || member.roles.contains(&self.config.verified_role_id) {
            return;
        }

        if member.roles.contains(&self.config.unverified_role_id) {
            self.persist_pending(member.guild_id, member.user.id, None, data);
            return;
        }

        let result = member
            .add_role(&ctx.http, self.config.unverified_role_id)
            .await
            .map_err(display_error);
        self.persist_pending(member.guild_id, member.user.id, result.as_ref().err(), data);
        if let Err(error) = result {
            eprintln!("SAFETY failed to assign Unverified role: {error}");
        }
    }

    pub async fn handle_component(
        &self,
        ctx: &Context,
        component: &ComponentInteraction,
        data: &DataManager,
        lang: &LanguageManager,
    ) -> bool {
        if !component.data.custom_id.starts_with("safety:") {
            return false;
        }

        let deferred = CreateInteractionResponse::Defer(
            CreateInteractionResponseMessage::new().ephemeral(true),
        );
        if let Err(error) = component.create_response(&ctx.http, deferred).await {
            eprintln!("SAFETY interaction ACK failed: {error}");
            return true;
        }

        let parsed = SafetyCustomId::parse(&component.data.custom_id);
        let guild_id = component.guild_id;
        let valid = parsed.as_ref().is_some_and(|custom_id| {
            custom_id_matches_guild_and_version(custom_id, guild_id)
                && component.member.as_ref().is_some_and(|member| {
                    member.user.id == component.user.id && member.guild_id == custom_id.guild_id
                })
        });
        if !valid
            || guild_id.is_none()
            || !self
                .is_enabled_guild(guild_id.unwrap_or_else(|| GuildId::new(1)))
                .await
        {
            self.edit_interaction(
                component,
                ctx,
                &lang.get().safety.responses.invalid_interaction,
            )
            .await;
            return true;
        }

        let custom_id = match parsed {
            Some(value) => value,
            None => return true,
        };
        let member = match custom_id
            .guild_id
            .member(&ctx.http, component.user.id)
            .await
        {
            Ok(member) if !member.user.bot => member,
            _ => {
                self.edit_interaction(
                    component,
                    ctx,
                    &lang.get().safety.responses.invalid_interaction,
                )
                .await;
                return true;
            }
        };

        let response = match custom_id.action {
            SafetyAction::Verify => self.verify_member(ctx, &member, data, lang).await,
            SafetyAction::NotNow => lang.get().safety.responses.not_now.clone(),
            SafetyAction::Subscribe => self.subscribe(ctx, &member, data, lang).await,
            SafetyAction::Unsubscribe => self.unsubscribe(ctx, &member, data, lang).await,
        };
        self.edit_interaction(component, ctx, &response).await;
        true
    }

    async fn verify_member(
        &self,
        ctx: &Context,
        member: &Member,
        data: &DataManager,
        lang: &LanguageManager,
    ) -> String {
        let already_verified = member.roles.contains(&self.config.verified_role_id);
        if !already_verified {
            if let Err(error) = member
                .add_role(&ctx.http, self.config.verified_role_id)
                .await
            {
                eprintln!("SAFETY add Verified role failed: {error}");
                return lang.get().safety.responses.role_update_failed.clone();
            }
        }

        if member.roles.contains(&self.config.unverified_role_id) {
            if let Err(error) = member
                .remove_role(&ctx.http, self.config.unverified_role_id)
                .await
            {
                eprintln!("SAFETY remove Unverified role failed: {error}");
                if !already_verified {
                    if let Err(rollback_error) = member
                        .remove_role(&ctx.http, self.config.verified_role_id)
                        .await
                    {
                        eprintln!(
                            "CRITICAL SAFETY Verified-role rollback failed: {rollback_error}"
                        );
                    }
                }
                return lang.get().safety.responses.role_update_failed.clone();
            }
        }

        let key = member_key(member.guild_id, member.user.id);
        if let Err(error) = data.update_data(|bot_data| {
            bot_data.safety.verification_pending.remove(&key);
        }) {
            eprintln!("SAFETY verified pending cleanup failed: {error}");
        }
        if already_verified {
            lang.get().safety.responses.already_verified.clone()
        } else {
            lang.get().safety.responses.verified.clone()
        }
    }

    async fn subscribe(
        &self,
        ctx: &Context,
        member: &Member,
        data: &DataManager,
        lang: &LanguageManager,
    ) -> String {
        let already_subscriber = member.roles.contains(&self.config.subscriber_role_id);
        if !already_subscriber {
            if let Err(error) = member
                .add_role(&ctx.http, self.config.subscriber_role_id)
                .await
            {
                eprintln!("SAFETY add Subscriber role failed: {error}");
                return lang.get().safety.responses.subscription_failed.clone();
            }
        }

        let subscription = AnnouncementSubscription {
            guild_id: member.guild_id.get(),
            user_id: member.user.id.get(),
            active: true,
            updated_at: Utc::now(),
            terms_version: ANNOUNCEMENT_TERMS_VERSION.to_string(),
        };
        let key = member_key(member.guild_id, member.user.id);
        let persist_result = data
            .update_data(|bot_data| {
                bot_data
                    .safety
                    .announcement_subscriptions
                    .insert(key.clone(), subscription);
            })
            .map_err(display_error);
        if let Err(error) = persist_result {
            eprintln!("SAFETY persist subscription failed: {error}");
            let _ = data.update_data(|bot_data| {
                if let Some(record) = bot_data.safety.announcement_subscriptions.get_mut(&key) {
                    record.active = false;
                    record.updated_at = Utc::now();
                }
            });
            if !already_subscriber {
                if let Err(rollback_error) = member
                    .remove_role(&ctx.http, self.config.subscriber_role_id)
                    .await
                {
                    eprintln!("CRITICAL SAFETY Subscriber-role rollback failed: {rollback_error}");
                }
            }
            return lang.get().safety.responses.subscription_failed.clone();
        }

        lang.get().safety.responses.subscribed.clone()
    }

    async fn unsubscribe(
        &self,
        ctx: &Context,
        member: &Member,
        data: &DataManager,
        lang: &LanguageManager,
    ) -> String {
        let key = member_key(member.guild_id, member.user.id);
        let persist_result = data
            .update_data(|bot_data| {
                let record = bot_data
                    .safety
                    .announcement_subscriptions
                    .entry(key)
                    .or_insert_with(|| AnnouncementSubscription {
                        guild_id: member.guild_id.get(),
                        user_id: member.user.id.get(),
                        active: false,
                        updated_at: Utc::now(),
                        terms_version: ANNOUNCEMENT_TERMS_VERSION.to_string(),
                    });
                record.active = false;
                record.updated_at = Utc::now();
            })
            .map_err(display_error);
        if let Err(error) = &persist_result {
            eprintln!("CRITICAL SAFETY persist unsubscribe suppression failed: {error}");
            return lang.get().safety.responses.subscription_failed.clone();
        }
        if let Err(error) = member
            .remove_role(&ctx.http, self.config.subscriber_role_id)
            .await
        {
            eprintln!("SAFETY remove Subscriber role failed after ledger suppression: {error}");
        }

        lang.get().safety.responses.unsubscribed.clone()
    }

    async fn edit_interaction(
        &self,
        component: &ComponentInteraction,
        ctx: &Context,
        content: &str,
    ) {
        if let Err(error) = component
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new()
                    .content(content)
                    .allowed_mentions(CreateAllowedMentions::new()),
            )
            .await
        {
            eprintln!("SAFETY interaction response edit failed: {error}");
        }
    }

    pub async fn handle_message(
        &self,
        ctx: &Context,
        message: &Message,
        data: &DataManager,
        lang: &LanguageManager,
        images: &ImageManager,
    ) -> bool {
        match classify_channel(message.channel_id.get()) {
            SafetyChannel::Other => false,
            SafetyChannel::Announcement => {
                if self.message_matches_enabled_guild(message).await
                    && is_authorized_announcement(message, self.config.owner_id)
                {
                    let _guard = self.announcement_lock.lock().await;
                    if let Err(error) = self.process_announcement(ctx, message, data, lang).await {
                        eprintln!("SAFETY announcement delivery paused: {error}");
                    }
                }
                true
            }
            SafetyChannel::Honeypot => {
                if !self.message_matches_enabled_guild(message).await {
                    return true;
                }
                let _guard = self.honeypot_lock.lock().await;
                let bot_id = ctx.cache.current_user().id;
                let current_id = data
                    .get_data()
                    .safety
                    .honeypot_message_ids
                    .get(&guild_key(
                        message.guild_id.unwrap_or_else(|| GuildId::new(1)),
                    ))
                    .copied();
                if current_id == Some(message.id.get())
                    && is_canonical_message(message, bot_id, HONEYPOT_MARKER)
                {
                    return true;
                }
                if is_human_message(message) {
                    if let Err(error) = self.process_honeypot(ctx, message, data, lang).await {
                        eprintln!("SAFETY honeypot stage failed: {error}");
                    }
                }
                if let Some(guild_id) = message.guild_id {
                    if let Err(error) = self
                        .reconcile_honeypot_panel(ctx, bot_id, guild_id, data, lang, images)
                        .await
                    {
                        eprintln!("SAFETY honeypot cleanup failed: {error}");
                    }
                }
                true
            }
        }
    }

    async fn process_announcement(
        &self,
        ctx: &Context,
        message: &Message,
        data: &DataManager,
        lang: &LanguageManager,
    ) -> SafetyResult<()> {
        let guild_id = message
            .guild_id
            .ok_or_else(|| "announcement source has no guild".to_string())?;
        let delivery_key = message.id.to_string();
        let existing = data
            .get_data()
            .safety
            .announcement_deliveries
            .get(&delivery_key)
            .cloned();
        if existing
            .as_ref()
            .is_some_and(|job| job.completed_at.is_some())
        {
            return Ok(());
        }

        if existing.is_none() {
            let cutoff = data
                .get_data()
                .safety
                .verification_started_at
                .get(&guild_key(guild_id))
                .copied()
                .ok_or_else(|| "verification cutoff missing".to_string())?;
            let subscriptions = data.get_data().safety.announcement_subscriptions;
            let mut recipients = Vec::new();
            for subscription in subscriptions.values().filter(|record| {
                record.guild_id == guild_id.get()
                    && record.active
                    && record.terms_version == ANNOUNCEMENT_TERMS_VERSION
                    && record.user_id != message.author.id.get()
            }) {
                let member = match guild_id.member(&ctx.http, subscription.user_id).await {
                    Ok(member) => member,
                    Err(error) => {
                        eprintln!(
                            "SAFETY subscriber member lookup skipped {}: {}",
                            subscription.user_id, error
                        );
                        continue;
                    }
                };
                let joined_before_cutoff = member
                    .joined_at
                    .is_some_and(|joined| joined.unix_timestamp() < cutoff.timestamp());
                if announcement_recipient_eligible(
                    subscription.active,
                    member.user.bot,
                    member.roles.contains(&self.config.subscriber_role_id),
                    member.roles.contains(&self.config.verified_role_id),
                    joined_before_cutoff,
                    member.user.id == message.author.id,
                ) {
                    recipients.push(member.user.id.get());
                }
            }
            recipients.sort_unstable();
            recipients.dedup();
            let job = AnnouncementDelivery {
                guild_id: guild_id.get(),
                source_message_id: message.id.get(),
                recipient_user_ids: recipients,
                delivered_user_ids: Vec::new(),
                permanent_failure_user_ids: Vec::new(),
                skipped_user_ids: Vec::new(),
                started_at: Utc::now(),
                completed_at: None,
            };
            data.update_data(|bot_data| {
                bot_data
                    .safety
                    .announcement_deliveries
                    .insert(delivery_key.clone(), job);
            })
            .map_err(display_error)?;
        }

        self.deliver_announcement_job(ctx, message, data, lang)
            .await
    }

    async fn deliver_announcement_job(
        &self,
        ctx: &Context,
        source: &Message,
        data: &DataManager,
        lang: &LanguageManager,
    ) -> SafetyResult<()> {
        let key = source.id.to_string();
        let job = data
            .get_data()
            .safety
            .announcement_deliveries
            .get(&key)
            .cloned()
            .ok_or_else(|| "announcement delivery job missing".to_string())?;
        let embed = safe_announcement_embed(source, lang);

        for user_id in pending_delivery_recipients(&job) {
            let user = UserId::new(user_id);
            match self
                .announcement_recipient_currently_eligible(
                    ctx,
                    GuildId::new(job.guild_id),
                    user,
                    source.author.id,
                    data,
                )
                .await
            {
                Ok(true) => {}
                Ok(false) => {
                    self.mark_announcement_skipped(data, &key, user_id)?;
                    continue;
                }
                Err(error) if is_not_found_error(&error) => {
                    self.mark_announcement_skipped(data, &key, user_id)?;
                    continue;
                }
                Err(error) => {
                    return Err(format!(
                        "recipient revalidation failed for {user_id}: {error}"
                    ));
                }
            }
            let builder = CreateMessage::new()
                .embed(embed.clone())
                .allowed_mentions(CreateAllowedMentions::new());
            match user.direct_message(&ctx.http, builder).await {
                Ok(_) => {
                    data.update_data(|bot_data| {
                        if let Some(delivery) =
                            bot_data.safety.announcement_deliveries.get_mut(&key)
                        {
                            delivery.delivered_user_ids.push(user_id);
                        }
                    })
                    .map_err(display_error)?;
                }
                Err(error) if is_permanent_dm_error(&error) => {
                    eprintln!("SAFETY permanent DM failure for {user_id}: {error}");
                    data.update_data(|bot_data| {
                        if let Some(delivery) =
                            bot_data.safety.announcement_deliveries.get_mut(&key)
                        {
                            delivery.permanent_failure_user_ids.push(user_id);
                        }
                    })
                    .map_err(display_error)?;
                }
                Err(error) => return Err(format!("transient DM failure for {user_id}: {error}")),
            }
        }

        data.update_data(|bot_data| {
            if let Some(delivery) = bot_data.safety.announcement_deliveries.get_mut(&key) {
                delivery.completed_at = Some(Utc::now());
            }
        })
        .map_err(display_error)
    }

    async fn announcement_recipient_currently_eligible(
        &self,
        ctx: &Context,
        guild_id: GuildId,
        user_id: UserId,
        author_id: UserId,
        data: &DataManager,
    ) -> Result<bool, SerenityError> {
        let snapshot = data.get_data();
        let active = snapshot
            .safety
            .announcement_subscriptions
            .get(&member_key(guild_id, user_id))
            .is_some_and(|record| {
                record.active && record.terms_version == ANNOUNCEMENT_TERMS_VERSION
            });
        let cutoff = snapshot
            .safety
            .verification_started_at
            .get(&guild_key(guild_id))
            .copied();
        let Some(cutoff) = cutoff else {
            return Ok(false);
        };
        let member = guild_id.member(&ctx.http, user_id).await?;
        let joined_before_cutoff = member
            .joined_at
            .is_some_and(|joined| joined.unix_timestamp() < cutoff.timestamp());
        Ok(announcement_recipient_eligible(
            active,
            member.user.bot,
            member.roles.contains(&self.config.subscriber_role_id),
            member.roles.contains(&self.config.verified_role_id),
            joined_before_cutoff,
            member.user.id == author_id,
        ))
    }

    fn mark_announcement_skipped(
        &self,
        data: &DataManager,
        key: &str,
        user_id: u64,
    ) -> SafetyResult<()> {
        data.update_data(|bot_data| {
            if let Some(delivery) = bot_data.safety.announcement_deliveries.get_mut(key) {
                delivery.skipped_user_ids.push(user_id);
            }
        })
        .map_err(display_error)
    }

    async fn process_honeypot(
        &self,
        ctx: &Context,
        message: &Message,
        data: &DataManager,
        lang: &LanguageManager,
    ) -> SafetyResult<()> {
        let guild_id = message
            .guild_id
            .ok_or_else(|| "honeypot source has no guild".to_string())?;
        let key = message.id.to_string();
        let existing = data
            .get_data()
            .safety
            .honeypot_recoveries
            .get(&key)
            .cloned();
        let initial_stage = existing
            .as_ref()
            .map_or(HoneypotStage::Received, |incident| incident.stage);
        if matches!(
            initial_stage,
            HoneypotStage::Completed | HoneypotStage::BanFailed
        ) {
            return Ok(());
        }
        if existing.is_none() {
            let incident = HoneypotRecovery {
                guild_id: guild_id.get(),
                user_id: message.author.id.get(),
                source_message_id: message.id.get(),
                started_at: Utc::now(),
                stage: HoneypotStage::Received,
                completed_at: None,
            };
            data.update_data(|bot_data| {
                bot_data
                    .safety
                    .honeypot_recoveries
                    .insert(key.clone(), incident);
            })
            .map_err(display_error)?;
        }

        if matches!(
            initial_stage,
            HoneypotStage::BanPending | HoneypotStage::UnbanPending
        ) {
            return self.recover_honeypot_unbans(ctx, guild_id, data).await;
        }

        if initial_stage == HoneypotStage::Received {
            self.update_honeypot_stage(data, &key, honeypot_transition(initial_stage, true), None)?;
            let security_notice = lang
                .get()
                .safety
                .honeypot
                .security_dm
                .replace("{invite}", &self.config.server_invite_url);
            let notice = CreateMessage::new()
                .content(security_notice)
                .allowed_mentions(CreateAllowedMentions::new());
            if let Err(error) = message.author.direct_message(&ctx.http, notice).await {
                eprintln!("SAFETY honeypot security DM failed: {error}");
            }
        }
        self.update_honeypot_stage(
            data,
            &key,
            honeypot_transition(HoneypotStage::NoticeAttempted, true),
            None,
        )?;

        if let Err(error) = ctx
            .http
            .ban_user(
                guild_id,
                message.author.id,
                HONEYPOT_DELETE_DAYS,
                Some("Security barrier: disclosed temporary moderation action"),
            )
            .await
        {
            eprintln!("SAFETY honeypot ban failed; unban skipped: {error}");
            self.update_honeypot_stage(
                data,
                &key,
                honeypot_transition(HoneypotStage::BanPending, false),
                Some(Utc::now()),
            )?;
            return Ok(());
        }

        if let Err(error) = self.update_honeypot_stage(
            data,
            &key,
            honeypot_transition(HoneypotStage::BanPending, true),
            None,
        ) {
            eprintln!(
                "CRITICAL SAFETY could not persist UnbanPending after successful ban: {error}"
            );
        }
        match ctx
            .http
            .remove_ban(
                guild_id,
                message.author.id,
                Some("Security barrier: immediate access restoration"),
            )
            .await
        {
            Ok(()) => self.update_honeypot_stage(
                data,
                &key,
                honeypot_transition(HoneypotStage::UnbanPending, true),
                Some(Utc::now()),
            ),
            Err(error) => {
                eprintln!("CRITICAL SAFETY honeypot unban failed: {error}");
                Err(format!("unban pending for {}", message.author.id))
            }
        }
    }

    fn update_honeypot_stage(
        &self,
        data: &DataManager,
        key: &str,
        stage: HoneypotStage,
        completed_at: Option<DateTime<Utc>>,
    ) -> SafetyResult<()> {
        data.update_data(|bot_data| {
            if let Some(incident) = bot_data.safety.honeypot_recoveries.get_mut(key) {
                incident.stage = stage;
                incident.completed_at = completed_at;
            }
        })
        .map_err(display_error)
    }

    async fn recover_honeypot_unbans(
        &self,
        ctx: &Context,
        guild_id: GuildId,
        data: &DataManager,
    ) -> SafetyResult<()> {
        let incidents: Vec<(String, HoneypotRecovery)> = data
            .get_data()
            .safety
            .honeypot_recoveries
            .into_iter()
            .filter(|(_, incident)| {
                incident.guild_id == guild_id.get()
                    && matches!(
                        incident.stage,
                        HoneypotStage::BanPending | HoneypotStage::UnbanPending
                    )
            })
            .collect();
        let mut recovery_errors = Vec::new();
        for (key, incident) in incidents {
            let user_id = UserId::new(incident.user_id);
            let ban = match guild_id.get_ban(&ctx.http, user_id).await {
                Ok(ban) => ban,
                Err(error) => {
                    recovery_errors.push(format!("get_ban failed for {user_id}: {error}"));
                    continue;
                }
            };
            if ban.is_some() {
                if let Err(error) = ctx
                    .http
                    .remove_ban(
                        guild_id,
                        user_id,
                        Some("Security barrier recovery: restore access"),
                    )
                    .await
                {
                    recovery_errors.push(format!(
                        "CRITICAL unban recovery failed for {user_id}: {error}"
                    ));
                    continue;
                }
            }
            if let Err(error) =
                self.update_honeypot_stage(data, &key, HoneypotStage::Completed, Some(Utc::now()))
            {
                recovery_errors.push(error);
            }
        }
        if recovery_errors.is_empty() {
            Ok(())
        } else {
            Err(recovery_errors.join("; "))
        }
    }

    async fn resume_announcements(
        &self,
        ctx: &Context,
        guild_id: GuildId,
        data: &DataManager,
        lang: &LanguageManager,
    ) -> SafetyResult<()> {
        let source_ids: Vec<u64> = data
            .get_data()
            .safety
            .announcement_deliveries
            .values()
            .filter(|job| job.guild_id == guild_id.get() && job.completed_at.is_none())
            .map(|job| job.source_message_id)
            .collect();
        let _guard = self.announcement_lock.lock().await;
        let mut recovery_errors = Vec::new();
        for source_id in source_ids {
            let source_result = ChannelId::new(ANNOUNCEMENT_CHANNEL_ID)
                .message(&ctx.http, MessageId::new(source_id))
                .await;
            let source = match source_result {
                Ok(source) => source,
                Err(error) if is_not_found_error(&error) => {
                    let key = source_id.to_string();
                    if let Err(persist_error) = data.update_data(|bot_data| {
                        if let Some(delivery) =
                            bot_data.safety.announcement_deliveries.get_mut(&key)
                        {
                            delivery.completed_at = Some(Utc::now());
                        }
                    }) {
                        recovery_errors.push(persist_error.to_string());
                    }
                    continue;
                }
                Err(error) => {
                    recovery_errors.push(format!(
                        "announcement source fetch failed for {source_id}: {error}"
                    ));
                    continue;
                }
            };
            if let Err(error) = self
                .deliver_announcement_job(ctx, &source, data, lang)
                .await
            {
                recovery_errors.push(error);
            }
        }
        if recovery_errors.is_empty() {
            Ok(())
        } else {
            Err(recovery_errors.join("; "))
        }
    }

    async fn reconcile_members(
        &self,
        ctx: &Context,
        guild_id: GuildId,
        data: &DataManager,
    ) -> SafetyResult<()> {
        let cutoff = data
            .get_data()
            .safety
            .verification_started_at
            .get(&guild_key(guild_id))
            .copied()
            .ok_or_else(|| "verification cutoff missing".to_string())?;
        let mut after = None;
        let mut current_members = HashSet::new();

        loop {
            let members = guild_id
                .members(&ctx.http, Some(1000), after)
                .await
                .map_err(display_error)?;
            if members.is_empty() {
                break;
            }
            for member in &members {
                current_members.insert(member.user.id.get());
                if member.user.bot {
                    continue;
                }
                let joined_at = member
                    .joined_at
                    .map(|timestamp| timestamp.unix_timestamp())
                    .unwrap_or_else(|| Utc::now().timestamp());
                let verified = member.roles.contains(&self.config.verified_role_id);
                let unverified = member.roles.contains(&self.config.unverified_role_id);
                let key = member_key(guild_id, member.user.id);
                if verified {
                    let _ = data.update_data(|bot_data| {
                        bot_data.safety.verification_pending.remove(&key);
                    });
                } else if joined_at >= cutoff.timestamp() {
                    if should_assign_unverified(joined_at, cutoff.timestamp(), verified, unverified)
                    {
                        let result = member
                            .add_role(&ctx.http, self.config.unverified_role_id)
                            .await
                            .map_err(display_error);
                        self.persist_pending(guild_id, member.user.id, result.as_ref().err(), data);
                        if let Err(error) = result {
                            eprintln!("SAFETY offline member role recovery failed: {error}");
                        }
                    } else {
                        self.persist_pending(guild_id, member.user.id, None, data);
                    }
                }
            }
            if members.len() < 1000 {
                break;
            }
            after = members.last().map(|member| member.user.id);
        }

        data.update_data(|bot_data| {
            bot_data.safety.verification_pending.retain(|_, pending| {
                pending.guild_id != guild_id.get()
                    || member_record_is_current(&current_members, pending.user_id)
            });
            bot_data
                .safety
                .announcement_subscriptions
                .retain(|_, subscription| {
                    subscription.guild_id != guild_id.get()
                        || member_record_is_current(&current_members, subscription.user_id)
                });
        })
        .map_err(display_error)
    }

    fn persist_pending(
        &self,
        guild_id: GuildId,
        user_id: UserId,
        error: Option<&String>,
        data: &DataManager,
    ) {
        let pending = VerificationPending {
            guild_id: guild_id.get(),
            user_id: user_id.get(),
            created_at: Utc::now(),
            last_error: error.cloned(),
        };
        if let Err(persist_error) = data.update_data(|bot_data| {
            bot_data
                .safety
                .verification_pending
                .insert(member_key(guild_id, user_id), pending);
        }) {
            eprintln!("SAFETY pending verification persistence failed: {persist_error}");
        }
    }

    async fn reconcile_verification_panel(
        &self,
        ctx: &Context,
        bot_id: UserId,
        guild_id: GuildId,
        channel_id: ChannelId,
        data: &DataManager,
        lang: &LanguageManager,
    ) -> SafetyResult<()> {
        let key = guild_key(guild_id);
        let stored_id = data
            .get_data()
            .safety
            .verification_message_ids
            .get(&key)
            .copied();
        let embed = verification_embed(lang);
        let components = verification_components(guild_id, lang);
        let current_id = self
            .upsert_canonical(
                ctx,
                channel_id,
                stored_id,
                bot_id,
                CanonicalContent {
                    marker: VERIFICATION_MARKER,
                    embed,
                    components,
                },
            )
            .await?;
        data.update_data(|bot_data| {
            bot_data
                .safety
                .verification_message_ids
                .insert(key, current_id.get());
        })
        .map_err(display_error)?;
        delete_other_canonical(ctx, channel_id, bot_id, VERIFICATION_MARKER, current_id).await
    }

    async fn reconcile_honeypot_panel(
        &self,
        ctx: &Context,
        bot_id: UserId,
        guild_id: GuildId,
        data: &DataManager,
        lang: &LanguageManager,
        images: &ImageManager,
    ) -> SafetyResult<()> {
        let channel_id = ChannelId::new(HONEYPOT_CHANNEL_ID);
        let key = guild_key(guild_id);
        let stored_id = data
            .get_data()
            .safety
            .honeypot_message_ids
            .get(&key)
            .copied();
        let embed = honeypot_embed(lang, images);
        let current_id = self
            .upsert_canonical(
                ctx,
                channel_id,
                stored_id,
                bot_id,
                CanonicalContent {
                    marker: HONEYPOT_MARKER,
                    embed,
                    components: Vec::new(),
                },
            )
            .await?;
        data.update_data(|bot_data| {
            bot_data
                .safety
                .honeypot_message_ids
                .insert(key, current_id.get());
        })
        .map_err(display_error)?;
        delete_all_except(ctx, channel_id, current_id).await
    }

    async fn upsert_canonical(
        &self,
        ctx: &Context,
        channel_id: ChannelId,
        stored_id: Option<u64>,
        bot_id: UserId,
        content: CanonicalContent<'_>,
    ) -> SafetyResult<MessageId> {
        let CanonicalContent {
            marker,
            embed,
            components,
        } = content;
        if let Some(message_id) = stored_id.map(MessageId::new) {
            if let Ok(message) = channel_id.message(&ctx.http, message_id).await {
                if is_canonical_message(&message, bot_id, marker) {
                    let edit = EditMessage::new()
                        .embed(embed.clone())
                        .components(components.clone())
                        .allowed_mentions(CreateAllowedMentions::new());
                    if channel_id
                        .edit_message(&ctx.http, message_id, edit)
                        .await
                        .is_ok()
                    {
                        return Ok(message_id);
                    }
                }
            }
        }

        let candidates: Vec<MessageId> = channel_messages(ctx, channel_id)
            .await?
            .into_iter()
            .filter(|message| is_canonical_message(message, bot_id, marker))
            .map(|message| message.id)
            .collect();
        if let Some(message_id) = select_canonical_id(None, &candidates) {
            let edit = EditMessage::new()
                .embed(embed.clone())
                .components(components.clone())
                .allowed_mentions(CreateAllowedMentions::new());
            if channel_id
                .edit_message(&ctx.http, message_id, edit)
                .await
                .is_ok()
            {
                return Ok(message_id);
            }
        }

        channel_id
            .send_message(
                &ctx.http,
                CreateMessage::new()
                    .embed(embed)
                    .components(components)
                    .allowed_mentions(CreateAllowedMentions::new()),
            )
            .await
            .map(|message| message.id)
            .map_err(display_error)
    }

    fn prune_completed_incidents(&self, data: &DataManager) {
        let cutoff = Utc::now() - Duration::days(INCIDENT_RETENTION_DAYS);
        if let Err(error) = data.update_data(|bot_data| {
            bot_data.safety.honeypot_recoveries.retain(|_, incident| {
                incident
                    .completed_at
                    .is_none_or(|completed_at| completed_at >= cutoff)
            });
        }) {
            eprintln!("SAFETY incident prune failed: {error}");
        }
    }

    async fn is_enabled_guild(&self, guild_id: GuildId) -> bool {
        *self.enabled_guild.read().await == Some(guild_id)
    }

    async fn message_matches_enabled_guild(&self, message: &Message) -> bool {
        match message.guild_id {
            Some(guild_id) => self.is_enabled_guild(guild_id).await,
            None => false,
        }
    }
}

async fn guild_channel(ctx: &Context, channel_id: ChannelId) -> SafetyResult<GuildChannel> {
    match channel_id
        .to_channel(&ctx.http)
        .await
        .map_err(display_error)?
    {
        Channel::Guild(channel) => Ok(channel),
        _ => Err(format!("channel {channel_id} is not a guild channel")),
    }
}

fn required_role<'a>(
    roles: &'a HashMap<RoleId, Role>,
    role_id: RoleId,
    name: &str,
) -> SafetyResult<&'a Role> {
    roles
        .get(&role_id)
        .ok_or_else(|| format!("{name} does not exist in the verification guild"))
}

fn aggregate_permissions(
    guild_id: GuildId,
    member: &Member,
    roles: &HashMap<RoleId, Role>,
) -> Permissions {
    let mut permissions = roles
        .get(&guild_id.everyone_role())
        .map_or_else(Permissions::empty, |role| role.permissions);
    for role_id in &member.roles {
        if let Some(role) = roles.get(role_id) {
            permissions |= role.permissions;
        }
    }
    if permissions.administrator() {
        Permissions::all()
    } else {
        permissions
    }
}

fn effective_role_permissions(
    channel: &GuildChannel,
    guild_id: GuildId,
    everyone: &Role,
    role: &Role,
) -> Permissions {
    let mut permissions = everyone.permissions | role.permissions;
    for target_role in [guild_id.everyone_role(), role.id] {
        if let Some(overwrite) = channel.permission_overwrites.iter().find(|overwrite| {
            matches!(overwrite.kind, PermissionOverwriteType::Role(id) if id == target_role)
        }) {
            permissions.remove(overwrite.deny);
            permissions.insert(overwrite.allow);
        }
    }
    permissions
}

fn verification_embed(lang: &LanguageManager) -> CreateEmbed {
    let text = &lang.get().safety.verification;
    CreateEmbed::new()
        .title(&text.title)
        .description(&text.description)
        .color(0x2ecc71)
        .footer(CreateEmbedFooter::new(format!(
            "{} • {VERIFICATION_MARKER}",
            text.footer
        )))
}

fn verification_components(guild_id: GuildId, lang: &LanguageManager) -> Vec<CreateActionRow> {
    let text = &lang.get().safety.verification;
    vec![
        CreateActionRow::Buttons(vec![
            CreateButton::new(SafetyCustomId::new(SafetyAction::Verify, guild_id).encode())
                .label(&text.verify_button)
                .style(ButtonStyle::Success),
            CreateButton::new(SafetyCustomId::new(SafetyAction::NotNow, guild_id).encode())
                .label(&text.not_now_button)
                .style(ButtonStyle::Secondary),
        ]),
        CreateActionRow::Buttons(vec![
            CreateButton::new(SafetyCustomId::new(SafetyAction::Subscribe, guild_id).encode())
                .label(&text.subscribe_button)
                .style(ButtonStyle::Primary),
            CreateButton::new(SafetyCustomId::new(SafetyAction::Unsubscribe, guild_id).encode())
                .label(&text.unsubscribe_button)
                .style(ButtonStyle::Secondary),
        ]),
    ]
}

fn honeypot_embed(lang: &LanguageManager, images: &ImageManager) -> CreateEmbed {
    let text = &lang.get().safety.honeypot;
    let mut embed = CreateEmbed::new()
        .title(&text.title)
        .description(&text.intro)
        .color(0xe74c3c)
        .field("English", &text.english, false)
        .field("Español", &text.spanish, false)
        .field("Português (Brasil)", &text.portuguese_brazil, false)
        .field("Русский", &text.russian, false)
        .field("简体中文", &text.chinese_simplified, false)
        .field("Français", &text.french, false)
        .footer(CreateEmbedFooter::new(&text.footer));
    if let Some(url) = images
        .get_image("safety", "barrier")
        .filter(|url| is_valid_http_image_url(url))
    {
        embed = embed.image(url);
    }
    embed
}

fn is_valid_http_image_url(raw: &str) -> bool {
    Url::parse(raw)
        .ok()
        .is_some_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
}

fn safe_announcement_embed(message: &Message, lang: &LanguageManager) -> CreateEmbed {
    let text = &lang.get().safety.announcement;
    let description = truncate_chars(
        if message.content.trim().is_empty() {
            "(No text content)"
        } else {
            &message.content
        },
        3500,
    );
    CreateEmbed::new()
        .title(&text.title)
        .description(description)
        .color(0x3498db)
        .field(&text.source_label, message.link(), false)
        .footer(CreateEmbedFooter::new(&text.footer))
        .timestamp(message.timestamp)
}

fn truncate_chars(value: &str, limit: usize) -> String {
    let mut truncated: String = value.chars().take(limit).collect();
    if value.chars().count() > limit {
        truncated.push('…');
    }
    truncated
}

async fn delete_other_canonical(
    ctx: &Context,
    channel_id: ChannelId,
    bot_id: UserId,
    marker: &str,
    keep: MessageId,
) -> SafetyResult<()> {
    for message in channel_messages(ctx, channel_id).await? {
        if message.id != keep && is_canonical_message(&message, bot_id, marker) {
            channel_id
                .delete_message(&ctx.http, message.id)
                .await
                .map_err(display_error)?;
        }
    }
    Ok(())
}

async fn delete_all_except(
    ctx: &Context,
    channel_id: ChannelId,
    keep: MessageId,
) -> SafetyResult<()> {
    for message in channel_messages(ctx, channel_id).await? {
        if message.id != keep {
            channel_id
                .delete_message(&ctx.http, message.id)
                .await
                .map_err(display_error)?;
        }
    }
    Ok(())
}

async fn channel_messages(ctx: &Context, channel_id: ChannelId) -> SafetyResult<Vec<Message>> {
    let mut messages = Vec::new();
    let mut before = None;
    loop {
        let mut builder = GetMessages::new().limit(100);
        if let Some(message_id) = before {
            builder = builder.before(message_id);
        }
        let page = channel_id
            .messages(&ctx.http, builder)
            .await
            .map_err(display_error)?;
        if page.is_empty() {
            break;
        }
        before = page.last().map(|message| message.id);
        let page_len = page.len();
        messages.extend(page);
        if page_len < 100 {
            break;
        }
    }
    Ok(messages)
}

fn is_canonical_message(message: &Message, bot_id: UserId, marker: &str) -> bool {
    message.author.id == bot_id
        && message.webhook_id.is_none()
        && message.embeds.iter().any(|embed| {
            embed
                .footer
                .as_ref()
                .is_some_and(|footer| footer.text.contains(marker))
        })
}

fn is_human_message(message: &Message) -> bool {
    !message.author.bot
        && message.webhook_id.is_none()
        && matches!(
            message.kind,
            MessageType::Regular | MessageType::InlineReply
        )
}

fn is_authorized_announcement(message: &Message, owner_id: UserId) -> bool {
    is_authorized_source(
        message.author.id.get(),
        owner_id.get(),
        message.author.bot,
        message.webhook_id.is_some(),
        matches!(
            message.kind,
            MessageType::Regular | MessageType::InlineReply
        ),
        message.guild_id.is_some(),
    )
}

fn is_authorized_source(
    author_id: u64,
    owner_id: u64,
    is_bot: bool,
    is_webhook: bool,
    is_human_message_type: bool,
    has_guild: bool,
) -> bool {
    author_id == owner_id && !is_bot && !is_webhook && is_human_message_type && has_guild
}

fn is_permanent_dm_error(error: &SerenityError) -> bool {
    match error {
        SerenityError::Http(HttpError::UnsuccessfulRequest(response)) => {
            response.error.code == 50007 || matches!(response.status_code.as_u16(), 403 | 404)
        }
        _ => false,
    }
}

fn is_not_found_error(error: &SerenityError) -> bool {
    matches!(
        error,
        SerenityError::Http(HttpError::UnsuccessfulRequest(response))
            if response.status_code.as_u16() == 404
    )
}

fn display_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn guild_key(guild_id: GuildId) -> String {
    guild_id.to_string()
}

fn member_key(guild_id: GuildId, user_id: UserId) -> String {
    format!("{guild_id}:{user_id}")
}

fn select_canonical_id(stored: Option<MessageId>, candidates: &[MessageId]) -> Option<MessageId> {
    stored
        .filter(|message_id| candidates.contains(message_id))
        .or_else(|| candidates.first().copied())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SafetyChannel {
    Announcement,
    Honeypot,
    Other,
}

fn classify_channel(channel_id: u64) -> SafetyChannel {
    match channel_id {
        ANNOUNCEMENT_CHANNEL_ID => SafetyChannel::Announcement,
        HONEYPOT_CHANNEL_ID => SafetyChannel::Honeypot,
        _ => SafetyChannel::Other,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SafetyAction {
    Verify,
    NotNow,
    Subscribe,
    Unsubscribe,
}

impl SafetyAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::Verify => "verify",
            Self::NotNow => "not-now",
            Self::Subscribe => "subscribe",
            Self::Unsubscribe => "unsubscribe",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "verify" => Some(Self::Verify),
            "not-now" => Some(Self::NotNow),
            "subscribe" => Some(Self::Subscribe),
            "unsubscribe" => Some(Self::Unsubscribe),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SafetyCustomId {
    action: SafetyAction,
    guild_id: GuildId,
    version: String,
}

impl SafetyCustomId {
    fn new(action: SafetyAction, guild_id: GuildId) -> Self {
        Self {
            action,
            guild_id,
            version: SAFETY_VERSION.to_string(),
        }
    }

    fn encode(&self) -> String {
        format!(
            "safety:{}:{}:{}",
            self.version,
            self.action.as_str(),
            self.guild_id
        )
    }

    fn parse(value: &str) -> Option<Self> {
        let mut parts = value.split(':');
        if parts.next()? != "safety" {
            return None;
        }
        let version = parts.next()?.to_string();
        let action = SafetyAction::parse(parts.next()?)?;
        let guild_id = GuildId::new(parts.next()?.parse().ok()?);
        if parts.next().is_some() {
            return None;
        }
        Some(Self {
            action,
            guild_id,
            version,
        })
    }
}

fn custom_id_matches_guild_and_version(
    custom_id: &SafetyCustomId,
    guild_id: Option<GuildId>,
) -> bool {
    guild_id == Some(custom_id.guild_id) && custom_id.version == SAFETY_VERSION
}

fn should_assign_unverified(
    joined_at: i64,
    cutoff: i64,
    has_verified: bool,
    has_unverified: bool,
) -> bool {
    joined_at >= cutoff && !has_verified && !has_unverified
}

fn announcement_recipient_eligible(
    active_ledger: bool,
    is_bot: bool,
    has_subscriber_role: bool,
    has_verified_role: bool,
    joined_before_cutoff: bool,
    is_author: bool,
) -> bool {
    active_ledger
        && !is_bot
        && has_subscriber_role
        && (has_verified_role || joined_before_cutoff)
        && !is_author
}

fn pending_delivery_recipients(job: &AnnouncementDelivery) -> Vec<u64> {
    let mut attempted: HashSet<u64> = job
        .delivered_user_ids
        .iter()
        .chain(&job.permanent_failure_user_ids)
        .chain(&job.skipped_user_ids)
        .copied()
        .collect();
    job.recipient_user_ids
        .iter()
        .copied()
        .filter(|user_id| attempted.insert(*user_id))
        .collect()
}

fn member_record_is_current(current_members: &HashSet<u64>, user_id: u64) -> bool {
    current_members.contains(&user_id)
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VerificationRoleMutation {
    AddVerified,
    RemoveUnverified,
}

#[cfg(test)]
fn verification_role_plan(
    action: SafetyAction,
    has_verified: bool,
    has_unverified: bool,
) -> Vec<VerificationRoleMutation> {
    if action != SafetyAction::Verify {
        return Vec::new();
    }
    let mut plan = Vec::new();
    if !has_verified {
        plan.push(VerificationRoleMutation::AddVerified);
    }
    if has_unverified {
        plan.push(VerificationRoleMutation::RemoveUnverified);
    }
    plan
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VerifyState {
    AddVerified,
    RemoveUnverified,
    RollbackVerified,
    Complete,
    Failed,
}

#[cfg(test)]
fn verify_transition(
    state: VerifyState,
    succeeded: bool,
    verified_was_preexisting: bool,
) -> VerifyState {
    match (state, succeeded, verified_was_preexisting) {
        (VerifyState::AddVerified, true, _) => VerifyState::RemoveUnverified,
        (VerifyState::AddVerified, false, _) => VerifyState::Failed,
        (VerifyState::RemoveUnverified, true, _) => VerifyState::Complete,
        (VerifyState::RemoveUnverified, false, false) => VerifyState::RollbackVerified,
        (VerifyState::RemoveUnverified, false, true) => VerifyState::Failed,
        (VerifyState::RollbackVerified, _, _) => VerifyState::Failed,
        (terminal, _, _) => terminal,
    }
}

fn honeypot_transition(stage: HoneypotStage, succeeded: bool) -> HoneypotStage {
    match (stage, succeeded) {
        (HoneypotStage::Received, _) => HoneypotStage::NoticeAttempted,
        (HoneypotStage::NoticeAttempted, _) => HoneypotStage::BanPending,
        (HoneypotStage::BanPending, true) => HoneypotStage::UnbanPending,
        (HoneypotStage::BanPending, false) => HoneypotStage::BanFailed,
        (HoneypotStage::UnbanPending, true) => HoneypotStage::Completed,
        (HoneypotStage::UnbanPending, false) => HoneypotStage::UnbanPending,
        (terminal, _) => terminal,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::BotData;

    #[test]
    fn channel_classifier_recognizes_announcement() {
        assert_eq!(
            classify_channel(ANNOUNCEMENT_CHANNEL_ID),
            SafetyChannel::Announcement
        );
    }

    #[test]
    fn channel_classifier_recognizes_honeypot() {
        assert_eq!(
            classify_channel(HONEYPOT_CHANNEL_ID),
            SafetyChannel::Honeypot
        );
    }

    #[test]
    fn custom_id_round_trip_binds_guild_action_and_version() {
        let value = SafetyCustomId::new(SafetyAction::Verify, GuildId::new(42));
        assert_eq!(SafetyCustomId::parse(&value.encode()), Some(value));
    }

    #[test]
    fn custom_id_rejects_extra_fields() {
        assert!(SafetyCustomId::parse("safety:v1:verify:42:7").is_none());
    }

    #[test]
    fn custom_id_rejects_stale_version_binding() {
        let custom_id = SafetyCustomId {
            action: SafetyAction::Verify,
            guild_id: GuildId::new(42),
            version: "v0".to_string(),
        };
        assert!(!custom_id_matches_guild_and_version(
            &custom_id,
            Some(GuildId::new(42))
        ));
    }

    #[test]
    fn cutoff_grandfathers_earlier_member() {
        assert!(!should_assign_unverified(99, 100, false, false));
    }

    #[test]
    fn post_cutoff_join_gets_unverified_role() {
        assert!(should_assign_unverified(100, 100, false, false));
    }

    #[test]
    fn offline_missed_post_cutoff_member_is_reconciled() {
        assert!(should_assign_unverified(101, 100, false, false));
    }

    #[test]
    fn verified_member_prevents_unverified_reassignment() {
        assert!(!should_assign_unverified(101, 100, true, false));
    }

    #[test]
    fn existing_unverified_role_persists_without_duplicate_assignment() {
        assert!(!should_assign_unverified(101, 100, false, true));
    }

    #[test]
    fn not_now_has_no_kick_effect() {
        assert!(verification_role_plan(SafetyAction::NotNow, false, true).is_empty());
    }

    #[test]
    fn verify_role_plan_adds_verified_before_removing_unverified() {
        assert_eq!(
            verification_role_plan(SafetyAction::Verify, false, true),
            vec![
                VerificationRoleMutation::AddVerified,
                VerificationRoleMutation::RemoveUnverified
            ]
        );
    }

    #[test]
    fn verify_add_failure_stops_fail_closed() {
        assert_eq!(
            verify_transition(VerifyState::AddVerified, false, false),
            VerifyState::Failed
        );
    }

    #[test]
    fn verify_remove_failure_requires_verified_rollback() {
        assert_eq!(
            verify_transition(VerifyState::RemoveUnverified, false, false),
            VerifyState::RollbackVerified
        );
    }

    #[test]
    fn subscriber_requires_ledger_role_and_verification_or_grandfathering() {
        assert!(!announcement_recipient_eligible(
            true, false, false, true, false, false
        ));
    }

    #[test]
    fn verified_subscriber_is_eligible() {
        assert!(announcement_recipient_eligible(
            true, false, true, true, false, false
        ));
    }

    #[test]
    fn grandfathered_subscriber_is_eligible_without_verified_role() {
        assert!(announcement_recipient_eligible(
            true, false, true, false, true, false
        ));
    }

    #[test]
    fn revoked_subscriber_is_not_eligible() {
        assert!(!announcement_recipient_eligible(
            false, false, true, true, false, false
        ));
    }

    #[test]
    fn announcement_author_is_excluded() {
        assert!(!announcement_recipient_eligible(
            true, false, true, true, false, true
        ));
    }

    #[test]
    fn announcement_source_fails_closed_for_wrong_owner() {
        assert!(!is_authorized_source(1, 2, false, false, true, true));
    }

    #[test]
    fn announcement_source_fails_closed_for_webhook() {
        assert!(!is_authorized_source(2, 2, false, true, true, true));
    }

    #[test]
    fn announcement_delivery_dedupe_returns_only_unattempted_unique_users() {
        let job = AnnouncementDelivery {
            guild_id: 1,
            source_message_id: 2,
            recipient_user_ids: vec![10, 11, 12, 13, 13],
            delivered_user_ids: vec![10],
            permanent_failure_user_ids: vec![11],
            skipped_user_ids: vec![12],
            started_at: Utc::now(),
            completed_at: None,
        };
        assert_eq!(pending_delivery_recipients(&job), vec![13]);
    }

    #[test]
    fn departed_member_record_is_pruned() {
        assert!(!member_record_is_current(&HashSet::from([10]), 11));
    }

    #[test]
    fn honeypot_ban_failure_never_transitions_to_unban() {
        assert_eq!(
            honeypot_transition(HoneypotStage::BanPending, false),
            HoneypotStage::BanFailed
        );
    }

    #[test]
    fn honeypot_unban_failure_remains_recoverable() {
        assert_eq!(
            honeypot_transition(HoneypotStage::UnbanPending, false),
            HoneypotStage::UnbanPending
        );
    }

    #[test]
    fn honeypot_delete_window_is_exactly_172800_seconds() {
        assert_eq!(u32::from(HONEYPOT_DELETE_DAYS) * 86_400, 172_800);
    }

    #[test]
    fn legacy_json_deserializes_with_default_safety_state() {
        let legacy = r#"{
            "button_messages": {},
            "conversations": {},
            "reminders": {},
            "feedback_messages": {},
            "last_updated": "2026-07-14T00:00:00Z"
        }"#;
        let data: BotData = serde_json::from_str(legacy).expect("legacy JSON should deserialize");
        assert!(data.safety.verification_started_at.is_empty());
    }

    #[test]
    fn canonical_selection_keeps_stored_message_when_present() {
        let stored = Some(MessageId::new(7));
        let candidates = [MessageId::new(5), MessageId::new(7), MessageId::new(9)];
        assert_eq!(select_canonical_id(stored, &candidates), stored);
    }

    #[test]
    fn six_warning_languages_are_complete_and_embed_is_under_limit() {
        let manager = LanguageManager::new().expect("language config should parse");
        let warning = &manager.get().safety.honeypot;
        let values = [
            &warning.english,
            &warning.spanish,
            &warning.portuguese_brazil,
            &warning.russian,
            &warning.chinese_simplified,
            &warning.french,
        ];
        let total = warning.title.chars().count()
            + warning.intro.chars().count()
            + warning.footer.chars().count()
            + values
                .iter()
                .map(|value| value.chars().count())
                .sum::<usize>();
        assert!(values.iter().all(|value| !value.trim().is_empty()) && total <= 6000);
    }
}
