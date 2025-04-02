use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::all::CommandDataOptionValue;
use serenity::model::id::{ChannelId};

use crate::config;
use crate::commands;
use crate::balance;

pub struct Handler {
    pub config: config::Config,
}

#[serenity::async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name.to_lowercase());
        if let Some(guild_id) = std::env::var("GUILD_ID")
            .ok()
            .and_then(|id| id.parse::<u64>().ok())
            .map(GuildId::from)
        {
            commands::clear_guild_commands(&ctx, guild_id).await;
            commands::clear_global_commands(&ctx).await;
            commands::register_commands(&ctx, guild_id).await;
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            let Some(guild_id) = command.guild_id else {
                if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                    let _ = dm_channel.say(&ctx.http, "🛑 This command can only be used in a server!").await;
                }
                return;
            };

            match command.data.name.as_str() {
                "ticket" => {
                    if command.channel_id != ChannelId::new(self.config.allowed_channel_id) {
                        if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                            let _ = dm_channel.say(&ctx.http, "🛑 This command can only be used in the designated channel.").await;
                        }
                        return;
                    }
                    match commands::create_ticket_channel(
                        &ctx,
                        guild_id,
                        command.user.id,
                        ChannelId::new(self.config.ticket_category_id[0]),
                        &config::get_mod_roles(&self.config)
                    ).await {
                        Ok(channel_id) => {
                            if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                                let _ = dm_channel.say(&ctx.http, format!("✅ Ticket created: <#{}>", channel_id)).await;
                            }
                        },
                        Err(e) => {
                            eprintln!("🛑 Error creating ticket: {:?}", e);
                            if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                                let _ = dm_channel.say(&ctx.http, "🛑 Could not create ticket, please try again later.").await;
                            }
                        }
                    }
                },
                "ticketclose" => {
                    let mod_roles = config::get_mod_roles(&self.config);
                    if let Some(member) = command.member.clone() {
                        if !member.roles.iter().any(|role| mod_roles.contains(role)) {
                            if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                                let _ = dm_channel.say(&ctx.http, "🛑 You do not have permission to close this ticket.").await;
                            }
                            return;
                        }
                    } else {
                        if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                            let _ = dm_channel.say(&ctx.http, "🛑 Unable to verify your permissions.").await;
                        }
                        return;
                    }
                    let channel = command.channel_id.to_channel(&ctx.http).await.unwrap();
                    if let Some(guild_channel) = channel.guild() {
                        // Проверка, что канал не является запрещённым (например, канал настроек)
                        let ignored_channels: std::collections::HashSet<u64> = vec![self.config.allowed_channel_id].into_iter().collect();
                        if ignored_channels.contains(&u64::from(guild_channel.id)) {
                            if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                                let _ = dm_channel.say(&ctx.http, "🛑 This channel cannot be closed.").await;
                            }
                            return;
                        }
                        if let Some(parent) = guild_channel.parent_id {
                            if u64::from(parent) != self.config.allowed_ticket_cat_id {
                                if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                                    let _ = dm_channel.say(&ctx.http, "🛑 This channel is not in the allowed ticket category.").await;
                                }
                                return;
                            }
                        } else {
                            if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                                let _ = dm_channel.say(&ctx.http, "🛑 This channel does not belong to any category, cannot close.").await;
                            }
                            return;
                        }
                        println!("closing ticket in channel: {}", guild_channel.name.to_lowercase());
                        match commands::close_ticket(&ctx, command.channel_id).await {
                            Ok(_) => {
                                if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                                    let _ = dm_channel.say(&ctx.http, "✅ Ticket has been closed.").await;
                                }
                            },
                            Err(e) => {
                                eprintln!("🛑 Error closing ticket: {:?}", e);
                                if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                                    let _ = dm_channel.say(&ctx.http, "🛑 Failed to close the ticket, please try again later.").await;
                                }
                            }
                        }
                    }
                },
                "pact" => {
                    if !balance::is_user_bound(&command.user.id.to_string()) {
                        if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                            let _ = dm_channel.say(&ctx.http, "🛑 Your account is not bound. please use /bind first.").await;
                        }
                        return;
                    }
                    let stake = if let Some(option) = command.data.options.get(0) {
                        match &option.value {
                            CommandDataOptionValue::Number(n) => *n,
                            _ => 0.0,
                        }
                    } else {
                        0.0
                    };
                    if stake <= 0.0 {
                        if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                            let _ = dm_channel.say(&ctx.http, "🛑 Please specify a valid stake (> 0) 🪙").await;
                        }
                        return;
                    }
                    let user_data = balance::load_user_data(&command.user.id.to_string());
                    if stake > user_data.balance {
                        if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                            let _ = dm_channel.say(&ctx.http, format!("🛑 Your stake ({:.2}) exceeds your balance ({:.2}) 🪙", stake, user_data.balance)).await;
                        }
                        return;
                    }
                    let result = balance::perform_pact(&command.user.id.to_string(), stake);
                    if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                        let _ = dm_channel.say(&ctx.http, result).await;
                    }
                },
                "bind" => {
                    let result = balance::bind_user(&command.user.id.to_string());
                    if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                        let _ = dm_channel.say(&ctx.http, result).await;
                    }
                },
                "balance" => {
                    if !balance::is_user_bound(&command.user.id.to_string()) {
                        if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                            let _ = dm_channel.say(&ctx.http, "🛑 Your account is not bound. please use /bind first.").await;
                        }
                        return;
                    }
                    let result = balance::get_balance(&command.user.id.to_string());
                    if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                        let _ = dm_channel.say(&ctx.http, result).await;
                    }
                },
                "pay" => {
                    let recipient = if let Some(option) = command.data.options.iter().find(|opt| opt.name == "recipient") {
                        match &option.value {
                            CommandDataOptionValue::User(user) => *user,
                            _ => {
                                if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                                    let _ = dm_channel.say(&ctx.http, "🛑 Invalid recipient value.").await;
                                }
                                return;
                            }
                        }
                    } else {
                        if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                            let _ = dm_channel.say(&ctx.http, "🛑 Recipient not provided.").await;
                        }
                        return;
                    };

                    let amount = if let Some(option) = command.data.options.iter().find(|opt| opt.name == "amount") {
                        match &option.value {
                            CommandDataOptionValue::Number(n) => *n,
                            _ => {
                                if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                                    let _ = dm_channel.say(&ctx.http, "🛑 Invalid amount value.").await;
                                }
                                return;
                            }
                        }
                    } else {
                        if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                            let _ = dm_channel.say(&ctx.http, "🛑 Amount not provided.").await;
                        }
                        return;
                    };

                    let sender_id = command.user.id.to_string();
                    let recipient_id = recipient.to_string();

                    let result_msg = balance::pay(&sender_id, &recipient_id, amount);

                    if result_msg.starts_with("✅") {
                        let dm_message_sender = format!(
                            "✅ Successfully transferred {:.2} 🪙 to {}",
                            amount,
                            recipient.mention(),
                        );

                        if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                            let _ = dm_channel.say(&ctx.http, dm_message_sender).await;
                        }

                        if let Ok(recipient_dm) = recipient.create_dm_channel(&ctx.http).await {
                            let dm_message_recipient = format!(
                                "💸 You have received {:.2} 🪙 from {}",
                                amount,
                                command.user.mention()
                            );
                            let _ = recipient_dm.say(&ctx.http, dm_message_recipient).await;
                        }
                    } else {
                        if let Ok(dm_channel) = command.user.create_dm_channel(&ctx.http).await {
                            let _ = dm_channel.say(&ctx.http, result_msg).await;
                        }
                    }
                },
                _ => {}
            }
        }
    }
}