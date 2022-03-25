#[macro_use]
extern crate rocket;

#[macro_use]
extern crate diesel;

#[macro_use]
extern crate diesel_migrations;

#[macro_use]
extern crate log;

mod api;
mod cache;
mod db_conn;
mod libs;
mod models;
mod random_hasher;
mod rds_conn;
mod rds_models;
mod schema;

use db_conn::{establish_connection, Conn, Db};
use diesel::Connection;
use random_hasher::RandomHasher;
use rds_conn::init_rds_client;
use std::env;
use tokio::time::{interval, Duration};

embed_migrations!("migrations/postgres");

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    load_env();
    if env::args().any(|arg| arg.eq("--init-database")) {
        init_database();
        return Ok(());
    }
    env_logger::init();
    let rmc = init_rds_client().await;
    let rconn = rds_conn::RdsConn(rmc.clone());
    tokio::spawn(async move {
        let mut itv = interval(Duration::from_secs(4 * 60 * 60));
        loop {
            itv.tick().await;
            models::Post::annealing(establish_connection(), &rconn).await;
        }
    });
    rocket::build()
        .mount(
            "/_api/v1",
            routes![
                api::comment::get_comment,
                api::comment::add_comment,
                api::post::get_list,
                api::post::get_one,
                api::post::publish_post,
                api::post::edit_cw,
                api::post::get_multi,
                api::search::search,
                api::attention::attention_post,
                api::attention::get_attention,
                api::systemlog::get_systemlog,
                api::operation::delete,
            ],
        )
        .register("/_api", catchers![api::catch_401_error])
        .manage(RandomHasher::get_random_one())
        .manage(rmc)
        .attach(Db::fairing())
        .launch()
        .await
}

fn load_env() {
    match dotenv::dotenv() {
        Ok(path) => eprintln!("Configuration read from {}", path.display()),
        Err(ref e) if e.not_found() => eprintln!("Warning: no .env was found"),
        e => e.map(|_| ()).unwrap(),
    }
}

fn init_database() {
    let database_url = env::var("DATABASE_URL").unwrap();
    let conn = Conn::establish(&database_url).unwrap();
    embedded_migrations::run(&conn).unwrap();
}
