use crate::commands::Result;
use diesel::prelude::*;

pub(crate) fn database_connection() -> Result<PgConnection> {
    Ok(PgConnection::establish(&std::env::var("DATABASE_URL")?)?)
}

embed_migrations!("migrations");

pub(crate) fn run_migrations() -> Result<()> {
    embedded_migrations::run_with_output(&database_connection()?, &mut std::io::sink())?;

    Ok(())
}
