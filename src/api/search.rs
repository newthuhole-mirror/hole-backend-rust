use crate::api::post::ps2outputs;
use crate::api::{CurrentUser, JsonApi, PolicyError::*};
use crate::db_conn::Db;
use crate::models::*;
use crate::rds_conn::RdsConn;
use rocket::serde::json::json;

#[get("/search?<search_mode>&<page>&<keywords>")]
pub async fn search(
    keywords: String,
    search_mode: u8,
    page: i32,
    user: CurrentUser,
    db: Db,
    rconn: RdsConn,
) -> JsonApi {
    user.id.ok_or(YouAreTmp)?;

    let page_size = 25;
    let start = (page - 1) * page_size;

    let ps = if !keywords.chars().any(|c| !c.eq(&' ')) {
        vec![]
    } else {
        Post::search(
            &db,
            &rconn,
            search_mode,
            keywords.to_string(),
            start.into(),
            page_size.into(),
        )
        .await?
    };
    let ps_data = ps2outputs(&ps, &user, &db, &rconn).await?;
    Ok(json!({
        "data": ps_data,
        "count": ps_data.len(),
        "code": 0
    }))
}
