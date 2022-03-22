use crate::api::{APIError, CurrentUser, PolicyError::*, API, UGC};
use crate::db_conn::Db;
use crate::rds_conn::RdsConn;
use crate::models::*;
use rocket::form::Form;
use rocket::serde::json::{json, Value};

#[derive(FromForm)]
pub struct DeleteInput {
    #[field(name = "type")]
    id_type: String,
    id: i32,
    note: String,
}

#[post("/delete", data = "<di>")]
pub async fn delete(di: Form<DeleteInput>, user: CurrentUser, db: Db, rconn: RdsConn) -> API<Value> {
    match di.id_type.as_str() {
        "cid" => {
            let c = Comment::get(&db, di.id).await?;
            c.soft_delete(&user, &db).await?;
            let mut p = Post::get(&db, c.post_id).await?;
            p = p.change_n_comments(&db, -1).await?;
            p = p.change_hot_score(&db, -2).await?;
            p.refresh_cache(&rconn, false).await;
        }
        "pid" => {
            let p = Post::get(&db, di.id).await?;
            p.soft_delete(&user, &db).await?;
        }
        _ => return Err(APIError::PcError(NotAllowed)),
    }

    Ok(json!({
        "code": 0
    }))
}
