#[macro_use]
extern crate rocket;

#[macro_use]
extern crate diesel;

mod random_hasher;
mod models;
mod schema;
mod api;

use random_hasher::RandomHasher;

#[launch]
fn rocket() -> _ {
    load_env();
    rocket::build()
        .mount("/_api", routes![
            api::post::get_list,
            api::post::get_one, 
            api::post::publish_post
        ])
        .register("/_api", catchers![
            api::catch_401_error
        ])
        .manage(RandomHasher::get_random_one())
}

fn load_env() {
    match dotenv::dotenv() {
        Ok(path) => eprintln!("Configuration read from {}", path.display()),
        Err(ref e) if e.not_found() => eprintln!("Warning: no .env was found"),
        e => e.map(|_| ()).unwrap(),
    }
}
