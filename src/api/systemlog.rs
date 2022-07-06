use crate::api::{CurrentUser, JsonApi};
use crate::random_hasher::RandomHasher;
use crate::rds_conn::RdsConn;
use crate::rds_models::Systemlog;
use rocket::serde::json::{json, Value};
use rocket::State;

#[get("/systemlog")]
pub async fn get_systemlog(user: CurrentUser, rh: &State<RandomHasher>, rconn: RdsConn) -> JsonApi {
    let logs = Systemlog::get_list(&rconn, 50).await?;

    Ok(json!({
        "tmp_token": rh.get_tmp_token(),
        "salt": look!(rh.salt),
        "start_time": rh.start_time.timestamp(),
        "custom_title": user.custom_title,
        "data": logs.into_iter().map(|log|
            json!({
                "type": log.action_type,
                "user": look!(log.user_hash),
                "timestamp": log.time.timestamp(),
                "detail": format!("{}\n{}", &log.target, &log.detail),
            })
        ).collect::<Vec<Value>>(),
    }))
}
