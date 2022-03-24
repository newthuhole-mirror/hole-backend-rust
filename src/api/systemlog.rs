use crate::api::{CurrentUser, API};
use crate::db_conn::Db;
use crate::random_hasher::RandomHasher;
use rocket::serde::json::{json, Value};
use rocket::State;

#[get("/systemlog")]
pub async fn get_systemlog(user: CurrentUser, rh: &State<RandomHasher>, db: Db) -> API<Value> {
    Ok(json!({
        "tmp_token": rh.get_tmp_token(),
        "salt": look!(rh.salt),
        "start_time": rh.start_time.timestamp(),
        "custom_title": user.custom_title,
        "data": [],
    }))
}
