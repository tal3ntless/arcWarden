use dotenvy::dotenv;
use std::env;
use std::thread;
use actix_web::rt::System;
use serenity::prelude::*;
use serenity::Client as DiscordClient;

mod config;
mod handler;
mod commands;
mod api;
mod balance;

use config::load_config;

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() {
    dotenv().ok();

    let config = load_config("config.json").expect("🛑 Failed to load config.json");

    if let Err(e) = balance::ensure_data_directory() {
        eprintln!("🛑 Error creating data directory: {:?}", e);
    }

    let token = env::var("DISCORD_TOKEN").expect("🛑 Missing DISCORD_TOKEN in .env");
    let intents = GatewayIntents::GUILDS;
    let handler = handler::Handler { config: config.clone() };

    let mut bot = DiscordClient::builder(&token, intents)
        .event_handler(handler)
        .await
        .expect("🛑 Error creating client");

    thread::spawn(|| {
        let sys = System::new();
        sys.block_on(api::start_api_server()).expect("🛑 API server failed");
    });

    if let Err(e) = bot.start().await {
        eprintln!("🛑 Client error: {:?}", e);
    }
}