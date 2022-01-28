use crate::Error;
use diesel::prelude::*;
use tracing::info;

pub fn run_migrations() -> Result<(), Error> {
    let conn = PgConnection::establish(&std::env::var("DATABASE_URL")?)?;

    diesel_migrations::embed_migrations!();

    info!("Running database migrations");
    let _ = embedded_migrations::run_with_output(&conn, &mut std::io::stdout())?;

    Ok(())
}
