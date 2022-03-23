use crate::api::comment::{c2output, CommentOutput};
use crate::api::{APIError, CurrentUser, JsonAPI, PolicyError::*, UGC};
use crate::db_conn::Db;
use crate::models::*;
use crate::rds_conn::RdsConn;
use crate::rds_models::*;
use chrono::{offset::Utc, DateTime};
use rocket::form::Form;
use rocket::futures::future;
use rocket::serde::{json::json, Serialize};

#[derive(FromForm)]
pub struct PostInput {
    #[field(validate = len(1..4097))]
    text: String,
    #[field(validate = len(0..33))]
    cw: String,
    allow_search: Option<i8>,
    use_title: Option<i8>,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct PostOutput {
    pid: i32,
    text: String,
    cw: Option<String>,
    author_title: Option<String>,
    is_tmp: bool,
    n_attentions: i32,
    n_comments: i32,
    create_time: DateTime<Utc>,
    last_comment_time: DateTime<Utc>,
    allow_search: bool,
    is_reported: Option<bool>,
    comments: Option<Vec<CommentOutput>>,
    can_del: bool,
    attention: bool,
    // for old version frontend
    timestamp: i64,
    likenum: i32,
    reply: i32,
}

#[derive(FromForm)]
pub struct CwInput {
    pid: i32,
    #[field(validate = len(0..33))]
    cw: String,
}

async fn p2output(
    p: &Post,
    user: &CurrentUser,
    db: &Db,
    rconn: &RdsConn,
) -> PostOutput {
    PostOutput {
        pid: p.id,
        text: format!("{}{}", if p.is_tmp { "[tmp]\n" } else { "" }, p.content),
        cw: if p.cw.len() > 0 {
            Some(p.cw.to_string())
        } else {
            None
        },
        n_attentions: p.n_attentions,
        n_comments: p.n_comments,
        create_time: p.create_time,
        last_comment_time: p.last_comment_time,
        allow_search: p.allow_search,
        author_title: if p.author_title.len() > 0 {
            Some(p.author_title.to_string())
        } else {
            None
        },
        is_tmp: p.is_tmp,
        is_reported: if user.is_admin {
            Some(p.is_reported)
        } else {
            None
        },
        comments: if p.n_comments > 50 {
            None
        } else {
            // 单个洞还有查询评论的接口，这里挂了不用报错
            let pid = p.id;
            if let Some(cs) = Comment::gets_by_post_id(db, pid).await.ok() {
                Some(c2output(p, &cs, user))
            } else {
                None
            }
        },
        can_del: p.check_permission(user, "wd").is_ok(),
        attention: Attention::init(&user.namehash, &rconn)
            .has(p.id)
            .await
            .unwrap_or_default(),
        // for old version frontend
        timestamp: p.create_time.timestamp(),
        likenum: p.n_attentions,
        reply: p.n_comments,
    }
}

pub async fn ps2outputs(
    ps: &Vec<Post>,
    user: &CurrentUser,
    db: &Db,
    rconn: &RdsConn,
) -> Vec<PostOutput> {
    future::join_all(
        ps.iter()
            .map(|p| async { p2output(p, &user, &db, &rconn).await }),
    )
    .await
}

#[get("/getone?<pid>")]
pub async fn get_one(pid: i32, user: CurrentUser, db: Db, rconn: RdsConn) -> JsonAPI {
    // let p = Post::get(&db, pid).await?;
    let p = Post::get_with_cache(&db, &rconn, pid).await?;
    p.check_permission(&user, "ro")?;
    Ok(json!({
        "data": p2output(&p, &user,&db, &rconn).await,
        "code": 0,
    }))
}

#[get("/getlist?<p>&<order_mode>")]
pub async fn get_list(
    p: Option<u32>,
    order_mode: u8,
    user: CurrentUser,
    db: Db,
    rconn: RdsConn,
) -> JsonAPI {
    let page = p.unwrap_or(1);
    let page_size = 25;
    let start = (page - 1) * page_size;
    let ps = Post::gets_by_page(&db, order_mode, start.into(), page_size.into()).await?;
    let ps_data = ps2outputs(&ps, &user, &db, &rconn).await;
    Ok(json!({
        "data": ps_data,
        "count": ps_data.len(),
        "code": 0
    }))
}

#[post("/dopost", data = "<poi>")]
pub async fn publish_post(
    poi: Form<PostInput>,
    user: CurrentUser,
    db: Db,
    rconn: RdsConn,
) -> JsonAPI {
    let p = Post::create(
        &db,
        NewPost {
            content: poi.text.to_string(),
            cw: poi.cw.to_string(),
            author_hash: user.namehash.to_string(),
            author_title: "".to_string(),
            is_tmp: user.id.is_none(),
            n_attentions: 1,
            allow_search: poi.allow_search.is_some(),
        },
    )
    .await?;
    Attention::init(&user.namehash, &rconn).add(p.id).await?;
    Ok(json!({
        "code": 0
    }))
}

#[post("/editcw", data = "<cwi>")]
pub async fn edit_cw(cwi: Form<CwInput>, user: CurrentUser, db: Db) -> JsonAPI {
    let p = Post::get(&db, cwi.pid).await?;
    if !(user.is_admin || p.author_hash == user.namehash) {
        return Err(APIError::PcError(NotAllowed));
    }
    p.check_permission(&user, "w")?;
    _ = p.update_cw(&db, cwi.cw.to_string()).await?;
    Ok(json!({"code": 0}))
}

#[get("/getmulti?<pids>")]
pub async fn get_multi(pids: Vec<i32>, user: CurrentUser, db: Db, rconn: RdsConn) -> JsonAPI {
    let ps = Post::get_multi(&db, pids).await?;
    let ps_data = ps2outputs(&ps, &user, &db, &rconn).await;

    Ok(json!({
        "code": 0,
        "data": ps_data,
    }))
}
