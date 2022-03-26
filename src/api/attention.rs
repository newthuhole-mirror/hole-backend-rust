use crate::api::post::ps2outputs;
use crate::api::{CurrentUser, JsonAPI, PolicyError::*, UGC};
use crate::db_conn::Db;
use crate::models::*;
use crate::rds_conn::RdsConn;
use crate::rds_models::*;
use rocket::form::Form;
use rocket::serde::json::json;

#[derive(FromForm)]
pub struct AttentionInput {
    pid: i32,
    #[field(validate = range(0..2))]
    switch: i32,
}

#[post("/attention", data = "<ai>")]
pub async fn attention_post(
    ai: Form<AttentionInput>,
    user: CurrentUser,
    db: Db,
    rconn: RdsConn,
) -> JsonAPI {
    // 临时用户不允许手动关注
    user.id.ok_or_else(|| NotAllowed)?;

    let mut p = Post::get(&db, &rconn, ai.pid).await?;
    p.check_permission(&user, "r")?;
    let mut att = Attention::init(&user.namehash, &rconn);
    let switch_to = ai.switch == 1;
    let delta: i32;
    if att.has(ai.pid).await? != switch_to {
        if switch_to {
            att.add(ai.pid).await?;
            delta = 1;
        } else {
            att.remove(ai.pid).await?;
            delta = -1;
        }
        p.change_n_attentions(&db, delta).await?;
        p.change_hot_score(&db, delta * 2).await?;
        if switch_to && user.is_admin {
            p.set_is_reported(&db, false).await?;
        }
        p.refresh_cache(&rconn, false).await;
    }

    Ok(json!({
        "code": 0,
        "attention": ai.switch == 1,
        "n_attentions": p.n_attentions,
        // for old version frontend
        "likenum": p.n_attentions,
    }))
}

#[get("/getattention")]
pub async fn get_attention(user: CurrentUser, db: Db, rconn: RdsConn) -> JsonAPI {
    let ids = Attention::init(&user.namehash, &rconn).all().await?;
    let ps = Post::get_multi(&db, &rconn, &ids).await?;
    let ps_data = ps2outputs(&ps, &user, &db, &rconn).await;

    Ok(json!({
        "code": 0,
        "data": ps_data,
    }))
}
