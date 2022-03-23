#[macro_use]
extern crate rocket;

#[macro_use]
extern crate diesel;

#[macro_use]
extern crate log;

mod api;
mod db_conn;
mod libs;
mod models;
mod random_hasher;
mod rds_conn;
mod rds_models;
mod cache;
mod schema;

use db_conn::Db;
use random_hasher::RandomHasher;
use rds_conn::init_rds_client;

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    load_env();
    env_logger::init();
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
        .manage(init_rds_client().await)
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
