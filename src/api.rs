use crate::{command_history::CommandHistory, commands::Args, db::DB, schema::roles, Error};
use diesel::prelude::*;
use serenity::{model::prelude::*, utils::parse_username};

/// Send a reply to the channel the message was received on.  
pub(crate) fn send_reply(args: &Args, message: &str) -> Result<(), Error> {
    if let Some(response_id) = response_exists(args) {
        info!("editing message: {:?}", response_id);
        args.msg
            .channel_id
            .edit_message(&args.cx, response_id, |msg| msg.content(message))?;
    } else {
        let command_id = args.msg.id;
        let response = args.msg.channel_id.say(&args.cx, message)?;

        let mut data = args.cx.data.write();
        let history = data.get_mut::<CommandHistory>().unwrap();
        history.insert(command_id, response.id);
    }

    Ok(())
}

/// Send a Discord reply message and truncate the message with a given truncation message if the
/// text is too long.
///
/// Only `text_body` is truncated. `text_end` will always be appended at the end. This is useful
/// for example for large code blocks. You will want to truncate the code block contents, but the
/// finalizing \`\`\` should always stay - that's what `text_end` is for.
pub(crate) fn reply_potentially_long_text(
    args: &Args,
    text_body: &str,
    text_end: &str,
    truncation_msg: &str,
) -> Result<(), Error> {
    let msg = if text_body.len() + text_end.len() > 2000 {
        // This is how long the text body may be at max to conform to Discord's limit
        let available_space = 2000 - text_end.len() - truncation_msg.len();

        let mut cut_off_point = available_space;
        while !text_body.is_char_boundary(cut_off_point) {
            cut_off_point -= 1;
        }

        format!(
            "{}{}{}",
            &text_body[..cut_off_point],
            text_end,
            truncation_msg
        )
    } else {
        format!("{}{}", text_body, text_end)
    };

    send_reply(args, &msg)
}

fn response_exists(args: &Args) -> Option<MessageId> {
    let data = args.cx.data.read();
    let history = data.get::<CommandHistory>().unwrap();
    history.get(&args.msg.id).cloned()
}

/// Determine if a member sending a message has the `Role`.  
pub(crate) fn has_role(args: &Args, role: &RoleId) -> Result<bool, Error> {
    Ok(args
        .msg
        .member
        .as_ref()
        .ok_or("Unable to fetch member")?
        .roles
        .contains(role))
}

fn check_permission(args: &Args, role: Option<String>) -> Result<bool, Error> {
    use std::str::FromStr;
    if let Some(role_id) = role {
        Ok(has_role(args, &RoleId::from(u64::from_str(&role_id)?))?)
    } else {
        Ok(false)
    }
}

/// Return whether or not the user is a mod.  
pub(crate) fn is_mod(args: &Args) -> Result<bool, Error> {
    let role = roles::table
        .filter(roles::name.eq("mod"))
        .first::<(i32, String, String)>(&DB.get()?)
        .optional()?;

    check_permission(args, role.map(|(_, role_id, _)| role_id))
}

pub(crate) fn is_wg_and_teams(args: &Args) -> Result<bool, Error> {
    let role = roles::table
        .filter(roles::name.eq("wg_and_teams"))
        .first::<(i32, String, String)>(&DB.get()?)
        .optional()?;

    check_permission(args, role.map(|(_, role_id, _)| role_id))
}

/// Set slow mode for a channel.  
///
/// A `seconds` value of 0 will disable slowmode
pub(crate) fn slow_mode(args: Args) -> Result<(), Error> {
    use std::str::FromStr;

    if is_mod(&args)? {
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
        ChannelId::from_str(channel_name)?.edit(&args.cx, |c| c.slow_mode_rate(*seconds))?;
    }
    Ok(())
}

pub(crate) fn slow_mode_help(args: Args) -> Result<(), Error> {
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
    send_reply(&args, &help_string)?;
    Ok(())
}

/// Kick a user from the guild.  
///
/// Requires the kick members permission
pub(crate) fn kick(args: Args) -> Result<(), Error> {
    if is_mod(&args)? {
        let user_id = parse_username(
            &args
                .params
                .get("user")
                .ok_or("unable to retrieve user param")?,
        )
        .ok_or("unable to retrieve user id")?;

        if let Some(guild) = args.msg.guild(&args.cx) {
            info!("Kicking user from guild");
            guild.read().kick(&args.cx, UserId::from(user_id))?
        }
    }
    Ok(())
}

pub(crate) fn kick_help(args: Args) -> Result<(), Error> {
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
    send_reply(&args, &help_string)?;
    Ok(())
}
