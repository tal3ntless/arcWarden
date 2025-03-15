// mainFunction: this is the entry point of the bot. It loads the environment variables,
// sets up the discord client with voice support via Songbird, and starts the bot
// make sure you have a .env file with DISCORD_TOKEN and GUILD_ID configured
use serenity::all::{CreateCommand};
use serenity::async_trait;
use serenity::builder::{CreateInteractionResponse, CreateInteractionResponseMessage, CreateChannel};
use serenity::model::channel::ChannelType;
use serenity::model::Permissions;
use serenity::model::prelude::*;
use serenity::prelude::*;
use songbird::{SerenityInit};
use dotenvy::dotenv;
use std::env;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU32, Ordering};
use serenity::Client as DiscordClient;

// global variable to store the last generated ticket ID
static LAST_TICKET_ID: AtomicU32 = AtomicU32::new(10000);
// consts for allowed channel and ticket category IDs
const TICKET_CATEGORY_ID: u64 = 1346903349911617566; // ticket category ID
const ALLOWED_CHANNEL_ID: u64 = 1346937627852668938;   // allowed channel ID for ticket command
// mod roles allowed to close tickets (update with your actual role IDs)
fn mod_roles() -> Vec<RoleId> {
    vec![
        RoleId::new(1114632710917476494),
        RoleId::new(1293950309999181824),
        RoleId::new(1293949160688451634),
        RoleId::new(1293949517816664159),
        RoleId::new(1293950811239481459),
    ]
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    // loadEnv: load environment variables from the .env file so that sensitive info isn't hardcoded
    dotenv().ok();

    // getToken: retrieve the Discord bot token from the environment
    let token = env::var("DISCORD_TOKEN").expect("Missing DISCORD_TOKEN in .env");

    // setIntents: Specify the discord events we need (guilds and voice state updates)
    let intents = GatewayIntents::GUILDS;

    // initHandler: create an instance of our event handler. This struct holds our HTTP client
    let handler = Handler;

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
struct Handler;

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
            println!("📎 Clearing old guild commands...");
            clear_guild_commands(&ctx, guild_id).await;
            println!("📎 Clearing global commands...");
            clear_global_commands(&ctx).await;
            println!("📎 Registering commands for guild: {:?}", guild_id);
            register_commands(&ctx, guild_id).await;
        }
    }

    // interactionCreate: handles slash-command interactions from discord
    // this function processes commands like play, join, quit, roll, and ticket commands
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            // ensureGuild: check that the command was issued in a guild (server)
            // if not, respond with an error message
            let Some(guild_id) = command.guild_id else {
                let _ = command.create_response(
                    &ctx.http,
                    CreateInteractionResponse::Message(
                        CreateInteractionResponseMessage::new()
                            .content("🛑 This command can only be used in a server!")
                    ),
                ).await;
                return;
            };

            // matchCommand: determine which command was issued and process accordingly
            match command.data.name.as_str() {
                "ticket" => {
                    // Check that the ticket command is used only in the designated channel
                    if command.channel_id != ChannelId::new(ALLOWED_CHANNEL_ID) {
                        let _ = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("🛑 This command can only be used in the designated channel.")
                            )
                        ).await;
                        return; // Prevent further execution if channel is not allowed
                    }

                    match create_ticket_channel(
                        &ctx,
                        guild_id,
                        command.user.id,
                        ChannelId::new(TICKET_CATEGORY_ID),
                        &mod_roles()
                    ).await {
                        Ok(channel_id) => {
                            let _ = command.create_response(
                                &ctx.http,
                                CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content(format!("✅ Ticket created: <#{}>", channel_id))
                                )
                            ).await;
                        },
                        Err(e) => {
                            eprintln!("🛑 Ticket creation error: {:?}", e);
                            let _ = command.create_response(
                                &ctx.http,
                                CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content("🛑 Could not create ticket, please try again later.")
                                )
                            ).await;
                        }
                    }
                }
                "ticketclose" => {
                    // Check permissions: only users with a mod role can close tickets
                    let mod_roles = mod_roles();
                    // Use the member data attached to the command (available in guild commands)
                    if let Some(member) = command.member.clone() {
                        if !member.roles.iter().any(|role| mod_roles.contains(role)) {
                            let _ = command.create_response(
                                &ctx.http,
                                CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content("🛑 You do not have permission to close this ticket.")
                                )
                            ).await;
                            return;
                        }
                    } else {
                        let _ = command.create_response(
                            &ctx.http,
                            CreateInteractionResponse::Message(
                                CreateInteractionResponseMessage::new()
                                    .content("🛑 Unable to verify your permissions.")
                            )
                        ).await;
                        return;
                    }

                    let channel = command.channel_id.to_channel(&ctx.http).await.unwrap();
                    if let Some(guild_channel) = channel.guild() {
                        // initialize list of channels that cannot be deleted
                        let ignored_channels: HashSet<u64> = vec![
                            1346937627852668938,
                        ].into_iter().collect();

                        let channel_id_u64: u64 = u64::from(guild_channel.id);
                        if ignored_channels.contains(&channel_id_u64) {
                            let _ = command.create_response(
                                &ctx.http,
                                CreateInteractionResponse::Message(
                                    CreateInteractionResponseMessage::new()
                                        .content("🛑 This channel cannot be closed.")
                                )
                            ).await;
                            return;
                        }

                        println!("✅ Closing ticket in channel: {}", guild_channel.name);
                        match close_ticket(&ctx, command.channel_id).await {
                            Ok(_) => {
                                let _ = command.create_response(
                                    &ctx.http,
                                    CreateInteractionResponse::Message(
                                        CreateInteractionResponseMessage::new()
                                            .content("✅ Ticket has been closed.")
                                    )
                                ).await;
                            },
                            Err(e) => {
                                eprintln!("🛑 Error closing ticket: {:?}", e);
                                let _ = command.create_response(
                                    &ctx.http,
                                    CreateInteractionResponse::Message(
                                        CreateInteractionResponseMessage::new()
                                            .content("🛑 Failed to close the ticket, please try again later.")
                                    )
                                ).await;
                            }
                        }
                    }
                }
                _ => {} // unhandledCommand: ignore any commands that we do not recognize
            }
        }
    }
}

