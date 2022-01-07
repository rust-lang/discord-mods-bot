use crate::{api, commands::Args, text::WELCOME_BILLBOARD, Error};
use serenity::{model::prelude::*, prelude::*};
use sqlx::postgres::PgPool;
use std::sync::Arc;
use tracing::info;

/// Write the welcome message to the welcome channel.  
pub async fn post_message(args: Arc<Args>) -> Result<(), Error> {
    use std::str::FromStr;

    if api::is_mod(args.clone()).await? {
        let channel_name = &args
            .params
            .get("channel")
            .ok_or("unable to retrieve channel param")?;

        let channel_id = ChannelId::from_str(channel_name)?;

        info!("Posting welcome message");
        let message = channel_id.say(&args.cx, WELCOME_BILLBOARD).await?;

        let message_id = message.id.0.to_string();
        let bot_id = message.author.id.to_string();
        let channel_id = channel_id.0.to_string();

        let mut transaction = args.db.begin().await?;

        let save_message =
            "insert into messages (name, message, channel) values ('welcome', $1, $2)
            on conflict (name) do update set message = $1, channel = $2";
        sqlx::query(save_message)
            .bind(message_id)
            .bind(channel_id)
            .execute(&mut transaction)
            .await?;

        let user_id = bot_id;

        let save_user = "insert into users (user_id, name) values ($1, 'me')
            on conflict (name) do update set user_id = $1, name = 'me'";
        sqlx::query(save_user)
            .bind(user_id)
            .execute(&mut transaction)
            .await?;

        transaction.commit().await?;

        let white_check_mark = ReactionType::from_str("✅")?;
        message.react(&args.cx, white_check_mark).await?;
    }
    Ok(())
}

pub async fn assign_talk_role(
    cx: &Context,
    reaction: &Reaction,
    db: Arc<PgPool>,
) -> Result<(), Error> {
    let channel = reaction.channel(cx).await?;
    let channel_id = ChannelId::from(&channel);
    let message = reaction.message(cx).await?;

    let mut transaction = db.begin().await?;

    let msg: Option<(i32, String, String, String)> =
        sqlx::query_as("select * from messages where name = 'welcome' limit 1")
            .fetch_optional(&mut transaction)
            .await?;

    let talk_role: Option<(i32, String, String)> =
        sqlx::query_as("select * from roles where name = 'talk' limit 1")
            .fetch_optional(&mut transaction)
            .await?;

    let me: Option<(i32, String, String)> =
        sqlx::query_as("select * from users where name = 'me' limit 1")
            .fetch_optional(&mut transaction)
            .await?;

    transaction.commit().await?;

    if let Some((_, _, cached_message_id, cached_channel_id)) = msg {
        if message.id.0.to_string() == cached_message_id
            && channel_id.0.to_string() == *cached_channel_id
        {
            if reaction.emoji == ReactionType::from('✅') {
                if let Some((_, role_id, _)) = talk_role {
                    if let Some(user_id) = reaction.user_id {
                        let guild = channel
                            .guild()
                            .ok_or("Unable to retrieve guild from channel")?;

                        let mut member = guild.guild_id.member(cx, user_id).await?;

                        use std::str::FromStr;
                        info!("Assigning talk role to {}", &member.user.id);
                        member
                            .add_role(&cx, RoleId::from(u64::from_str(&role_id)?))
                            .await?;

                        // Requires ManageMessage permission
                        if let Some((_, _, bot_id)) = me {
                            if user_id.to_string() != bot_id {
                                reaction.delete(cx).await?;
                            }
                        }
                    }
                }
            } else {
                reaction.delete(cx).await?;
            }
        }
    }
    Ok(())
}

pub async fn help(args: Arc<Args>) -> Result<(), Error> {
    let help_string = format!(
        "
Post the welcome message to `channel`
```
{command}
```
**Example:**
```
?CoC #welcome

```
will post the welcome message to the `channel` specified.  
",
        command = "?CoC {channel}"
    );

    api::send_reply(args.clone(), &help_string).await?;
    Ok(())
}
