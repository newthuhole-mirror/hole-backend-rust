use crate::api::post::ps2outputs;
use crate::api::{CurrentUser, JsonApi, PolicyError::*, Ugc};
use crate::db_conn::Db;
use crate::libs::diesel_logger::LoggingConnection;
use crate::models::*;
use crate::rds_conn::RdsConn;
use crate::rds_models::*;
use crate::schema;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use rocket::form::Form;
use rocket::serde::json::json;
use rocket::serde::json::serde_json;
use rocket::serde::Serialize;
use std::fs::File;
use url::Url;
use web_push::{
    ContentEncoding, SubscriptionInfo, VapidSignatureBuilder, WebPushClient, WebPushMessageBuilder,
};

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
) -> JsonApi {
    // 临时用户不允许手动关注
    user.id.ok_or(YouAreTmp)?;

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
        let hot_delta = if p.n_attentions <= 3 * p.n_comments {
            delta * 2
        } else {
            0
        };
        update!(
            p,
            posts,
            &db,
            { n_attentions, add delta },
            { hot_score, add hot_delta }
        );
        if switch_to && user.is_admin {
            update!(p, posts, &db, { is_reported, to false });
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
pub async fn get_attention(user: CurrentUser, db: Db, rconn: RdsConn) -> JsonApi {
    let mut ids = Attention::init(&user.namehash, &rconn).all().await?;
    ids.sort_by_key(|x| -x);
    let ps = Post::get_multi(&db, &rconn, &ids).await?;
    let ps_data = ps2outputs(&ps, &user, &db, &rconn).await?;

    code0!(ps_data)
}

#[derive(FromForm)]
pub struct NotificatinInput {
    enable: bool,
    endpoint: String,
    auth: String,
    p256dh: String,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
struct PushData {
    title: String,
    pid: i32,
    text: String,
}

#[post("/post/<pid>/notification", data = "<ni>")]
pub async fn set_notification(pid: i32, ni: Form<NotificatinInput>, _user: CurrentUser) -> JsonApi {
    let url_host = Url::parse(&ni.endpoint)
        .map_err(|_| UnknownPushEndpoint)?
        .host()
        .ok_or(UnknownPushEndpoint)?
        .to_string();
    (url_host.ends_with("googleapis.com") || url_host.ends_with("mozilla.com"))
        .then(|| ())
        .ok_or(UnknownPushEndpoint)?;

    if ni.enable {
        let subscription_info = SubscriptionInfo::new(&ni.endpoint, &ni.p256dh, &ni.auth);

        let file = File::open("keys/private.pem").unwrap();
        let sig_builder = VapidSignatureBuilder::from_pem(file, &subscription_info)
            .unwrap()
            .build()
            .unwrap();

        let mut builder = WebPushMessageBuilder::new(&subscription_info).unwrap();
        let data = PushData {
            title: "测试".to_owned(),
            pid,
            text: format!("#{} 开启提醒测试成功，消息提醒功能即将正式上线", &pid),
        };
        let content = serde_json::to_string(&data).unwrap();
        builder.set_payload(ContentEncoding::Aes128Gcm, content.as_bytes());
        builder.set_vapid_signature(sig_builder);

        let client = WebPushClient::new()?;

        client.send(builder.build()?).await?;
    }

    code0!()
}
