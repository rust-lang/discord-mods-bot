#[macro_use]
extern crate diesel;

#[macro_use]
extern crate diesel_migrations;

mod api;
mod ban;
mod command_history;
mod commands;
mod crates;
mod db;
mod jobs;
mod playground;
mod schema;
mod state_machine;
mod tags;
mod text;
mod welcome;

pub type Error = Box<dyn std::error::Error + Send + Sync>;

pub const HOUR: u64 = 3600;

use crate::commands::{Command, Commands};
use indexmap::IndexMap;
use reqwest::Client as HttpClient;
use serde::Deserialize;
use serenity::{async_trait, model::prelude::*, prelude::*};
use sqlx::postgres::{PgPool, PgPoolOptions};
use std::sync::Arc;
use tracing::{error, info};

#[derive(Deserialize)]
struct Config {
    tags: bool,
    crates: bool,
    eval: bool,
    discord_token: String,
    mod_id: String,
    talk_id: String,
    wg_and_teams_id: Option<String>,
}

async fn upsert_role(
    name: &str,
    role_id: &str,
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
) -> Result<(), Error> {
    sqlx::query(
        "insert into roles(role, name) values ($1, $2)
                on conflict (name) do update set role = $1",
    )
    .bind(role_id)
    .bind(name)
    .execute(transaction)
    .await?;

    Ok(())
}

async fn init_data(config: &Config, pool: Arc<PgPool>) -> Result<(), Error> {
    info!("Loading data into database");

    let mut transaction = pool.begin().await?;

    upsert_role("mod", &config.mod_id, &mut transaction).await?;
    upsert_role("talk", &config.talk_id, &mut transaction).await?;

    if config.tags || config.crates {
        let wg_and_teams_role = config
            .wg_and_teams_id
            .as_ref()
            .ok_or(text::WG_AND_TEAMS_MISSING_ENV_VAR)?;
        upsert_role("wg_and_teams", &wg_and_teams_role, &mut transaction).await?;
    }

    transaction.commit().await?;

    Ok(())
}

