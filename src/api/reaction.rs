use crate::api::{CurrentUser, JsonApi, PolicyError::*, Ugc};
use crate::db_conn::Db;
use crate::models::*;
use crate::rds_conn::RdsConn;
use crate::rds_models::*;
use rocket::form::Form;
use rocket::serde::json::json;

#[derive(FromForm)]
pub struct ReactionInput {
    #[field(validate = range(-1..2))]
    status: i32,
}

#[post("/post/<pid>/reaction", data = "<ri>")]
pub async fn reaction(
    pid: i32,
    ri: Form<ReactionInput>,
    user: CurrentUser,
    db: Db,
    rconn: RdsConn,
) -> JsonApi {
    user.id.ok_or(YouAreTmp)?;

    let mut p = Post::get(&db, &rconn, pid).await?;
    p.check_permission(&user, "r")?;
    let mut r_up = Reaction::init(pid, 1, &rconn);
    let mut r_down = Reaction::init(pid, -1, &rconn);

    let (delta_up, delta_down): (i32, i32) = match ri.status {
        1 => (
            r_up.add(&user.namehash).await? as i32,
            -(r_down.rem(&user.namehash).await? as i32),
        ),
        -1 => (
            -(r_up.rem(&user.namehash).await? as i32),
            r_down.add(&user.namehash).await? as i32,
        ),
        _ => (
            -(r_up.rem(&user.namehash).await? as i32),
            -(r_down.rem(&user.namehash).await? as i32),
        ),
    };

    if delta_up != 0 || delta_down != 0 {
        update!(
            p,
            posts,
            &db,
            { up_votes, add delta_up },
            { down_votes, add delta_down }
        );

        p.refresh_cache(&rconn, false).await;
    }

    Ok(json!({
        "code": 0,
        "data": {
            "up_votes": p.up_votes,
            "down_votes": p.down_votes,
            "reaction_status": ri.status,
        },
    }))
}
