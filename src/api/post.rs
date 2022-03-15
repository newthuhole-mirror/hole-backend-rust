use crate::api::comment::{c2output, CommentOutput};
use crate::api::{APIError, CurrentUser, PolicyError::*, API};
use crate::models::*;
use chrono::NaiveDateTime;
use diesel::SqliteConnection;
use rocket::form::Form;
use rocket::serde::{
    json::{json, Value},
    Serialize,
};

#[derive(FromForm)]
pub struct PostInput<'r> {
    #[field(validate = len(1..4097))]
    text: &'r str,
    #[field(validate = len(0..33))]
    cw: &'r str,
    allow_search: Option<i8>,
    use_title: Option<i8>,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct PostOutput {
    pid: i32,
    text: String,
    cw: Option<String>,
    author_title: String,
    n_likes: i32,
    n_comments: i32,
    create_time: NaiveDateTime,
    last_comment_time: NaiveDateTime,
    allow_search: bool,
    is_reported: Option<bool>,
    comments: Vec<CommentOutput>,
    can_del: bool,
    // for old version frontend
    timestamp: i64,
    custom_title: Option<String>,
}

fn p2output(p: &Post, user: &CurrentUser, conn: &SqliteConnection) -> PostOutput {
    PostOutput {
        pid: p.id,
        text: p.content.to_string(),

        cw: if p.cw.len() > 0 {
            Some(p.cw.to_string())
        } else {
            None
        },
        author_title: p.author_title.to_string(),
        n_likes: p.n_likes,
        n_comments: p.n_comments,
        create_time: p.create_time,
        last_comment_time: p.last_comment_time,
        allow_search: p.allow_search,
        is_reported: if user.is_admin {
            Some(p.is_reported)
        } else {
            None
        },
        comments: if p.n_comments > 50 {
            vec![]
        } else {
            // 单个洞还有查询评论的接口，这里挂了不用报错
            c2output(p, &p.get_comments(conn).unwrap_or(vec![]), user)
        },
        can_del: user.is_admin || p.author_hash == user.namehash,
        // for old version frontend
        timestamp: p.create_time.timestamp(),
        custom_title: if p.author_title.len() > 0 {
            Some(p.author_title.to_string())
        } else {
            None
        },
    }
}

#[get("/getone?<pid>")]
pub fn get_one(pid: i32, user: CurrentUser) -> API<Value> {
    let conn = establish_connection();
    let p = Post::get(&conn, pid).map_err(APIError::from_db)?;
    if !user.is_admin {
        if p.is_reported {
            return Err(APIError::PcError(IsReported));
        }
        if p.is_deleted {
            return Err(APIError::PcError(IsDeleted));
        }
    }
    Ok(json!({
        "data": p2output(&p, &user, &conn),
        "code": 0,
    }))
}

#[get("/getlist?<p>&<order_mode>")]
pub fn get_list(p: Option<u32>, order_mode: u8, user: CurrentUser) -> API<Value> {
    let page = p.unwrap_or(1);
    let conn = establish_connection();
    let ps = Post::gets_by_page(&conn, order_mode, page, 25, user.is_admin)
        .map_err(APIError::from_db)?;
    let ps_data = ps
        .iter()
        .map(|p| p2output(p, &user, &conn))
        .collect::<Vec<PostOutput>>();
    Ok(json!({
        "data": ps_data,
        "count": ps_data.len(),
        "code": 0
    }))
}

#[post("/dopost", data = "<poi>")]
pub fn publish_post(poi: Form<PostInput>, user: CurrentUser) -> API<Value> {
    let conn = establish_connection();
    dbg!(poi.use_title, poi.allow_search);
    let r = Post::create(
        &conn,
        NewPost {
            content: &poi.text,
            cw: &poi.cw,
            author_hash: &user.namehash,
            author_title: "",
            allow_search: poi.allow_search.is_some(),
        },
    )
    .map_err(APIError::from_db)?;
    Ok(json!({
        "data": r,
        "code": 0
    }))
}
