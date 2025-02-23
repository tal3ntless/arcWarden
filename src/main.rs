use serenity::async_trait;
use serenity::model::gateway::Ready;
use serenity::model::prelude::*;
use serenity::prelude::*;
use dotenvy::dotenv;
use rand::Rng;
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use std::time::Instant;
use serenity::builder::GetMessages;

/// the basic structure of the bot event handler.
/// stores the global cooldown for the `!clear` command.
struct Handler {
    last_clear: Arc<Mutex<Instant>>, // global cooldown for clearing messages
}

impl Handler {
    /// creates a new event handler instance.
    fn new() -> Self {
        Self {
            last_clear: Arc::new(Mutex::new(Instant::now() - Duration::from_secs(30))),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    /// called when the bot has successfully connected to discord.
    async fn ready(&self, _ctx: Context, ready: Ready) {
        println!("Bot {} is up and running!", ready.user.name);
    }

    /// handles incoming messages.
    async fn message(&self, ctx: Context, msg: Message) {
        // ignore messages from other bots
        if msg.author.bot {
            return;
        }

        let args: Vec<&str> = msg.content.split_whitespace().collect();
        match args.first().map(|s| *s) {
            // dice roll command
            Some("!roll") => {
                let roll = rand::thread_rng().gen_range(1..=6);
                let response = format!("🎲 You rolled a **{}**!", roll);
                if let Err(e) = msg.channel_id.say(&ctx.http, response).await {
                    println!("Error sending the message: {:?}", e);
                }
            }
            // message clear command
            Some("!clear") => {
                let role_name = "exampleRole";

                // check if a user has the required role
                if let Some(guild_id) = msg.guild_id {
                    if let Ok(member) = guild_id.member(&ctx.http, msg.author.id).await {
                        let mut has_role = false;

                        for r in &member.roles {
                            if let Ok(role) = guild_id.role(&ctx.http, *r).await {
                                if role.name == role_name {
                                    has_role = true;
                                    break;
                                }
                            }
                        }

                        if !has_role {
                            let _ = msg.channel_id.say(&ctx.http, "🟥 You do not have permission to use this command.").await;
                            return;
                        }
                    }
                }

                // check the global cooldown
                let now = Instant::now();
                {
                    let mut last_clear = self.last_clear.lock().await;
                    let elapsed = now.duration_since(*last_clear);

                    if elapsed < Duration::from_secs(30) {
                        let remaining = 30 - elapsed.as_secs();
                        let _ = msg.channel_id.say(&ctx.http, format!("🕗 Hold on another **{}** seconds.", remaining)).await;
                        return;
                    }

                    *last_clear = now;
                } // unlock mutexGuard

                // check the message count arg
                if let Some(num_str) = args.get(1) {
                    if let Ok(mut num) = num_str.parse::<u64>() {
                        num = num.min(15); // limit the number of deleted posts to 15

                        // get the last messages in the channel
                        if let Ok(messages) = msg.channel_id.messages(&ctx.http, GetMessages::default().limit(num as u8)).await {
                            let messages_to_delete: Vec<MessageId> = messages.iter().map(|m| m.id).collect();

                            // delete messages
                            if let Err(e) = msg.channel_id.delete_messages(&ctx.http, messages_to_delete).await {
                                println!("Message deletion error: {:?}", e);
                            } else {
                                let _ = msg.channel_id.say(&ctx.http, format!("🟩 Deleted **{}** messages\n🕗 Cooldown: 30 sec.", num)).await;
                            }
                        }
                    } else {
                        let _ = msg.channel_id.say(&ctx.http, "🟥 Error: specify the number of messages (e.g., `!clear 10`).").await;
                    }
                } else {
                    let _ = msg.channel_id.say(&ctx.http, "🟥 Error: specify how many messages to delete (e.g. `!clear 5`).").await;
                }
            }
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok(); // load environment variables from `.env' (e.g. discordToken)
    let token = env::var("DISCORD_TOKEN").expect("Token was not found in .env");

    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT | GatewayIntents::GUILDS;

    // create the bot client
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler::new())
        .await
        .expect("Error during client creation");

    // run
    if let Err(e) = client.start().await {
        println!("A bug in the bot: {:?}", e);
    }
}