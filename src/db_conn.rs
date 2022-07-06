use diesel::Connection;
use rocket_sync_db_pools::{database, diesel};
use std::env;

pub type Conn = diesel::pg::PgConnection;

#[database("pg_v2")]
pub struct Db(Conn);

// get sync connection, only for annealing
pub fn establish_connection() -> Conn {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    Conn::establish(&database_url)
        .unwrap_or_else(|_| panic!("Error connecting to {}", database_url))
}
