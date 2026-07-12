use std::{env, str::FromStr};

use anyhow::Context as _;

use serenity::{
    model::id::{ChannelId, GuildId},
    prelude::*,
};

use udamanami::ai;
use udamanami::db::BotDatabase;
use udamanami::Bot;

fn env_var(key: &str) -> Option<String> {
    env::var(key).ok()
}

fn env_var_required(key: &str) -> anyhow::Result<String> {
    env::var(key).with_context(|| format!("'{key}' was not found"))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Get the discord token set in environment variables
    let token = env_var_required("DISCORD_TOKEN")?;

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT
        | GatewayIntents::DIRECT_MESSAGES;

    let channel_ids = env_var("ROOMS_ID").map_or(vec![], |rooms| {
        rooms
            .split(',')
            .filter_map(|id| ChannelId::from_str(id.trim()).ok())
            .collect()
    });

    let debug_channel_id = env_var("DEBUG_ROOM_ID")
        .map(|id| ChannelId::from_str(id.trim()).unwrap())
        .unwrap_or_default();

    let disabled_commands = env_var("DISABLED_COMMANDS")
        .map(|commands| {
            commands
                .split(',')
                .map(|command| command.trim().to_owned())
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();
    let disabled_commands = disabled_commands
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>();

    let guild_id = env_var("DISCORD_GUILD_ID")
        .map(|id| GuildId::from_str(id.trim()).unwrap())
        .unwrap();

    let commit_hash = env_var("COMMIT_HASH");

    let commit_date = env_var("COMMIT_DATE");

    let gpt = ai::GptAI::manami(&env_var_required("OPENAI_API_KEY")?);

    let database_path = env_var("DATABASE_PATH").unwrap_or_else(|| "./db.sqlite".to_owned());
    let database = BotDatabase::new(&database_path).await?;

    let bot = Bot::new(
        channel_ids,
        debug_channel_id,
        guild_id,
        gpt,
        commit_hash,
        commit_date,
        &disabled_commands,
        database,
    )
    .await;

    let mut client = Client::builder(&token, intents)
        .event_handler(bot)
        .await
        .expect("Err creating client");

    client.start().await?;

    Ok(())
}
