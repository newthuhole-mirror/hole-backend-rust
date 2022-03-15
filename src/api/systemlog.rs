use crate::api::{CurrentUser, API};
use crate::random_hasher::RandomHasher;
use chrono::SubsecRound;
use rocket::serde::json::{json, Value};
use rocket::State;

#[get("/systemlog")]
pub fn get_systemlog(user: CurrentUser, rh: &State<RandomHasher>) -> API<Value> {
    Ok(json!({
        "tmp_token": rh.get_tmp_token(),
        "salt": look!(rh.salt),
        "start_time": rh.start_time.round_subsecs(0),
        "custom_title": user.custom_title,
    }))
}
