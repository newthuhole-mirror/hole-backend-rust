use crate::api::{CurrentUser, JsonAPI, PolicyError::*, UGC};
use crate::db_conn::Db;
use crate::libs::diesel_logger::LoggingConnection;
use crate::models::*;
use crate::rds_conn::RdsConn;
use crate::rds_models::*;
use crate::schema;
use chrono::offset::Local;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use rocket::form::Form;
use rocket::serde::json::json;

#[derive(FromForm)]
pub struct DeleteInput {
    #[field(name = "type")]
    id_type: String,
    id: i32,
    note: String,
}

#[post("/delete", data = "<di>")]
pub async fn delete(di: Form<DeleteInput>, user: CurrentUser, db: Db, rconn: RdsConn) -> JsonAPI {
    let (author_hash, p) = match di.id_type.as_str() {
        "cid" => {
            let mut c = Comment::get(&db, di.id).await?;
            c.soft_delete(&user, &db).await?;
            let mut p = Post::get(&db, &rconn, c.post_id).await?;
            update!(
                p,
                posts,
                &db,
                { n_comments, add -1 },
                { hot_score, add -1 }
            );

            p.refresh_cache(&rconn, false).await;
            p.clear_comments_cache(&rconn).await;

            (c.author_hash.clone(), p)
        }
        "pid" => {
            let mut p = Post::get(&db, &rconn, di.id).await?;
            p.soft_delete(&user, &db).await?;

            // 如果是删除，需要也从0号缓存队列中去掉
            p.refresh_cache(&rconn, true).await;

            (p.author_hash.clone(), p)
        }
        _ => Err(NotAllowed)?,
    };

    if user.is_admin && !user.namehash.eq(&author_hash) {
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
            BannedUsers::add(&rconn, &author_hash).await?;
            DangerousUser::add(&rconn, &author_hash).await?;
        }
    }

    code0!()
}

#[derive(FromForm)]
pub struct ReportInput {
    pid: i32,
    #[field(validate = len(0..1000))]
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
        target: format!(
            "#{} {}",
            ri.pid,
            if ri.reason.starts_with("评论区") {
                "评论区"
            } else {
                ""
            }
        ),
        detail: ri.reason.clone(),
        time: Local::now(),
    }
    .create(&rconn)
    .await?;

    code0!()
}

#[derive(FromForm)]
pub struct BlockInput {
    #[field(name = "type")]
    content_type: String,
    id: i32,
}

#[post("/block", data = "<bi>")]
pub async fn block(bi: Form<BlockInput>, user: CurrentUser, db: Db, rconn: RdsConn) -> JsonAPI {
    let mut blk = BlockedUsers::init(user.id.ok_or_else(|| NotAllowed)?, &rconn);

    let nh_to_block = match bi.content_type.as_str() {
        "post" => Post::get(&db, &rconn, bi.id).await?.author_hash,
        "comment" => Comment::get(&db, bi.id).await?.author_hash,
        _ => Err(NotAllowed)?,
    };

    if nh_to_block.eq(&user.namehash) {
        Err(NotAllowed)?;
    }

    blk.add(&nh_to_block).await?;
    let curr = BlockCounter::count_incr(&rconn, &nh_to_block).await?;

    if curr >= BLOCK_THRESHOLD || user.is_admin {
        DangerousUser::add(&rconn, &nh_to_block).await?;
    }

    Ok(json!({
        "code": 0,
        "data": {
            "curr": curr,
            "threshold": BLOCK_THRESHOLD,
        },
    }))
}

#[derive(FromForm)]
pub struct TitleInput {
    #[field(validate = len(1..31))]
    title: String,
}

#[post("/title", data = "<ti>")]
pub async fn set_title(ti: Form<TitleInput>, user: CurrentUser, rconn: RdsConn) -> JsonAPI {
    if CustomTitle::set(&rconn, &user.namehash, &ti.title).await? {
        code0!()
    } else {
        Err(TitleUsed)?
    }
}
