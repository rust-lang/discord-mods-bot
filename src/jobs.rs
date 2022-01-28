use crate::{ban::unban_users, command_history::clear_command_history, Error, HOUR};
use serenity::client::Context;
use sqlx::postgres::PgPool;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::time::{sleep, Duration};

static JOBS_THREAD_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub fn start_jobs(cx: Context, db: Arc<PgPool>) {
    if !JOBS_THREAD_INITIALIZED.load(Ordering::SeqCst) {
        JOBS_THREAD_INITIALIZED.store(true, Ordering::SeqCst);
        tokio::spawn(async move {
            loop {
                unban_users(&cx, db.clone()).await?;
                clear_command_history(&cx).await?;

                sleep(Duration::new(HOUR, 0)).await;
            }

            Ok::<_, Error>(())
        });
    }
}
