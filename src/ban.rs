use crate::{commands::Result, db::DB, schema::bans};
use diesel::prelude::*;
use serenity::{model::prelude::*, prelude::*};
use std::{
    sync::atomic::{AtomicBool, Ordering},
    thread::sleep,
    time::{Duration, SystemTime},
};

static UNBAN_THREAD_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub(crate) fn save_ban(user_id: String, guild_id: String) -> Result<()> {
    info!("Recording ban for user {}", &user_id);
    let conn = DB.get()?;
    diesel::insert_into(bans::table)
        .values((
            bans::user_id.eq(user_id),
            bans::guild_id.eq(guild_id),
            bans::start_time.eq(SystemTime::now()),
            bans::end_time.eq(SystemTime::now()
                .checked_add(Duration::new(1 * 60, 0))
                .ok_or("out of range Duration for ban end_time")?),
        ))
        .execute(&conn)?;

    Ok(())
}

pub(crate) fn save_unban(user_id: String, guild_id: String) -> Result<()> {
    info!("Recording unban for user {}", &user_id);
    let conn = DB.get()?;
    diesel::update(bans::table)
        .filter(
            bans::user_id
                .eq(user_id)
                .and(bans::guild_id.eq(guild_id).and(bans::unbanned.eq(false))),
        )
        .set(bans::unbanned.eq(true))
        .execute(&conn)?;
    Ok(())
}

type SendSyncError = Box<dyn std::error::Error + Send + Sync>;

pub(crate) fn start_unban_thread(cx: Context) {
    use std::str::FromStr;
    if !UNBAN_THREAD_INITIALIZED.load(Ordering::SeqCst) {
        UNBAN_THREAD_INITIALIZED.store(true, Ordering::SeqCst);
        std::thread::spawn(move || -> std::result::Result<(), SendSyncError> {
            loop {
                sleep(Duration::new(20, 0));
                let conn = DB.get()?;
                let to_unban = bans::table
                    .filter(
                        bans::unbanned
                        .eq(false)
                        .and(bans::end_time.le(SystemTime::now())),
                    )
                    .load::<(i32, String, String, bool, SystemTime, SystemTime)>(&conn)?;

                for row in &to_unban {
                    let guild_id = GuildId::from(u64::from_str(&row.2)?);
                    info!("Unbanning user {}", &row.1);
                    guild_id.unban(&cx, u64::from_str(&row.1)?)?;
                }
            }
        });
    }
}