use crate::{api, commands::Args, text::ban_message, Error, HOUR};
use serenity::{model::prelude::*, prelude::*, utils::parse_username};
use sqlx::{
    postgres::PgPool,
    types::chrono::{DateTime, Utc},
};
use std::{
    sync::Arc,
    time::{Duration, SystemTime},
};
use tracing::info;

pub async fn save_ban(
    user_id: String,
    guild_id: String,
    hours: u64,
    db: Arc<PgPool>,
) -> Result<(), Error> {
    info!("Recording ban for user {}", &user_id);
    sqlx::query(
        "insert into bans(user_id, guild_id, start_time, end_time) values ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(guild_id)
    .bind(DateTime::<Utc>::from(SystemTime::now()))
    .bind(DateTime::<Utc>::from(
        SystemTime::now()
            .checked_add(Duration::new(hours * HOUR, 0))
            .ok_or("out of range Duration for ban end_time")?,
    ))
    .execute(&*db)
    .await?;

    Ok(())
}

pub async fn save_unban(user_id: String, guild_id: String, db: Arc<PgPool>) -> Result<(), Error> {
    info!("Recording unban for user {}", &user_id);
    sqlx::query(
        "update bans set unbanned = true where user_id = $1 and guild_id = $2 and unbanned = false",
    )
    .bind(user_id)
    .bind(guild_id)
    .execute(&*db)
    .await?;

    Ok(())
}

pub async fn unban_users(cx: &Context, db: Arc<PgPool>) -> Result<(), Error> {
    use std::str::FromStr;

    let to_unban: Vec<(i32, String, String, bool, DateTime<Utc>, DateTime<Utc>)> =
        sqlx::query_as("select * from bans where unbanned = false and end_time < $1")
            .bind(DateTime::<Utc>::from(SystemTime::now()))
            .fetch_all(&*db)
            .await?;

    for row in &to_unban {
        let guild_id = GuildId::from(u64::from_str(&row.2)?);
        info!("Unbanning user {}", &row.1);
        guild_id.unban(&cx, u64::from_str(&row.1)?).await?;
    }

    Ok(())
}

/// Temporarily ban an user from the guild.  
///
/// Requires the ban members permission
pub async fn temp_ban(args: Arc<Args>) -> Result<(), Error> {
    let user_id = parse_username(
        &args
            .params
            .get("user")
            .ok_or("unable to retrieve user param")?,
    )
    .ok_or("unable to retrieve user id")?;

    use std::str::FromStr;

    let hours = match u64::from_str(
        args.params
            .get("hours")
            .ok_or("unable to retrieve hours param")?,
    ) {
        Ok(hours) => hours,
        Err(e) => {
            api::send_reply(&args, &format!("{}", e))?;
            return Err(Box::new(e));
        }
    };

    let reason = args
        .params
        .get("reason")
        .ok_or("unable to retrieve reason param")?;

    if let Some(guild) = args.msg.guild(&args.cx) {
        info!("Banning user from guild");
        let user = UserId::from(user_id);

        user.create_dm_channel(&args.cx)
            .await?
            .say(&args.cx, ban_message(reason, hours))
            .await?;

        guild.ban(&args.cx, &user, 7).await?;

        save_ban(
            format!("{}", user_id),
            format!("{}", guild.id),
            hours,
            args.db.clone(),
        )
        .await?;
    }
    Ok(())
}

pub async fn help(args: Arc<Args>) -> Result<(), Error> {
    let hours = 24;
    let reason = "violating the code of conduct";

    let help_string = format!(
        "
Ban a user for a temporary amount of time
```
{command}
```
**Example:**
```
?ban @someuser {hours} {reason}
```
will ban a user for {hours} hours and send them the following message:
```
{user_message}
```
",
        command = "?ban {user} {hours} reason...",
        user_message = ban_message(reason, hours),
        hours = hours,
        reason = reason,
    );

    api::send_reply(args.clone(), &help_string).await?;
    Ok(())
}
