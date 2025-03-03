use serenity::async_trait;
use serenity::builder::{CreateInteractionResponse, CreateInteractionResponseMessage};
use serenity::model::prelude::*;
use serenity::prelude::*;
use songbird::{SerenityInit, input::YoutubeDl};
use dotenvy::dotenv;
use std::env;
use rand::Rng;

#[tokio::main]
async fn main() {
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("Missing DISCORD_TOKEN in .env");
    let intents = GatewayIntents::GUILDS | GatewayIntents::GUILD_VOICE_STATES;

    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .register_songbird()
        .await
        .expect("Error creating client");

    if let Err(e) = client.start().await {
        println!("Client error: {:?}", e);
    }
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("✅ {} is connected!", ready.user.name);

        if let Some(guild_id) = env::var("GUILD_ID")
            .ok()
            .and_then(|id| id.parse::<u64>().ok())
            .map(GuildId::from)
        {
            println!("Clearing old commands...");
            clear_guild_commands(&ctx, guild_id).await;

            println!("Registering commands for guild: {:?}", guild_id);
            register_commands(&ctx, guild_id).await;
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            let guild_id = match command.guild_id {
                Some(id) => id,
                None => {
                    let _ = command.create_response(
                        &ctx.http,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content("❌ This command can only be used in a server!")
                        ),
                    ).await;
                    return;
                }
            };

            match command.data.name.as_str() {
                "play" => {
                    let url = command.data.options.get(0)
                        .and_then(|opt| match &opt.value {
                            CommandDataOptionValue::String(link) => Some(link.clone()),
                            _ => None,
                        });

                    let Some(url) = url else {
                        let _ = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("❌ URL is required!")
                            )
                        ).await;
                        return;
                    };

                    println!("Received play command with URL: {}", url);

                    let client = reqwest::Client::new();
                    let source = YoutubeDl::new_search(client, url.clone());

                    let manager = songbird::get(&ctx).await.unwrap().clone();
                    if let Some(call_lock) = manager.get(guild_id) {
                        let mut call = call_lock.lock().await;
                        let _handle = call.play(source.into());
                        println!("Audio started playing.");
                        let _ = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content(format!("🎶 Now playing: {}", url))
                            )
                        ).await;
                    } else {
                        let _ = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("❌ Bot is not in a voice channel! Use `/join` first.")
                            )
                        ).await;
                    }
                }
                "join" => {
                    let manager = songbird::get(&ctx).await.unwrap().clone();
                    let channel_id = get_user_voice_channel(&ctx, guild_id, command.user.id).await;
                    if let Some(ch) = channel_id {
                        if let Err(e) = manager.join(guild_id, ch).await {
                            println!("Failed to join voice channel: {:?}", e);
                        } else {
                            let _ = command.create_response(
                                &ctx.http,
                                CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content("🎤 Joined your voice channel!")
                                ),
                            ).await;
                        }
                    } else {
                        let _ = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("❌ You are not in a voice channel!")
                            ),
                        ).await;
                    }
                }
                "quit" => {
                    let manager = songbird::get(&ctx).await.unwrap().clone();
                    if manager.get(guild_id).is_some() {
                        if let Err(e) = manager.remove(guild_id).await {
                            println!("Error leaving voice channel: {:?}", e);
                        }
                        let _ = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("👋 Left the voice channel!")
                            ),
                        ).await;
                    } else {
                        let _ = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("❌ Bot is not in a voice channel!")
                            ),
                        ).await;
                    }
                }
                "roll" => {
                    let roll = rand::thread_rng().gen_range(1..=6);
                    let _ = command.create_response(
                        &ctx.http,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(format!("🎲 You rolled: {}", roll))
                        )
                    ).await;
                }
                _ => {}
            }
        }
    }
}

async fn clear_guild_commands(ctx: &Context, guild_id: GuildId) {
    if let Err(e) = guild_id.set_commands(&ctx.http, Vec::new()).await {
        eprintln!("❌ Failed to clear commands: {}", e);
    } else {
        println!("✅ All old guild commands deleted.");
    }
}

async fn register_commands(ctx: &Context, guild_id: GuildId) {
    use serenity::all::{CreateCommand, CreateCommandOption, CommandOptionType};

    let commands = vec![
        CreateCommand::new("roll")
            .description("🎲 Rolls a random number"),
        CreateCommand::new("play")
            .description("🎶 Plays music from a given URL")
            .add_option(
                CreateCommandOption::new(CommandOptionType::String, "url", "URL to play")
                    .required(true)
            ),
        CreateCommand::new("join")
            .description("🎤 Joins your voice channel"),
        CreateCommand::new("quit")
            .description("👋 Leaves the voice channel"),
    ];

    if let Err(e) = guild_id.set_commands(&ctx.http, commands).await {
        eprintln!("❌ Failed to register commands: {}", e);
    }
}

async fn get_user_voice_channel(ctx: &Context, guild_id: GuildId, user_id: UserId) -> Option<ChannelId> {
    let guild = ctx.cache.guild(guild_id)?;
    guild.voice_states.get(&user_id)?.channel_id
}