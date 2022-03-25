use crate::api::{APIError, CurrentUser, PolicyError::*, API, UGC};
use crate::db_conn::Db;
use crate::models::*;
use crate::rds_conn::RdsConn;
use crate::rds_models::*;
use chrono::offset::Local;
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
    let mut c: Comment;
    let author_hash: &str;
    match di.id_type.as_str() {
        "cid" => {
            c = Comment::get(&db, di.id).await?;
            c.soft_delete(&user, &db).await?;
            p = Post::get(&db, &rconn, c.post_id).await?;
            p.change_n_comments(&db, -1).await?;
            p.change_hot_score(&db, -1).await?;

            p.refresh_cache(&rconn, false).await;
            p.clear_comments_cache(&rconn).await;

            author_hash = &c.author_hash;
        }
        "pid" => {
            p = Post::get(&db, &rconn, di.id).await?;
            p.soft_delete(&user, &db).await?;
            // 如果是删除，需要也从0号缓存队列中去掉
            p.refresh_cache(&rconn, true).await;

            author_hash = &p.author_hash;
        }
        _ => return Err(APIError::PcError(NotAllowed)),
    }

    if user.is_admin && author_hash != user.namehash {
        Systemlog {
            user_hash: user.namehash,
            action_type: LogType::AdminDelete,
            target: format!("#{}, {}={}", p.id, di.id_type, di.id),
            detail: di.note.clone(),
            time: Local::now(),
        }
        .create(&rconn)
        .await?;
    }

    Ok(json!({
        "code": 0
    }))
}
