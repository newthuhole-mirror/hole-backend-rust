use crate::api::{APIError, CurrentUser, PolicyError::*, API};
use crate::models::*;
use chrono::NaiveDateTime;
use rocket::serde::{
    json::{json, Value},
    Serialize,
};
use std::collections::HashMap;

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct CommentOutput {
    cid: i32,
    text: String,
    can_del: bool,
    name_id: i32,
    create_time: NaiveDateTime,
    // for old version frontend
    timestamp: i64,
}

pub fn c2output(p: &Post, cs: &Vec<Comment>, user: &CurrentUser) -> Vec<CommentOutput> {
    let mut hash2id = HashMap::<&String, i32>::from([(&p.author_hash, 0)]);
    cs.iter()
        .filter_map(|c| {
            let name_id: i32 = match hash2id.get(&c.author_hash) {
                Some(id) => *id,
                None => {
                    let x = hash2id.len().try_into().unwrap();
                    hash2id.insert(&c.author_hash, x);
                    x
                }
            };
            if c.is_deleted {
                None
            } else {
                Some(CommentOutput {
                    cid: c.id,
                    text: c.content.to_string(),
                    can_del: user.is_admin || c.author_hash == user.namehash,
                    name_id: name_id,
                    create_time: c.create_time,
                    timestamp: c.create_time.timestamp(),
                })
            }
        })
        .collect()
}

#[get("/getcomment?<pid>")]
pub fn get_comment(pid: i32, user: CurrentUser) -> API<Value> {
    let conn = establish_connection();
    let p = Post::get(&conn, pid).map_err(APIError::from_db)?;
    if p.is_deleted {
        return Err(APIError::PcError(IsDeleted));
    }
    let cs = p.get_comments(&conn).map_err(APIError::from_db)?;
    Ok(json!({
        "code": 0,
        "data": c2output(&p, &cs, &user),
        "n_likes": p.n_likes,
        // for old version frontend
        "likenum": p.n_likes,
    }))
}
