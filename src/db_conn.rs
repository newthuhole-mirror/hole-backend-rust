use rocket_sync_db_pools::{database, diesel};

pub type Conn = diesel::SqliteConnection;

#[database("sqlite_v2")]
pub struct Db(Conn);