// generateTicketID: func to generate a unique ticket ID (increments by 1 with each call)
fn generate_ticket_id() -> u32 {
    LAST_TICKET_ID.fetch_add(1, Ordering::Relaxed)
}

// createTicket: function to create a ticket channel
// ctx – context, guild_id – guild ID, initiator – user ID, ticket_category_id – ticket category channel ID,
// mod_role_ids – slice of moderator role IDs
async fn create_ticket_channel(
    ctx: &Context,
    guild_id: GuildId,
    initiator: UserId,
    ticket_category_id: ChannelId,
    mod_role_ids: &[RoleId],
) -> Result<ChannelId, serenity::Error> {
    let ticket_id = generate_ticket_id();
    let channel_name = format!("ticket-{}", ticket_id);

    println!("✅ Creating ticket: ID #{} for user: {}", ticket_id, initiator);

    // ticketPerms: build the permission overwrites vector
    let mut overwrites = Vec::new();

    // deny access for @everyone (role with ID equal to guild_id)
    overwrites.push(PermissionOverwrite {
        kind: PermissionOverwriteType::Role(RoleId::new(u64::from(guild_id))),
        allow: Permissions::empty(),
        deny: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
    });

    // grant initiator permission to view and send messages in the channel
    overwrites.push(PermissionOverwrite {
        allow: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
        deny: Permissions::empty(),
        kind: PermissionOverwriteType::Member(initiator),
    });
    // grant each mod role permission to operate with channel
    for mod_role in mod_role_ids {
        overwrites.push(PermissionOverwrite {
            allow: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
            deny: Permissions::empty(),
            kind: PermissionOverwriteType::Role(*mod_role),
        });
    }
    // grant the bot permission to view and send messages in the channel
    overwrites.push(PermissionOverwrite {
        allow: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
        deny: Permissions::empty(),
        kind: PermissionOverwriteType::Member(ctx.cache.current_user().id),
    });

    let create_channel = CreateChannel::new(channel_name)  // always specify name
        .kind(ChannelType::Text)
        .category(ticket_category_id)
        .permissions(overwrites);

    let new_channel = guild_id.create_channel(&ctx.http, create_channel).await?;

    println!("✅ Ticket created: {} (channel ID: {})", new_channel.name, new_channel.id);
    Ok(new_channel.id)
}

// ticketClose: function to close a ticket (delete the channel)
// before deletion, attempt to extract the ticket ID from the channel name for logging purposes
async fn close_ticket(ctx: &Context, channel_id: ChannelId) -> Result<(), serenity::Error> {
    channel_id.delete(&ctx.http).await?;
    Ok(())
}

// clearGuildCommands: clears all registered slash commands for the specified guild
// this ensures that old or deprecated commands are removed before registering new ones
#[inline(always)]
async fn clear_guild_commands(ctx: &Context, guild_id: GuildId) {
    if let Err(e) = guild_id.set_commands(&ctx.http, Vec::new()).await {
        eprintln!("🛑 failedToClearCommands: {}", e);
    } else {
        println!("📌 clearedGuildCommands: All old guild commands deleted.");
    }
}

// registerCommands: registers the slash commands for the bot
// this function sets up the commands: roll, play, join, quit, and the new ticket commands (ticket, ticketclose)
#[inline(always)]
async fn register_commands(ctx: &Context, guild_id: GuildId) {
    let commands = vec![
        CreateCommand::new("ticket")
            .description("📍 Creates a new ticket"),
        CreateCommand::new("ticketclose")
            .description("📁 Closes the current ticket"),
    ];

    if let Err(e) = guild_id.set_commands(&ctx.http, commands).await {
        eprintln!("🛑 failedToRegisterCommands: {}", e);
    }
}
async fn clear_global_commands(ctx: &Context) {
    match ctx.http.get_global_commands().await {
        Ok(commands) => {
            for command in commands {
                if let Err(e) = ctx.http.delete_global_command(command.id).await {
                    eprintln!("🛑 Failed to delete global command {}: {}", command.name, e);
                }
            }
            println!("📌 Cleared global commands.");
        }
        Err(e) => eprintln!("🛑 Failed to fetch global commands: {}", e),
    }
}