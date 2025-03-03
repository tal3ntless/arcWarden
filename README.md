# **arcWarden**

## 🌿 Overview
This project is an opensource discord bot in Rust, using Serenity and Songbird to work with voice channels.

## 🪐 Discord
[***Join the official discord***](https://discord.gg/ygfqd8Mtps)

## 📋 Features
✅ **Play music** via `/play <url>` 
✅ **Join to a voice channel** with `/join'`  
✅ **Disconnect from the channel** with `/quit`  
✅ **Dice roll** (`/roll`, 1 to 6)  
✅ **Automatic registration of commands** at bot startup  
✅ **High performance**

## ⬇️ Installation
1. **Clone the repository:**
    ```bash
    git clone https://github.com/tal3ntless/arcWarden.git
    ```  
2. **Create an `.env` file:**
    ```env
    DISCORD_TOKEN=your_discord_bot_token
    GUILD_ID=your_guild_id.
    ```  
3. **Build project:**
    ```bash
    cargo build --release
    ```  

## ☄️ Quick start
Run the bot:
```bash
cargo run --release
```

## 📎 Notes
- **API Versions**: this bot is built with Serenity 0.12.4 and Songbird 0.5. Future API changes may require adjustments.
- **Permissions**: ensure your discord bot has the necessary permissions to manage slash commands and join voice channels.
- **Resource Management**: the bot stops any current playback before starting a new track to help manage memory usage.
