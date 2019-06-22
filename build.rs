use diesel::*;
use std::{env, io};

fn main() {
    let connection =
        SqliteConnection::establish("tags.db").expect("Unable to establish connection to database");

    let migrations_dir = diesel_migrations::find_migrations_directory().unwrap();
    println!("cargo:rerun-if-changed={}", migrations_dir.display());
    diesel_migrations::run_pending_migrations_in_directory(
        &connection,
        &migrations_dir,
        &mut io::sink(),
    )
    .unwrap();
}
