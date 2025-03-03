// mainFunction: this is the entry point of the bot. It loads the environment variables,
// sets up the discord client with voice support via Songbird, and starts the bot
// make sure you have a .env file with DISCORD_TOKEN and GUILD_ID configured
use serenity::async_trait;
use serenity::builder::{CreateInteractionResponse, CreateInteractionResponseMessage};
use serenity::model::prelude::*;
use serenity::prelude::*;
use songbird::{SerenityInit, input::YoutubeDl};
use dotenvy::dotenv;
use std::env;
use rand::Rng;
use reqwest::Client as ReqwestClient;
use serenity::Client as DiscordClient;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    // loadEnv: load environment variables from the .env file so that sensitive info isn't hardcoded
    dotenv().ok();

    // getToken: retrieve the Discord bot token from the environment
    let token = env::var("DISCORD_TOKEN").expect("Missing DISCORD_TOKEN in .env");

    // setIntents: Specify the discord events we need (guilds and voice state updates)
    let intents = GatewayIntents::GUILDS | GatewayIntents::GUILD_VOICE_STATES;

    // createHttpClient: initialize the Reqwest HTTP client; used later by YoutubeDl to fetch audio
    let reqwest_client = ReqwestClient::new();

    // initHandler: create an instance of our event handler. This struct holds our HTTP client
    let handler = Handler { client: reqwest_client };

    // buildClient: construct the discord client, integrate Songbird for voice features,
    // and attach our event handler to process incoming events
    let mut bot = DiscordClient::builder(&token, intents)
        .register_songbird() // registerSongbird: adds voice support to the bot
        .event_handler(handler) // setEventHandler: connects our handler to process events like commands
        .await
        .expect("Error creating client");

    // startBot: run the bot. If an error occurs, print it to the console
    if let Err(e) = bot.start().await {
        eprintln!("Client error: {:?}", e);
    }
}

// handlerStruct: defines the event handler for our bot. It holds any dependencies needed,
// in this case the Reqwest client for making HTTP requests
struct Handler {
    client: ReqwestClient,
}

#[async_trait]
impl EventHandler for Handler {
    // readyEvent: triggered when the bot is successfully connected to discord
    // it registers slash commands and performs any necessary startup logic
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("📌 {} is connected!", ready.user.name);

