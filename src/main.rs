use serenity::async_trait;
use serenity::model::gateway::Ready;
use serenity::model::prelude::*;
use serenity::prelude::*;
use dotenvy::dotenv;
use rand::Rng;
use std::env;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        println!("Bot {} is up and running!", ready.user.name);
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.author.bot {
            return; // Ignore messages from bots
        }

        match msg.content.as_str() {
            "!ping" => {
                if let Err(e) = msg.channel_id.say(&ctx.http, "Pong!").await {
                    println!("Error sending the message: {:?}", e);
                }
            }
            "!roll" => {
                let roll = rand::thread_rng().gen_range(1..=6);
                let response = format!("🎲 You rolled a **{}**!", roll);
                if let Err(e) = msg.channel_id.say(&ctx.http, response).await {
                    println!("Error sending the message: {:?}", e);
                }
            }
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let token = env::var("DISCORD_TOKEN").expect("Token was not found in .env");

    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("Error during client creation");

    if let Err(e) = client.start().await {
        println!("A bug in the bot: {:?}", e);
    }
}