#[macro_use]
extern crate rocket;

#[macro_use]
extern crate diesel;

mod api;
mod db_conn;
mod models;
mod random_hasher;
mod schema;

use db_conn::init_pool;
use random_hasher::RandomHasher;

#[launch]
fn rocket() -> _ {
    load_env();
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
                api::systemlog::get_systemlog,
                api::operation::delete,
            ],
        )
        .register("/_api", catchers![api::catch_401_error])
        .manage(RandomHasher::get_random_one())
        .manage(init_pool())
}

fn load_env() {
    match dotenv::dotenv() {
        Ok(path) => eprintln!("Configuration read from {}", path.display()),
        Err(ref e) if e.not_found() => eprintln!("Warning: no .env was found"),
        e => e.map(|_| ()).unwrap(),
    }
}