        // loadGuildId: attempt to retrieve the guild ID from the environment
        // this guild ID is used for registering slash commands
        if let Some(guild_id) = env::var("GUILD_ID")
            .ok()
            .and_then(|id| id.parse::<u64>().ok())
            .map(GuildId::from)
        {
            println!("📎clearingOldCommands: Removing any existing guild commands...");
            clear_guild_commands(&ctx, guild_id).await;
            println!("📎registeringCommands: Adding commands for guild: {:?}", guild_id);
            register_commands(&ctx, guild_id).await;
        }
    }

    // interactionCreate: handles slash-command interactions from discord
    // this function processes commands like play, join, quit, and roll
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            // ensureGuild: check that the command was issued in a guild (server)
            // if not, respond with an error message
            let Some(guild_id) = command.guild_id else {
                let _ = command.create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("🟥 This command can only be used in a server!")
                    ),
                ).await;
                return;
            };

            // matchCommand: determine which command was issued and process accordingly
            match command.data.name.as_str() {
                "play" => {
                    // extractUrl: retrieve the URL parameter from the command options
                    let url = command.data.options.get(0)
                        .and_then(|opt| match &opt.value {
                            CommandDataOptionValue::String(link) => Some(link.clone()),
                            _ => None,
                        });

                    // validate that a URL was provided
                    let Some(url) = url else {
                        let _ = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("🟥 URL is required!")
                            )
                        ).await;
                        return;
                    };

                    println!("receivedPlayCommand: URL provided: {}", url);

                    // verifyUrl: ensure the provided URL starts with "https://"
                    // this basic validation helps prevent invalid or malicious input
                    if !url.starts_with("https://") {
                        let _ = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("🟥 Invalid URL!")
                            )
                        ).await;
                        return;
                    }

                    // createSource: use YoutubeDl to create an audio source from the URL,
                    // leveraging our HTTP client for network requests
                    let source = YoutubeDl::new_search(self.client.clone(), url.clone());
                    // getManager: retrieve the Songbird voice manager responsible for voice operations
                    let manager = songbird::get(&ctx).await.unwrap().clone();

                    // checkVoiceChannel: verify that the bot is currently in a voice channel
                    if let Some(call_lock) = manager.get(guild_id) {
                        let mut call = call_lock.lock().await;
                        // clearPreviousTrack: stop any currently playing audio to free resources
                        call.stop();
                        // playAudio: begin playing the new audio source
                        let _handle = call.play(source.into());
                        println!("audioPlaying: Now playing {}", url);
                        let _ = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content(format!("🎶 Now playing: {}", url))
                            )
                        ).await;
                    } else {
                        // botNotInVoice: inform the user that the bot must join a voice channel first
                        let _ = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("🟥 Bot is not in a voice channel! Use `/join` first.")
                            )
                        ).await;
                    }
                }
                "join" => {
                    // joinVoice: attempt to have the bot join the voice channel of the user
                    let manager = songbird::get(&ctx).await.unwrap().clone();
                    let channel_id = get_user_voice_channel(&ctx, guild_id, command.user.id).await;
                    if let Some(ch) = channel_id {
                        if let Err(e) = manager.join(guild_id, ch).await {
                            println!("failedToJoinVoiceChannel: {:?}", e);
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
                        // userNotInVoice: notify the user if they are not connected to a voice channel
                        let _ = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("🟥 You are not in a voice channel!")
                            ),
                        ).await;
                    }
                }
                "quit" => {
                    // quitVoice: disconnect the bot from the voice channel, if it is connected
                    let manager = songbird::get(&ctx).await.unwrap().clone();
                    if manager.get(guild_id).is_some() {
                        if let Err(e) = manager.remove(guild_id).await {
                            println!("failedToQuitVoiceChannel: {:?}", e);
                        }
                        let _ = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("👋 Left the voice channel!")
                            ),
                        ).await;
                    } else {
                        // botNotInVoiceForQuit: inform the user if the bot is not currently connected
                        let _ = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("🟥 Bot is not in a voice channel!")
                            ),
                        ).await;
                    }
                }
                "roll" => {
                    // rollDice: generate a random number between 1-6
                    // Note: we wrap the RNG creation in a block so that it doesn't cross an await boundary,
                    // ensuring the future is Send
                    let roll = {
                        let mut rng = rand::rng(); // createRng: using the updated rand API
                        rng.random_range(1..=6)
                    };
                    let _ = command.create_response(
                        &ctx.http,
                        CreateInteractionResponse::Message(
                            CreateInteractionResponseMessage::new()
                                .content(format!("🎲 You rolled: **{}**", roll))
                        )
                    ).await;
                }
                _ => {} // unhandledCommand: ignore any commands that we do not recognize
            }
        }
    }
}

// clearGuildCommands: clears all registered slash commands for the specified guild
// this ensures that old or deprecated commands are removed before registering new ones
#[inline(always)]
async fn clear_guild_commands(ctx: &Context, guild_id: GuildId) {
    if let Err(e) = guild_id.set_commands(&ctx.http, Vec::new()).await {
        eprintln!("🟥 failedToClearCommands: {}", e);
    } else {
        println!("📌 clearedGuildCommands: All old guild commands deleted.");
    }
}

// registerCommands: registers the slash commands for the bot
// this function sets up the commands: roll, play, join, and quit
#[inline(always)]
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
        eprintln!("🟥 failedToRegisterCommands: {}", e);
    }
}

// getUserVoiceChannel: returns the voice channel ID where the user is currently connected,
// or None if the user is not in any voice channel
async fn get_user_voice_channel(ctx: &Context, guild_id: GuildId, user_id: UserId) -> Option<ChannelId> {
    let guild = ctx.cache.guild(guild_id)?;
    guild.voice_states.get(&user_id)?.channel_id
}