async fn app() -> Result<(), Error> {
    let config = envy::from_env::<Config>()?;

    tracing_subscriber::fmt::init();

    info!("starting...");

    let pool = Arc::new(
        PgPoolOptions::new()
            .connect(&std::env::var("DATABASE_URL")?)
            .await?,
    );

    let _ = db::run_migrations()?;

    let _ = init_data(&config, pool.clone()).await?;

    let mut cmds = Commands::new();

    if config.tags {
        // Tags
        cmds.add(
            "?tags delete {key}",
            Command::new_with_auth(&tags::delete, &api::is_wg_and_teams),
        );
        cmds.add(
            "?tags create {key} value...",
            Command::new_with_auth(&tags::post, &api::is_wg_and_teams),
        );
        cmds.add(
            "?tags update {key} value...",
            Command::new_with_auth(&tags::update, &api::is_wg_and_teams),
        );
        cmds.add("?tag {key}", Command::new(&tags::get));
        cmds.add("?tags", Command::new(&tags::get_all));
        cmds.help("?tags", "A key value store", Command::new(&tags::help));
    }

    if config.crates {
        // crates.io
        cmds.add("?crate query...", Command::new(&crates::search));
        cmds.help(
            "?crate",
            "Lookup crates on crates.io",
            Command::new(&crates::help),
        );

        // docs.rs
        cmds.add("?docs query...", Command::new(&crates::doc_search));
        cmds.help(
            "?docs",
            "Lookup documentation",
            Command::new(&crates::doc_help),
        );
    }

    if config.eval {
        // rust playground
        cmds.add(
            "?play mode={} edition={} channel={} warn={} ```\ncode``` ...",
            Command::new(&playground::run),
        );
        cmds.add("?play code...", Command::new(&playground::err));
        cmds.help(
            "?play",
            "Compile and run rust code in a playground",
            Command::new(&|args| async { playground::help(args, "play").await }),
        );

        cmds.add(
            "?eval mode={} edition={} channel={} warn={} ```\ncode``` ...",
            Command::new(&playground::eval),
        );
        cmds.add(
            "?eval mode={} edition={} channel={} warn={} ```code``` ...",
            Command::new(&playground::eval),
        );
        cmds.add(
            "?eval mode={} edition={} channel={} warn={} `code` ...",
            Command::new(&playground::eval),
        );
        cmds.add("?eval code...", Command::new(&playground::eval_err));
        cmds.help(
            "?eval",
            "Evaluate a single rust expression",
            Command::new(&|args| async { playground::help(args, "eval").await }),
        );
    }

    // Slow mode.
    // 0 seconds disables slowmode
    cmds.add(
        "?slowmode {channel} {seconds}",
        Command::new_with_auth(&api::slow_mode, &api::is_mod),
    );
    cmds.help(
        "?slowmode",
        "Set slowmode on a channel",
        Command::new_with_auth(&api::slow_mode_help, &api::is_mod),
    );

    // Kick
    cmds.add(
        "?kick {user}",
        Command::new_with_auth(&api::kick, &api::is_mod),
    );
    cmds.help(
        "?kick",
        "Kick a user from the guild",
        Command::new_with_auth(&api::kick_help, &api::is_mod),
    );

    // Ban
    cmds.add(
        "?ban {user} {hours} reason...",
        Command::new_with_auth(&ban::temp_ban, &api::is_mod),
    );
    cmds.help(
        "?ban",
        "Temporarily ban a user from the guild",
        Command::new_with_auth(&ban::help, &api::is_mod),
    );

    // Post the welcome message to the welcome channel.
    cmds.add(
        "?CoC {channel}",
        Command::new_with_auth(&welcome::post_message, &api::is_mod),
    );
    cmds.help(
        "?CoC",
        "Post the code of conduct message to a channel",
        Command::new_with_auth(&welcome::help, &api::is_mod),
    );

    cmds.add("?help", Command::help());

    let mut client = Client::builder(&config.discord_token)
        .event_handler(Events {
            http: Arc::new(HttpClient::new()),
            db: pool.clone(),
            cmds: Arc::new(cmds),
        })
        .await?;

    client.start().await?;

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = app().await {
        error!("{}", e);
        std::process::exit(1);
    }
}

struct Events {
    http: Arc<HttpClient>,
    db: Arc<PgPool>,
    cmds: Arc<Commands>,
}

#[async_trait]
impl EventHandler for Events {
    async fn ready(&self, cx: Context, ready: Ready) {
        info!("{} connected to discord", ready.user.name);
        {
            let mut data = cx.data.write().await;
            data.insert::<command_history::CommandHistory>(IndexMap::new());
        }

        jobs::start_jobs(cx, self.db.clone());
    }

    async fn message(&self, cx: Context, message: Message) {
        self.cmds
            .execute(cx, message, self.http.clone(), self.db.clone())
            .await;
    }

    async fn message_update(
        &self,
        cx: Context,
        _: Option<Message>,
        _: Option<Message>,
        ev: MessageUpdateEvent,
    ) {
        if let Err(e) =
            command_history::replay_message(cx, ev, &self.cmds, self.http.clone(), self.db.clone())
                .await
        {
            error!("{}", e);
        }
    }

    async fn message_delete(
        &self,
        cx: Context,
        channel_id: ChannelId,
        message_id: MessageId,
        _guild_id: Option<GuildId>,
    ) {
        let mut data = cx.data.write().await;
        let history = data.get_mut::<command_history::CommandHistory>().unwrap();
        if let Some(response_id) = history.remove(&message_id) {
            info!("deleting message: {:?}", response_id);
            let _ = channel_id.delete_message(&cx, response_id).await;
        }
    }

    async fn reaction_add(&self, cx: Context, reaction: Reaction) {
        if let Err(e) = welcome::assign_talk_role(&cx, &reaction, self.db.clone()).await {
            error!("{}", e);
        }
    }

    async fn guild_ban_removal(&self, _cx: Context, guild_id: GuildId, user: User) {
        if let Err(e) = ban::save_unban(
            format!("{}", user.id),
            format!("{}", guild_id),
            self.db.clone(),
        )
        .await
        {
            error!("{}", e);
        }
    }
}
