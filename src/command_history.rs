use crate::{
    commands::{Commands, PREFIX},
    Error, HOUR,
};
use indexmap::IndexMap;
use reqwest::Client as HttpClient;
use serenity::{model::prelude::*, prelude::*, utils::CustomMessage};
use sqlx::postgres::PgPool;
use std::{sync::Arc, time::Duration};
use tracing::info;

const MESSAGE_AGE_MAX: Duration = Duration::from_secs(HOUR);

pub struct CommandHistory;

impl TypeMapKey for CommandHistory {
    type Value = IndexMap<MessageId, MessageId>;
}

pub async fn replay_message(
    cx: Context,
    ev: MessageUpdateEvent,
    cmds: &Commands,
    http: Arc<HttpClient>,
    db: Arc<PgPool>,
) -> Result<(), Error> {
    let age = ev.timestamp.and_then(|create| {
        ev.edited_timestamp
            .and_then(|edit| edit.signed_duration_since(create).to_std().ok())
    });

    if age.is_some() && age.unwrap() < MESSAGE_AGE_MAX {
        let mut msg = CustomMessage::new();
        msg.id(ev.id)
            .channel_id(ev.channel_id)
            .content(ev.content.unwrap_or_default());

        let msg = msg.build();

        if msg.content.starts_with(PREFIX) {
            info!(
                "sending edited message - {:?} {:?}",
                msg.content, msg.author
            );
            cmds.execute(cx, msg, http, db).await;
        }
    }

    Ok(())
}

pub async fn clear_command_history(cx: &Context) -> Result<(), Error> {
    let mut data = cx.data.write().await;
    let history = data.get_mut::<CommandHistory>().unwrap();

    // always keep the last command in history
    if history.len() > 0 {
        info!("Clearing command history");
        history.drain(..history.len() - 1);
    }
    Ok(())
}
