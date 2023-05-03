use crate::api::{CurrentUser, JsonApi};
use crate::cache::cached_user_count;
use crate::db_conn::Db;
use crate::random_hasher::RandomHasher;
use crate::rds_conn::RdsConn;
use crate::rds_models::{get_admin_list, get_candidate_list, Systemlog};
use rocket::serde::json::{json, Value};
use rocket::State;

#[get("/systemlog")]
pub async fn get_systemlog(
    user: CurrentUser,
    rh: &State<RandomHasher>,
    db: Db,
    mut rconn: RdsConn,
) -> JsonApi {
    let logs = Systemlog::get_list(&rconn, 50).await?;

    Ok(json!({
        "tmp_token": rh.get_tmp_token(),
        "salt": look!(rh.salt),
        "start_time": rh.start_time.timestamp(),
        "user_count": cached_user_count(&db, &mut rconn).await?,
        "custom_title": user.custom_title,
        "admin_list": get_admin_list(&rconn).await?,
        "candidate_list": get_candidate_list(&rconn).await?,
        "data": logs.into_iter().map(|log|
            json!({
                "type": log.action_type,
                "user": log.user_hash,
                "timestamp": log.time.timestamp(),
                "detail": format!("{}\n{}", &log.target, &log.detail),
            })
        ).collect::<Vec<Value>>(),
    }))
}
