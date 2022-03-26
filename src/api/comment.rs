use crate::api::{APIError, CurrentUser, JsonAPI, PolicyError::*, UGC};
use crate::db_conn::Db;
use crate::libs::diesel_logger::LoggingConnection;
use crate::models::*;
use crate::rds_conn::RdsConn;
use crate::rds_models::*;
use crate::schema;
use chrono::{offset::Utc, DateTime};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use rocket::form::Form;
use rocket::futures::join;
use rocket::serde::{json::json, Serialize};
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
    author_title: String,
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
                    author_title: c.author_title.to_string(),
                    can_del: c.check_permission(user, "wd").is_ok(),
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
pub async fn get_comment(pid: i32, user: CurrentUser, db: Db, rconn: RdsConn) -> JsonAPI {
    let p = Post::get(&db, &rconn, pid).await?;
    if p.is_deleted {
        return Err(APIError::PcError(IsDeleted));
    }
    let cs = p.get_comments(&db, &rconn).await?;
    let data = c2output(&p, &cs, &user);

    Ok(json!({
        "code": 0,
        "data": data,
        "n_attentions": p.n_attentions,
        // for old version frontend
        "likenum": p.n_attentions,
        "attention": Attention::init(&user.namehash, &rconn).has(p.id).await? ,
    }))
}

#[post("/docomment", data = "<ci>")]
pub async fn add_comment(
    ci: Form<CommentInput>,
    user: CurrentUser,
    db: Db,
    rconn: RdsConn,
) -> JsonAPI {
    let mut p = Post::get(&db, &rconn, ci.pid).await?;
    let c = Comment::create(
        &db,
        NewComment {
            content: ci.text.to_string(),
            author_hash: user.namehash.to_string(),
            author_title: "".to_string(),
            is_tmp: user.id.is_none(),
            post_id: ci.pid,
        },
    )
    .await?;

    let mut att = Attention::init(&user.namehash, &rconn);
    let hs_delta;
    let at_delta;

    if !att.has(p.id).await? {
        hs_delta = 3;
        at_delta = 1;
        att.add(p.id).await?;
    } else {
        hs_delta = 1;
        at_delta = 0;
    }

    update!(
        p,
        posts,
        &db,
        { n_comments, add 1 },
        { last_comment_time, to c.create_time },
        { n_attentions, add at_delta },
        { hot_score, add hs_delta }
    );

    join!(
        p.refresh_cache(&rconn, false),
        p.clear_comments_cache(&rconn),
    );

    Ok(json!({
        "code": 0
    }))
}
