use rocket_sync_db_pools::{database, diesel};

pub type Conn = diesel::pg::PgConnection;

#[database("pg_v2")]
pub struct Db(Conn);

