use crate::api::{APIError, CurrentUser, JsonAPI, PolicyError::*, UGC};
use crate::db_conn::Db;
use crate::models::*;
use crate::rds_conn::RdsConn;
use crate::rds_models::*;
use chrono::offset::Local;
use rocket::form::Form;
use rocket::serde::json::json;
use crate::libs::diesel_logger::LoggingConnection;
use crate::schema;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

#[derive(FromForm)]
pub struct DeleteInput {
    #[field(name = "type")]
    id_type: String,
    id: i32,
    note: String,
}

#[post("/delete", data = "<di>")]
pub async fn delete(di: Form<DeleteInput>, user: CurrentUser, db: Db, rconn: RdsConn) -> JsonAPI {
    let mut p: Post;
    let mut c: Comment;
    let author_hash: &str;
    match di.id_type.as_str() {
        "cid" => {
            c = Comment::get(&db, di.id).await?;
            c.soft_delete(&user, &db).await?;
            p = Post::get(&db, &rconn, c.post_id).await?;
            update!(
                p,
                posts,
                &db,
                { n_comments, add -1 },
                { hot_score, add -1 }
            );

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

    if user.is_admin && !user.namehash.eq(author_hash) {
        Systemlog {
            user_hash: user.namehash.clone(),
            action_type: LogType::AdminDelete,
            target: format!("#{}, {}={}", p.id, di.id_type, di.id),
            detail: di.note.clone(),
            time: Local::now(),
        }
        .create(&rconn)
        .await?;

        if di.note.starts_with("!ban ") {
            Systemlog {
                user_hash: user.namehash.clone(),
                action_type: LogType::Ban,
                target: look!(author_hash),
                detail: di.note.clone(),
                time: Local::now(),
            }
            .create(&rconn)
            .await?;
            BannedUsers::add(&rconn, author_hash).await?;
        }
    }

    Ok(json!({
        "code": 0
    }))
}

#[derive(FromForm)]
pub struct ReportInput {
    pid: i32,
    reason: String,
}

#[post("/report", data = "<ri>")]
pub async fn report(ri: Form<ReportInput>, user: CurrentUser, db: Db, rconn: RdsConn) -> JsonAPI {
    // 临时用户不允许举报
    user.id.ok_or_else(|| NotAllowed)?;

    let mut p = Post::get(&db, &rconn, ri.pid).await?;
    update!(p, posts, &db, { is_reported, to true });
    p.refresh_cache(&rconn, false).await;
    Systemlog {
        user_hash: user.namehash,
        action_type: LogType::Report,
        target: format!("#{} {}", ri.pid, if ri.reason.starts_with("评论区") { "评论区" } else {""}),
        detail: ri.reason.clone(),
        time: Local::now(),
    }.create(&rconn)
    .await?;
    Ok(json!({
        "code": 0
    }))
}
