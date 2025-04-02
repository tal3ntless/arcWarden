use serde::{Serialize, Deserialize};
use std::fs;
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use rand::Rng;
use serenity::prelude::*;
use serenity::model::user::User;

const DATA_DIRECTORY: &str = "./data";
const PACT_COOLDOWN: u64 = 86400;

#[derive(Serialize, Deserialize)]
pub struct UserData {
    pub balance: f64,
    pub last_pact: u64,
}

impl Default for UserData {
    fn default() -> Self {
        UserData {
            balance: 0.0,
            last_pact: 0,
        }
    }
}

pub fn ensure_data_directory() -> io::Result<()> {
    if !Path::new(DATA_DIRECTORY).exists() {
        fs::create_dir_all(DATA_DIRECTORY)?;
    }
    Ok(())
}

pub fn is_user_bound(user_id: &str) -> bool {
    let file_path = format!("{}/{}.json", DATA_DIRECTORY, user_id);
    Path::new(&file_path).exists()
}

pub fn load_user_data(user_id: &str) -> UserData {
    let file_path = format!("{}/{}.json", DATA_DIRECTORY, user_id);
    if Path::new(&file_path).exists() {
        if let Ok(data_str) = fs::read_to_string(&file_path) {
            if let Ok(data) = serde_json::from_str::<UserData>(&data_str) {
                return data;
            }
        }
    }
    UserData::default()
}

pub fn save_user_data(user_id: &str, data: &UserData) -> io::Result<()> {
    let file_path = format!("{}/{}.json", DATA_DIRECTORY, user_id);
    let json_data = serde_json::to_string(data).unwrap_or_default();
    fs::write(file_path, json_data)?;
    Ok(())
}

fn current_unix_time() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

pub fn perform_pact(user_id: &str, stake: f64) -> String {
    let mut user_data = load_user_data(user_id);
    let now = current_unix_time();

    if now.saturating_sub(user_data.last_pact) < PACT_COOLDOWN {
        let remaining = PACT_COOLDOWN - (now - user_data.last_pact);
        let hours = remaining / 3600;
        let minutes = (remaining % 3600) / 60;
        let seconds = remaining % 60;
        return format!(
            "🛑 Command already used. try again in {} h {} m {} s 🕔",
            hours, minutes, seconds
        );
    }

    if stake > user_data.balance {
        return format!(
            "🛑 Your stake ({:.2}) 🪙 exceeds your available balance ({:.2}) 🪙",
            stake, user_data.balance
        );
    }

    user_data.last_pact = now;

    let mut rng = rand::thread_rng();
    let roll: f64 = rng.gen_range(0.0..1.0);
    if roll < 0.45 {
        let bonus_percentage = rng.gen_range(7..=32);
        let bonus_amount = stake * (bonus_percentage as f64 / 100.0);
        user_data.balance += bonus_amount;
        let _ = save_user_data(user_id, &user_data);
        format!(
            "✅ Pact successful! bonus: +{}% (≈{:.2})🪙 New balance: {:.2} 🪙",
            bonus_percentage, bonus_amount, user_data.balance
        )
    } else {
        let penalty_percentage = rng.gen_range(7..=32);
        let penalty_amount = stake * (penalty_percentage as f64 / 100.0);
        user_data.balance -= penalty_amount;
        let _ = save_user_data(user_id, &user_data);
        format!(
            "🛑 Pact failed – {}% (≈{:.2}) 🪙 lost. New balance: {:.2} 🪙",
            penalty_percentage, penalty_amount, user_data.balance
        )
    }
}

pub fn bind_user(user_id: &str) -> String {
    let file_path = format!("{}/{}.json", DATA_DIRECTORY, user_id);
    if Path::new(&file_path).exists() {
        let _ = load_user_data(user_id).balance;
        return "🛑 Account already bound.".to_string();
    }
    let user_data = UserData::default();
    match save_user_data(user_id, &user_data) {
        Ok(_) => "✅ Account bound successfully.".to_string(),
        Err(e) => format!("🛑 Error binding account: {:?}", e),
    }
}

pub fn get_balance(user_id: &str) -> String {
    let balance = load_user_data(user_id).balance;
    format!("ℹ️ Your balance: {:.2} 🪙", balance)
}

pub fn pay(from_user: &str, to_user: &str, amount: f64) -> String {
    if from_user == to_user {
        return "🛑 You cannot pay yourself.".to_string();
    }
    if amount <= 0.0 {
        return "🛑 The amount must be greater than zero.".to_string();
    }
    if !is_user_bound(from_user) {
        return "🛑 Your account is not bound. Please use /bind first.".to_string();
    }
    if !is_user_bound(to_user) {
        return "🛑 Recipient account is not bound.".to_string();
    }
    let mut sender_data = load_user_data(from_user);
    if amount > sender_data.balance {
        return format!(
            "🛑 Insufficient funds: your balance is {:.2}",
            sender_data.balance
        );
    }
    let mut recipient_data = load_user_data(to_user);

    sender_data.balance -= amount;
    recipient_data.balance += amount;

    if let Err(e) = save_user_data(from_user, &sender_data) {
        return format!("🛑 Failed to update sender data: {:?}", e);
    }
    if let Err(e) = save_user_data(to_user, &recipient_data) {
        sender_data.balance += amount;
        let _ = save_user_data(from_user, &sender_data);
        return format!("🛑 Failed to update recipient data: {:?}", e);
    }

    format!(
        "✅ Successfully transferred {:.2} 🪙 to {}",
        amount, to_user,
    )
}

pub async fn perform_pact_dm(ctx: &Context, user: &User, stake: f64) -> Result<(), serenity::Error> {
    let result = perform_pact(&user.id.to_string(), stake);
    if let Ok(dm_channel) = user.create_dm_channel(&ctx.http).await {
        let _ = dm_channel.say(&ctx.http, result).await;
    }
    Ok(())
}

pub async fn bind_user_dm(ctx: &Context, user: &User) -> Result<(), serenity::Error> {
    let result = bind_user(&user.id.to_string());
    if let Ok(dm_channel) = user.create_dm_channel(&ctx.http).await {
        let _ = dm_channel.say(&ctx.http, result).await;
    }
    Ok(())
}

pub async fn get_balance_dm(ctx: &Context, user: &User) -> Result<(), serenity::Error> {
    let result = get_balance(&user.id.to_string());
    if let Ok(dm_channel) = user.create_dm_channel(&ctx.http).await {
        let _ = dm_channel.say(&ctx.http, result).await;
    }
    Ok(())
}

pub async fn pay_dm(ctx: &Context, sender: &User, recipient: &User, amount: f64) -> Result<(), serenity::Error> {
    let result = pay(&sender.id.to_string(), &recipient.id.to_string(), amount);

    if let Ok(dm_channel) = sender.create_dm_channel(&ctx.http).await {
        let _ = dm_channel.say(&ctx.http, result.clone()).await;
    }

    if result.starts_with("✅") {
        if let Ok(dm_channel) = recipient.create_dm_channel(&ctx.http).await {
            let dm_message = format!(
                "💸 You have received {:.2} 🪙 from {}",
                amount,
                sender.mention()
            );
            let _ = dm_channel.say(&ctx.http, dm_message).await;
        }
    }

    Ok(())
}