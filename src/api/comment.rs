use crate::api::{APIError, CurrentUser, MapToAPIError, PolicyError::*, API};
use crate::db_conn::Db;
use crate::models::*;
use crate::rds_conn::RdsConn;
use crate::rds_models::*;
use chrono::{offset::Utc, DateTime};
use rocket::form::Form;
use rocket::serde::{
    json::{json, Value},
    Serialize,
};
use std::collections::HashMap;

#[derive(FromForm)]
pub struct CommentInput {
    pid: i32,
    #[field(validate = len(1..4097))]
    text: String,
    use_title: Option<i8>,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub struct CommentOutput {
    cid: i32,
    text: String,
    can_del: bool,
    name_id: i32,
    is_tmp: bool,
    create_time: DateTime<Utc>,
    // for old version frontend
    timestamp: i64,
}

pub fn c2output<'r>(p: &'r Post, cs: &Vec<Comment>, user: &CurrentUser) -> Vec<CommentOutput> {
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
                // TODO: block
                None
            } else {
                Some(CommentOutput {
                    cid: c.id,
                    text: format!("{}{}", if c.is_tmp { "[tmp]\n" } else { "" }, c.content),
                    can_del: user.is_admin || c.author_hash == user.namehash,
                    name_id: name_id,
                    is_tmp: c.is_tmp,
                    create_time: c.create_time,
                    timestamp: c.create_time.timestamp(),
                })
            }
        })
        .collect()
}

#[get("/getcomment?<pid>")]
pub async fn get_comment(pid: i32, user: CurrentUser, db: Db, rconn: RdsConn) -> API<Value> {
    let p = Post::get(&db, pid).await.m()?;
    if p.is_deleted {
        return Err(APIError::PcError(IsDeleted));
    }
    let pid = p.id;
    let cs = Comment::gets_by_post_id(&db, pid).await.m()?;
    let data = c2output(&p, &cs, &user);

    Ok(json!({
        "code": 0,
        "data": data,
        "n_attentions": p.n_attentions,
        // for old version frontend
        "likenum": p.n_attentions,
        "attention": Attention::init(&user.namehash, rconn.clone()).has(p.id).await.m()? ,
    }))
}

#[post("/docomment", data = "<ci>")]
pub async fn add_comment(
    ci: Form<CommentInput>,
    user: CurrentUser,
    db: Db,
    rconn: RdsConn,
) -> API<Value> {
    let p = Post::get(&db, ci.pid).await.m()?;
    Comment::create(
        &db,
        NewComment {
            content: ci.text.to_string(),
            author_hash: user.namehash.to_string(),
            author_title: "".to_string(),
            is_tmp: user.id.is_none(),
            post_id: ci.pid,
        },
    )
    .await
    .m()?;
    p.change_n_comments(&db, 1).await.m()?;
    // auto attention after comment
    let mut att = Attention::init(&user.namehash, rconn);
    if !att.has(p.id).await.m()? {
        att.add(p.id).await.m()?;
        p.change_n_attentions(&db, 1).await.m()?;
    }
    Ok(json!({
        "code": 0
    }))
}
