use crate::api::{APIError, CurrentUser, PolicyError::*, API, UGC};
use crate::db_conn::Db;
use crate::models::*;
use crate::rds_conn::RdsConn;
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
pub async fn delete(
    di: Form<DeleteInput>,
    user: CurrentUser,
    db: Db,
    rconn: RdsConn,
) -> API<Value> {
    let mut p: Post;
    match di.id_type.as_str() {
        "cid" => {
            let mut c = Comment::get(&db, di.id).await?;
            c.soft_delete(&user, &db).await?;
            p = Post::get(&db, &rconn, c.post_id).await?;
            p.change_n_comments(&db, -1).await?;
            p.change_hot_score(&db, -1).await?;

            p.clear_comments_cache(&rconn).await;
        }
        "pid" => {
            p = Post::get(&db, &rconn, di.id).await?;
            p.soft_delete(&user, &db).await?;
        }
        _ => return Err(APIError::PcError(NotAllowed)),
    }

    p.refresh_cache(&rconn, false).await;

    Ok(json!({
        "code": 0
    }))
}
