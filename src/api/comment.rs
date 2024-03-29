use crate::api::{ApiError, CurrentUser, JsonApi, PolicyError::*, Ugc};
use crate::cache::BlockDictCache;
use crate::db_conn::Db;
use crate::models::*;
use crate::rds_conn::RdsConn;
use crate::rds_models::*;
use futures::{future, join};
use rocket::form::Form;
use rocket::serde::{json::json, Serialize};
use std::collections::HashMap;

#[derive(FromForm)]
pub struct CommentInput {
    #[field(validate = len(1..12289))]
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
    create_time: i64,
    is_blocked: bool,
    //blocked_count: Option<i32>,
    // for old version frontend
    timestamp: i64,
    blocked: bool,
}

pub async fn c2output<'r>(
    p: &'r Post,
    cs: &[Comment],
    user: &CurrentUser,
    cached_block_dict: &HashMap<String, bool>,
    //rconn: &RdsConn,
) -> Vec<CommentOutput> {
    let mut hash2id = HashMap::<&String, i32>::from([(&p.author_hash, 0)]);
    let name_ids_iter = cs.iter().map(|c| match hash2id.get(&c.author_hash) {
        Some(id) => *id,
        None => {
            let x = hash2id.len().try_into().unwrap();
            hash2id.insert(&c.author_hash, x);
            x
        }
    });
    future::join_all(cs.iter().zip(name_ids_iter).map(|(c, name_id)| async move {
        if c.is_deleted {
            None
        } else {
            let is_blocked = cached_block_dict[&c.author_hash];
            let can_view = user.is_admin
                || (!is_blocked && user.id.is_some() || user.namehash.eq(&c.author_hash));
            Some(CommentOutput {
                cid: c.id,
                text: (if can_view { &c.content } else { "" }).to_string(),
                author_title: c.author_title.to_string(),
                can_del: c.check_permission(user, "wd").is_ok(),
                name_id,
                is_tmp: c.is_tmp,
                create_time: c.create_time.timestamp(),
                is_blocked,
                /*
                blocked_count: if user.is_admin {
                    BlockCounter::get_count(rconn, &c.author_hash)
                        .await
                        .ok()
                        .flatten()
                } else {
                    None
                },
                */
                timestamp: c.create_time.timestamp(),
                blocked: is_blocked,
            })
        }
    }))
    .await
    .into_iter()
    .flatten()
    .collect()
}

#[get("/getcomment?<pid>")]
pub async fn get_comment(pid: i32, user: CurrentUser, db: Db, rconn: RdsConn) -> JsonApi {
    let p = Post::get(&db, &rconn, pid).await?;
    if p.is_deleted {
        return Err(ApiError::Pc(IsDeleted));
    }
    let cs = p.get_comments(&db, &rconn).await?;
    let hash_list = cs.iter().map(|c| &c.author_hash).collect::<Vec<_>>();
    let cached_block_dict = BlockDictCache::init(&user.namehash, p.id, &rconn)
        .get_or_create(&user, &hash_list)
        .await?;
    let data = c2output(&p, &cs, &user, &cached_block_dict).await;

    Ok(json!({
        "code": 0,
        "data": data,
        "n_attentions": p.n_attentions,
        // for old version frontend
        "likenum": p.n_attentions,
        "attention": Attention::init(&user.namehash, &rconn).has(p.id).await? ,
    }))
}

#[post("/post/<pid>/comment", data = "<ci>")]
pub async fn add_comment(
    pid: i32,
    ci: Form<CommentInput>,
    user: CurrentUser,
    db: Db,
    rconn: RdsConn,
) -> JsonApi {
    let mut p = Post::get(&db, &rconn, pid).await?;
    if p.author_hash != user.namehash {
        user.id.ok_or(YouAreTmp)?;
    }
    let use_title = ci.use_title.is_some() || user.is_admin || user.is_candidate;
    let c = Comment::create(
        &db,
        NewComment {
            content: ci.text.to_string(),
            author_hash: user.namehash.to_string(),
            author_title: user
                .custom_title
                .and_then(|title| use_title.then_some(title))
                .unwrap_or_default(),
            is_tmp: user.id.is_none(),
            post_id: pid,
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
        hs_delta = (p.n_comments < 3 * p.n_attentions) as i32;
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
