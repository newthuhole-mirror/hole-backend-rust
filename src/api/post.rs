use crate::api::comment::{c2output, CommentOutput};
use crate::api::vote::get_poll_dict;
use crate::api::{Api, CurrentUser, JsonApi, PolicyError::*, Ugc};
use crate::cache::*;
use crate::db_conn::Db;
use crate::libs::diesel_logger::LoggingConnection;
use crate::models::*;
use crate::rds_conn::RdsConn;
use crate::rds_models::*;
use crate::schema;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use rocket::form::Form;
use rocket::futures::future::{self, OptionFuture};
use rocket::serde::{
    json::{json, Value},
    Serialize,
};

#[derive(FromForm)]
pub struct PostInput {
    #[field(validate = len(1..12289))]
    text: String,
    #[field(validate = len(0..97))]
    cw: String,
    allow_search: Option<i8>,
    use_title: Option<i8>,
    #[field(validate = len(0..97))]
    poll_options: Vec<String>,
    room_id: Option<i32>,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct PostOutput {
    pid: i32,
    room_id: i32,
    text: String,
    cw: Option<String>,
    author_title: Option<String>,
    is_tmp: bool,
    n_attentions: i32,
    n_comments: i32,
    create_time: i64,
    last_comment_time: i64,
    allow_search: bool,
    is_reported: Option<bool>,
    comments: Option<Vec<CommentOutput>>,
    can_del: bool,
    attention: bool,
    hot_score: Option<i32>,
    is_blocked: bool,
    //blocked_count: Option<i32>,
    poll: Option<Value>,
    // for old version frontend
    timestamp: i64,
    likenum: i32,
    reply: i32,
    blocked: bool,
}

#[derive(FromForm)]
pub struct CwInput {
    pid: i32,
    #[field(validate = len(0..97))]
    cw: String,
}

async fn p2output(p: &Post, user: &CurrentUser, db: &Db, rconn: &RdsConn) -> Api<PostOutput> {
    let comments: Option<Vec<Comment>> = if p.n_comments < 5 {
        Some(p.get_comments(db, rconn).await?)
    } else {
        None
    };
    let hash_list = comments
        .iter()
        .flatten()
        .map(|c| &c.author_hash)
        .chain(std::iter::once(&p.author_hash))
        .collect::<Vec<_>>();
    //dbg!(&hash_list);
    let cached_block_dict = BlockDictCache::init(&user.namehash, p.id, rconn)
        .get_or_create(user, &hash_list)
        .await?;
    let is_blocked = cached_block_dict[&p.author_hash];
    let can_view =
        user.is_admin || (!is_blocked && user.id.is_some() || user.namehash.eq(&p.author_hash));
    Ok(PostOutput {
        pid: p.id,
        room_id: p.room_id,
        text: can_view.then_some(p.content.clone()).unwrap_or_default(),
        cw: (!p.cw.is_empty()).then_some(p.cw.clone()),
        n_attentions: p.n_attentions,
        n_comments: p.n_comments,
        create_time: p.create_time.timestamp(),
        last_comment_time: p.last_comment_time.timestamp(),
        allow_search: p.allow_search,
        author_title: (!p.author_title.is_empty()).then_some(p.author_title.clone()),
        is_tmp: p.is_tmp,
        is_reported: user.is_admin.then_some(p.is_reported),
        comments: OptionFuture::from(
            comments.map(|cs| async move { c2output(p, &cs, user, &cached_block_dict).await }),
        )
        .await,
        can_del: p.check_permission(user, "wd").is_ok(),
        attention: Attention::init(&user.namehash, rconn).has(p.id).await?,
        hot_score: user.is_admin.then_some(p.hot_score),
        is_blocked,
        /*
        blocked_count: if user.is_admin {
            BlockCounter::get_count(rconn, &p.author_hash).await?
        } else {
            None
        },
        */
        poll: if can_view {
            get_poll_dict(p.id, rconn, &user.namehash).await
        } else {
            None
        },
        // for old version frontend
        timestamp: p.create_time.timestamp(),
        likenum: p.n_attentions,
        reply: p.n_comments,
        blocked: is_blocked,
    })
}

pub async fn ps2outputs(
    ps: &[Post],
    user: &CurrentUser,
    db: &Db,
    rconn: &RdsConn,
) -> Api<Vec<PostOutput>> {
    future::try_join_all(
        ps.iter()
            .map(|p| async { p2output(p, user, db, rconn).await }),
    )
    .await
}

#[get("/getone?<pid>")]
pub async fn get_one(pid: i32, user: CurrentUser, db: Db, rconn: RdsConn) -> JsonApi {
    let p = Post::get(&db, &rconn, pid).await?;
    p.check_permission(&user, "ro")?;
    Ok(json!({
        "data": p2output(&p, &user,&db, &rconn).await?,
        "code": 0,
    }))
}

#[get("/getlist?<p>&<order_mode>&<room_id>")]
pub async fn get_list(
    p: Option<u32>,
    order_mode: u8,
    room_id: Option<i32>,
    user: CurrentUser,
    db: Db,
    rconn: RdsConn,
) -> JsonApi {
    user.id.ok_or(YouAreTmp)?;
    let page = p.unwrap_or(1);
    let page_size = 25;
    let start = (page - 1) * page_size;
    let ps = Post::gets_by_page(
        &db,
        &rconn,
        room_id,
        order_mode,
        start.into(),
        page_size.into(),
    )
    .await?;
    let ps_data = ps2outputs(&ps, &user, &db, &rconn).await?;

    Ok(json!({
        "data": ps_data,
        "count": ps_data.len(),
        "custom_title": user.custom_title,
        "title_secret": user.title_secret,
        "is_admin": user.is_admin,
        "is_candidate": user.is_candidate,
        "auto_block_rank": user.auto_block_rank,
        "announcement": get_announcement(&rconn).await?,
        "code": 0
    }))
}

#[post("/dopost", data = "<poi>")]
pub async fn publish_post(
    poi: Form<PostInput>,
    user: CurrentUser,
    db: Db,
    rconn: RdsConn,
) -> JsonApi {
    let use_title = poi.use_title.is_some() || user.is_admin || user.is_candidate;

    let p = Post::create(
        &db,
        NewPost {
            content: poi.text.to_string(),
            cw: poi.cw.to_string(),
            author_hash: user.namehash.to_string(),
            author_title: if use_title {
                user.custom_title
            } else {
                "".to_owned()
            },
            is_tmp: user.id.is_none(),
            n_attentions: 1,
            allow_search: poi.allow_search.is_some(),
            room_id: poi.room_id.unwrap_or_default(),
        },
    )
    .await?;
    Attention::init(&user.namehash, &rconn).add(p.id).await?;
    p.refresh_cache(&rconn, true).await;

    if !poi.poll_options.is_empty() {
        PollOption::init(p.id, &rconn)
            .set_list(&poi.poll_options)
            .await?;
    }
    code0!()
}

#[post("/editcw", data = "<cwi>")]
pub async fn edit_cw(cwi: Form<CwInput>, user: CurrentUser, db: Db, rconn: RdsConn) -> JsonApi {
    let mut p = Post::get(&db, &rconn, cwi.pid).await?;
    p.check_permission(&user, "w")?;
    update!(p, posts, &db, { cw, to cwi.cw.to_string() });
    p.refresh_cache(&rconn, false).await;
    code0!()
}

#[get("/getmulti?<pids>")]
pub async fn get_multi(pids: Vec<i32>, user: CurrentUser, db: Db, rconn: RdsConn) -> JsonApi {
    user.id.ok_or(YouAreTmp)?;
    let ps = Post::get_multi(&db, &rconn, &pids).await?;
    let ps_data = ps2outputs(&ps, &user, &db, &rconn).await?;

    Ok(json!({
        "code": 0,
        "data": ps_data,
    }))
}
