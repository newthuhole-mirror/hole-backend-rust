use crate::api::post::ps2outputs;
use crate::api::{APIError, CurrentUser, MapToAPIError, PolicyError::*, API, UGC};
use crate::db_conn::Db;
use crate::models::*;
use crate::rds_conn::RdsConn;
use crate::rds_models::*;
use rocket::form::Form;
use rocket::serde::json::{json, Value};

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
) -> API<Value> {
    user.id.ok_or_else(|| APIError::PcError(NotAllowed))?;
    let p = Post::get(&db, ai.pid).await.m()?;
    p.check_permission(&user, "r")?;
    let mut att = Attention::init(&user.namehash, rconn);
    let switch_to = ai.switch == 1;
    let mut delta: i32 = 0;
    if att.has(ai.pid).await.m()? != switch_to {
        if switch_to {
            att.add(ai.pid).await.m()?;
            delta = 1;
        } else {
            att.remove(ai.pid).await.m()?;
            delta = -1;
        }
        p.change_n_attentions(&db, delta).await.m()?;
    }

    Ok(json!({
        "code": 0,
        "attention": ai.switch == 1,
        "n_attentions": p.n_attentions + delta,
        // for old version frontend
        "likenum": p.n_attentions + delta,
    }))
}

#[get("/getattention")]
pub async fn get_attention(user: CurrentUser, db: Db, rconn: RdsConn) -> API<Value> {
    let ids = Attention::init(&user.namehash, rconn.clone())
        .all()
        .await
        .m()?;
    let ps = Post::get_multi(&db, ids).await.m()?;
    let ps_data = ps2outputs(&ps, &user, &db, rconn.clone()).await;

    Ok(json!({
        "code": 0,
        "data": ps_data,
    }))
}
