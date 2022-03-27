use crate::api::{CurrentUser, JsonAPI, PolicyError::*};
use crate::rds_conn::RdsConn;
use crate::rds_models::*;
use rocket::form::Form;
use rocket::futures::future;
use rocket::serde::json::{json, Value};

pub async fn get_poll_dict(pid: i32, rconn: &RdsConn, namehash: &str) -> Option<Value> {
    let opts = PollOption::init(pid, rconn)
        .get_list()
        .await
        .unwrap_or_default();
    if opts.is_empty() {
        None
    } else {
        let choice = future::join_all(opts.iter().enumerate().map(|(idx, opt)| async move {
            PollVote::init(pid, idx, rconn)
                .has(namehash)
                .await
                .unwrap_or_default()
                .then(|| opt)
        }))
        .await
        .into_iter()
        .filter_map(|x| x)
        .collect::<Vec<&String>>()
        .pop();
        Some(json!({
            "answers": future::join_all(
                opts.iter().enumerate().map(|(idx, opt)| async move {
                    json!({
                        "option": opt,
                        "votes": PollVote::init(pid, idx, rconn).count().await.unwrap_or_default(),
                    })
                })
                    ).await,
            "vote": choice,
        }))
    }
}

#[derive(FromForm)]
pub struct VoteInput {
    pid: i32,
    vote: String,
}

#[post("/vote", data = "<vi>")]
pub async fn vote(vi: Form<VoteInput>, user: CurrentUser, rconn: RdsConn) -> JsonAPI {
    user.id.ok_or_else(|| NotAllowed)?;

    let pid = vi.pid;
    let opts = PollOption::init(pid, &rconn).get_list().await?;
    if opts.is_empty() {
        Err(NotAllowed)?;
    }

    for idx in 0..opts.len() {
        if PollVote::init(pid, idx, &rconn).has(&user.namehash).await? {
            Err(NotAllowed)?;
        }
    }

    let idx: usize = opts
        .iter()
        .position(|x| x.eq(&vi.vote))
        .ok_or_else(|| NotAllowed)?;

    PollVote::init(pid, idx, &rconn).add(&user.namehash).await?;

    code0!(get_poll_dict(vi.pid, &rconn, &user.namehash).await)
}
