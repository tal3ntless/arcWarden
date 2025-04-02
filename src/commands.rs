use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::builder::{CreateChannel, CreateCommand, CreateCommandOption};
use serenity::model::channel::ChannelType;
use serenity::model::Permissions;
use serenity::all::{CommandOptionType};
use std::sync::atomic::{AtomicU32, Ordering};

static LAST_TICKET_ID: AtomicU32 = AtomicU32::new(10000);

fn generate_ticket_id() -> u32 {
    LAST_TICKET_ID.fetch_add(1, Ordering::Relaxed)
}

pub async fn create_ticket_channel(
    ctx: &Context,
    guild_id: GuildId,
    initiator: UserId,
    ticket_category_id: ChannelId,
    mod_role_ids: &[RoleId],
) -> Result<ChannelId, serenity::Error> {
    let ticket_id = generate_ticket_id();
    let channel_name = format!("ticket-{}", ticket_id);
    println!("✅ Creating ticket: ID #{} for user: {}", ticket_id, initiator);

    let mut overwrites = Vec::new();
    overwrites.push(PermissionOverwrite {
        kind: PermissionOverwriteType::Role(RoleId::new(u64::from(guild_id))),
        allow: Permissions::empty(),
        deny: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
    });
    overwrites.push(PermissionOverwrite {
        kind: PermissionOverwriteType::Member(initiator),
        allow: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
        deny: Permissions::empty(),
    });
    for mod_role in mod_role_ids {
        overwrites.push(PermissionOverwrite {
            kind: PermissionOverwriteType::Role(*mod_role),
            allow: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
            deny: Permissions::empty(),
        });
    }
    overwrites.push(PermissionOverwrite {
        kind: PermissionOverwriteType::Member(ctx.cache.current_user().id),
        allow: Permissions::VIEW_CHANNEL | Permissions::SEND_MESSAGES,
        deny: Permissions::empty(),
    });

    let create_channel = CreateChannel::new(channel_name)
        .kind(ChannelType::Text)
        .category(ticket_category_id)
        .permissions(overwrites);
    let new_channel = guild_id.create_channel(&ctx.http, create_channel).await?;
    println!("✅ Ticket created: {} (channel ID: {})", new_channel.name, new_channel.id);
    Ok(new_channel.id)
}

pub async fn close_ticket(ctx: &Context, channel_id: ChannelId) -> Result<(), serenity::Error> {
    channel_id.delete(&ctx.http).await?;
    Ok(())
}

pub async fn clear_guild_commands(ctx: &Context, guild_id: GuildId) {
    if let Err(e) = guild_id.set_commands(&ctx.http, Vec::new()).await {
        eprintln!("🛑 Failed to clear guild commands: {}", e);
    } else {
        println!("📌 Cleared guild commands.");
    }
}

pub async fn clear_global_commands(ctx: &Context) {
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

pub async fn register_commands(ctx: &Context, guild_id: GuildId) {
    let commands = vec![
        CreateCommand::new("ticket").description("📍 Creates a new ticket"),
        CreateCommand::new("ticketclose").description("📁 Closes the current ticket"),
        CreateCommand::new("pact").description("🪙 Enter the Twilight Financial Pact")
            .add_option(
                CreateCommandOption::new(CommandOptionType::Number, "stake", "Stake amount")
                    .required(true)
            ),
        CreateCommand::new("bind").description("🖇️ Bind id to database"),
        CreateCommand::new("balance").description("💼 Show your current balance"),
        CreateCommand::new("pay").description("💸 Transfer coins to another user")
            .add_option(
                CreateCommandOption::new(CommandOptionType::User, "recipient", "User to pay")
                    .required(true)
            )
            .add_option(
                CreateCommandOption::new(CommandOptionType::Number, "amount", "Amount to transfer")
                    .required(true)
            ),
    ];
    if let Err(e) = guild_id.set_commands(&ctx.http, commands).await {
        eprintln!("🛑 Failed to register commands: {}", e);
    }
}