use crate::{
    command_history::CommandHistory,
    commands::{Args, Auth},
    Error,
};
use indexmap::IndexMap;
use serenity::{model::prelude::*, utils::parse_username};
use std::sync::Arc;
use tracing::info;

/// Send a reply to the channel the message was received on.  
pub async fn send_reply(args: Arc<Args>, message: &str) -> Result<(), Error> {
    if let Some(response_id) = response_exists(args.clone()).await {
        info!("editing message: {:?}", response_id);
        args.msg
            .channel_id
            .edit_message(&args.clone().cx, response_id, |msg| msg.content(message))
            .await?;
    } else {
        let command_id = args.msg.id;
        let response = args.clone().msg.channel_id.say(&args.cx, message).await?;

        let mut data = args.cx.data.write().await;
        let history = data.get_mut::<CommandHistory>().unwrap();
        history.insert(command_id, response.id);
    }

    Ok(())
}

async fn response_exists(args: Arc<Args>) -> Option<MessageId> {
    let data = args.cx.data.read().await;
    let history = data.get::<CommandHistory>().unwrap();
    history.get(&args.msg.id).cloned()
}

/// Determine if a member sending a message has the `Role`.  
pub fn has_role(args: Arc<Args>, role: &RoleId) -> Result<bool, Error> {
    Ok(args
        .msg
        .member
        .as_ref()
        .ok_or("Unable to fetch member")?
        .roles
        .contains(role))
}

fn check_permission(args: Arc<Args>, role: Option<String>) -> Result<bool, Error> {
    use std::str::FromStr;
    if let Some(role_id) = role {
        Ok(has_role(
            args.clone(),
            &RoleId::from(u64::from_str(&role_id)?),
        )?)
    } else {
        Ok(false)
    }
}

/// Return whether or not the user is a mod.  
pub async fn is_mod(args: Arc<Args>) -> Result<bool, Error> {
    let role: Option<(i32, String, String)> =
        sqlx::query_as("select * from roles where name = 'mod'")
            .fetch_optional(&*args.db)
            .await?;

    check_permission(args.clone(), role.map(|(_, role_id, _)| role_id))
}

pub async fn is_wg_and_teams(args: Arc<Args>) -> Result<bool, Error> {
    let role: Option<(i32, String, String)> =
        sqlx::query_as("select * from roles where name = 'wg_and_teams'")
            .fetch_optional(&*args.db)
            .await?;

    check_permission(args.clone(), role.map(|(_, role_id, _)| role_id))
}

pub async fn main_menu(
    args: Arc<Args>,
    commands: &IndexMap<&'static str, (&'static str, &'static Auth)>,
) -> String {
    use futures::stream::{self, StreamExt};

    let mut menu = format!("Commands:\n");

    menu = stream::iter(commands)
        .fold(menu, |mut menu, (base_cmd, (description, auth))| {
            let args_clone = args.clone();
            async move {
                if let Ok(true) = auth.call(args_clone).await {
                    menu += &format!("\t{cmd:<12}{desc}\n", cmd = base_cmd, desc = description);
                }
                menu
            }
        })
        .await;

    menu += &format!("\t{help:<12}This menu\n", help = "?help");
    menu += "\nType ?help command for more info on a command.";
    menu += "\n\nAdditional Info:\n";
    menu += "\tYou can edit your message to the bot and the bot will edit its response.";
    menu
}

/// Set slow mode for a channel.  
///
/// A `seconds` value of 0 will disable slowmode
pub async fn slow_mode(args: Arc<Args>) -> Result<(), Error> {
    use std::str::FromStr;

    if is_mod(args.clone()).await? {
        let seconds = &args
            .params
            .get("seconds")
            .ok_or("unable to retrieve seconds param")?
            .parse::<u64>()?;

        let channel_name = &args
            .params
            .get("channel")
            .ok_or("unable to retrieve channel param")?;

        info!("Applying slowmode to channel {}", &channel_name);
        ChannelId::from_str(channel_name)?
            .edit(&args.cx, |c| c.rate_limit_per_user(*seconds))
            .await?;
    }
    Ok(())
}

pub async fn slow_mode_help(args: Arc<Args>) -> Result<(), Error> {
    let help_string = "
Set slowmode on a channel
```
?slowmode {channel} {seconds}
```
**Example:**
```
?slowmode #bot-usage 10
```
will set slowmode on the `#bot-usage` channel with a delay of 10 seconds.  

**Disable slowmode:**
```
?slowmode #bot-usage 0
```
will disable slowmode on the `#bot-usage` channel.";
    send_reply(args.clone(), &help_string).await?;
    Ok(())
}

/// Kick a user from the guild.  
///
/// Requires the kick members permission
pub async fn kick(args: Arc<Args>) -> Result<(), Error> {
    if is_mod(args.clone()).await? {
        let user_id = parse_username(
            &args
                .params
                .get("user")
                .ok_or("unable to retrieve user param")?,
        )
        .ok_or("unable to retrieve user id")?;

        if let Some(guild) = args.msg.guild(&args.cx) {
            info!("Kicking user from guild");
            guild.kick(&args.cx, UserId::from(user_id)).await?
        }
    }
    Ok(())
}

pub async fn kick_help(args: Arc<Args>) -> Result<(), Error> {
    let help_string = "
Kick a user from the guild
```
?kick {user}
```
**Example:**
```
?kick @someuser
```
will kick a user from the guild.";
    send_reply(args.clone(), &help_string).await?;
    Ok(())
}
