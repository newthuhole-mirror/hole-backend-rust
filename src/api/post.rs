use crate::api::{APIError, CurrentUser, PolicyError::*, API};
use crate::models::*;
use chrono::NaiveDateTime;
use rocket::serde::{
    json::{json, Json, Value},
    Deserialize, Serialize,
};

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct PostInput<'r> {
    content: &'r str,
    cw: &'r str,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct PostOutput {
    id: i32,
    content: String,
    cw: String,
    author_title: String,
    n_likes: i32,
    n_comments: i32,
    create_time: NaiveDateTime,
    last_comment_time: NaiveDateTime,
    allow_search: bool,
    is_reported: Option<bool>,
    // for old version frontend
    timestamp: NaiveDateTime,
}

fn p2output(p: &Post, user: &CurrentUser) -> PostOutput {
    PostOutput {
        id: p.id,
        content: p.content.to_string(),
        cw: p.cw.to_string(),
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
        // for old version frontend
        timestamp: p.create_time,
    }
}

#[get("/post/<pid>")]
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
        "data": p2output(&p, &user),
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
        .map(|p| p2output(p, &user))
        .collect::<Vec<PostOutput>>();
    Ok(json!({
        "data": ps_data,
        "count": ps_data.len(),
        "code": 0
    }))
}

#[post("/dopost", format = "json", data = "<poi>")]
pub fn publish_post(poi: Json<PostInput>, user: CurrentUser) -> API<Value> {
    let conn = establish_connection();
    let r = Post::create(
        &conn,
        NewPost {
            content: &poi.content,
            cw: &poi.cw,
            author_hash: &user.namehash,
            author_title: "",
        },
    )
    .map_err(APIError::from_db)?;
    Ok(json!({
        "data": r,
        "code": 0
    }))
}
