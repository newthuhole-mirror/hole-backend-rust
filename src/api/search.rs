use crate::api::post::ps2outputs;
use crate::api::{CurrentUser, JsonAPI};
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
) -> JsonAPI {
    let page_size = 25;
    let start = (page - 1) * page_size;

    let kws = keywords.split(" ").filter(|x| !x.is_empty()).collect::<Vec<&str>>();
    let ps = if kws.is_empty() {
        vec![]
    } else {
        Post::search(
            &db,
            search_mode,
            keywords.to_string(),
            start.into(),
            page_size.into(),
        )
        .await?
    };
    let mark_kws = if search_mode == 1 {kws} else {vec![]};
    let ps_data = ps2outputs(&ps, &user, &db, &rconn).await;
    Ok(json!({
        "data": ps_data,
        "count": ps_data.len(),
        "code": 0
    }))
}
