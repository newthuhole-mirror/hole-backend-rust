use crate::api::{ApiError, CurrentUser, JsonApi, PolicyError::*, Ugc};
use crate::cache::*;
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
pub async fn delete(di: Form<DeleteInput>, user: CurrentUser, db: Db, rconn: RdsConn) -> JsonApi {
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

            // 有评论：清空主楼而非删除
            if p.author_hash == user.namehash && p.n_comments > 0 {
                update! {
                    p,
                    posts,
                    &db,
                    { content, to "[洞主已删除]" }
                }
            } else {
                p.soft_delete(&user, &db).await?;
            }

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
        }
    }

    code0!()
}

#[derive(FromForm)]
pub struct ReportInput {
    pid: i32,
    #[field(validate = len(0..1000))]
    reason: String,
    should_hide: Option<u8>,
}

#[post("/report", data = "<ri>")]
pub async fn report(ri: Form<ReportInput>, user: CurrentUser, db: Db, rconn: RdsConn) -> JsonApi {
    // 临时用户不允许举报
    user.id.ok_or(NotAllowed)?;

    // 被拉黑10次不允许举报
    (BlockCounter::get_count(&rconn, &user.namehash)
        .await?
        .unwrap_or(0)
        < 10)
        .then(|| ())
        .ok_or(NotAllowed)?;

    (!ri.reason.is_empty()).then(|| ()).ok_or(NoReason)?;

    let mut p = Post::get(&db, &rconn, ri.pid).await?;
    if ri.should_hide.is_some() {
        update!(p, posts, &db, { is_reported, to true });
        p.refresh_cache(&rconn, false).await;
    }

    Systemlog {
        user_hash: user.namehash.to_string(),
        action_type: LogType::Report,
        target: format!("#{}", ri.pid),
        detail: ri.reason.clone(),
        time: Local::now(),
    }
    .create(&rconn)
    .await?;

    // 自动发布一条洞
    let p = Post::create(
        &db,
        NewPost {
            content: format!("[系统自动代发]\n我举报了 #{}\n理由: {}", &p.id, &ri.reason),
            cw: "举报".to_string(),
            author_hash: user.namehash.to_string(),
            author_title: String::default(),
            is_tmp: false,
            n_attentions: 1,
            allow_search: true,
            room_id: 42,
        },
    )
    .await?;
    Attention::init(&user.namehash, &rconn).add(p.id).await?;
    p.refresh_cache(&rconn, true).await;

    code0!()
}

#[derive(FromForm)]
pub struct BlockInput {
    #[field(name = "type")]
    content_type: String,
    id: i32,
}

#[post("/block", data = "<bi>")]
pub async fn block(bi: Form<BlockInput>, user: CurrentUser, db: Db, rconn: RdsConn) -> JsonApi {
    user.id.ok_or(NotAllowed)?;

    let mut blk = BlockedUsers::init(user.id.ok_or(NotAllowed)?, &rconn);

    let pid;
    let nh_to_block = match bi.content_type.as_str() {
        "post" => {
            let p = Post::get(&db, &rconn, bi.id).await?;
            pid = p.id;
            p.author_hash
        }
        "comment" => {
            let c = Comment::get(&db, bi.id).await?;
            pid = c.post_id;
            c.author_hash
        }
        _ => return Err(ApiError::Pc(NotAllowed)),
    };

    if nh_to_block.eq(&user.namehash) {
        Err(NotAllowed)?;
    }

    let curr = if blk.add(&nh_to_block).await? > 0 {
        BlockCounter::count_incr(&rconn, &nh_to_block).await?
    } else {
        114514
    };

    BlockDictCache::init(&user.namehash, pid, &rconn)
        .clear()
        .await?;

    Ok(json!({
        "code": 0,
        "data": {
            "curr": curr,
        },
    }))
}

#[derive(FromForm)]
pub struct TitleInput {
    #[field(validate = len(1..31))]
    title: String,
}

#[post("/title", data = "<ti>")]
pub async fn set_title(ti: Form<TitleInput>, user: CurrentUser, rconn: RdsConn) -> JsonApi {
    if CustomTitle::set(&rconn, &user.namehash, &ti.title).await? {
        code0!()
    } else {
        Err(TitleUsed)?
    }
}

#[derive(FromForm)]
pub struct AutoBlockInput {
    rank: u8,
}

#[post("/auto_block", data = "<ai>")]
pub async fn set_auto_block(
    ai: Form<AutoBlockInput>,
    user: CurrentUser,
    rconn: RdsConn,
) -> JsonApi {
    AutoBlockRank::set(&rconn, &user.namehash, ai.rank).await?;
    code0!()
}
