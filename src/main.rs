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
mod cors;
mod db_conn;
mod libs;
#[cfg(feature = "mastlogin")]
mod login;
mod models;
mod random_hasher;
mod rds_conn;
mod rds_models;
mod schema;

use db_conn::{establish_connection, Conn, Db};
use diesel::Connection;
use random_hasher::RandomHasher;
use rds_conn::{init_rds_client, RdsConn};
use rds_models::clear_outdate_redis_data;
use rocket::tokio;
use rocket::tokio::time::{sleep, Duration};
use std::env;

embed_migrations!("migrations/postgres");

#[rocket::main]
async fn main() {
    load_env();
    if env::args().any(|arg| arg.eq("--init-database")) {
        init_database();
        return;
    }
    env_logger::init();
    let rmc = init_rds_client().await;
    let mut rconn = RdsConn(rmc.clone());
    let mut c_start = establish_connection();
    models::User::clear_non_admin_users(&mut c_start, &mut rconn).await;
    clear_outdate_redis_data(&mut rconn).await;
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(3 * 60 * 60)).await;
            models::Post::annealing(&mut c_start, &mut rconn).await;
        }
    });

    let rconn = RdsConn(rmc.clone());
    tokio::spawn(async move {
        loop {
            for room_id in (0..5).map(Some).chain([None, Some(42)]) {
                cache::PostListCache::init(room_id, 3, &rconn).clear().await;
            }
            sleep(Duration::from_secs(5 * 60)).await;
        }
    });

    let _ = rocket::build()
        .mount(
            "/_api/v1",
            routes![
                api::comment::get_comment,
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
                api::operation::report,
                api::operation::block,
                api::operation::set_auto_block,
                api::vote::vote,
                cors::options_handler,
            ],
        )
        .mount(
            "/_api/v2",
            routes![
                api::attention::set_notification,
                api::reaction::reaction,
                api::comment::add_comment,
                api::operation::set_title,
                api::upload::local_upload,
                cors::options_handler,
            ],
        )
        .mount(
            "/_login",
            [
                #[cfg(feature = "mastlogin")]
                routes![
                    login::cs_login,
                    login::cs_auth,
                    login::gh_login,
                    login::gh_auth
                ],
                routes![],
            ]
            .concat(),
        )
        .register(
            "/_api",
            catchers![
                api::catch_401_error,
                api::catch_403_error,
                api::catch_404_error
            ],
        )
        .manage(RandomHasher::get_random_one())
        .manage(rmc)
        .attach(Db::fairing())
        .attach(cors::Cors {
            whitelist: env::var("FRONTEND_WHITELIST")
                .unwrap_or_default()
                .split(',')
                .map(|s| s.to_string())
                .collect::<Vec<String>>(),
        })
        .launch()
        .await;
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
