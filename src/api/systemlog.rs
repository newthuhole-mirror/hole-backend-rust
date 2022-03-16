use crate::api::{CurrentUser, API};
use crate::random_hasher::RandomHasher;
use rocket::serde::json::{json, Value};
use rocket::State;
use crate::db_conn::DbConn;

#[get("/systemlog")]
pub fn get_systemlog(user: CurrentUser, rh: &State<RandomHasher>, conn: DbConn) -> API<Value> {
    Ok(json!({
        "tmp_token": rh.get_tmp_token(),
        "salt": look!(rh.salt),
        "start_time": rh.start_time.timestamp(),
        "custom_title": user.custom_title,
        "data": [],
    }))
}
