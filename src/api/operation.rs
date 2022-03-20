use crate::api::{APIError, CurrentUser, PolicyError::*, API, UGC, MapToAPIError};
use crate::db_conn::Db;
use crate::models::*;
use rocket::form::Form;
use rocket::serde::json::{json, Value};

#[derive(FromForm)]
pub struct DeleteInput {
    #[field(name = "type")]
    id_type: String,
    id: i32,
    note: String,
}

#[post("/delete", data = "<di>")]
pub async fn delete(di: Form<DeleteInput>, user: CurrentUser, db: Db) -> API<Value> {
    match di.id_type.as_str() {
        "cid" => {
            let c = Comment::get(&db, di.id).await.m()?;
            c.soft_delete(&user, &db).await?;
        }
        "pid" => {
            let p = Post::get(&db, di.id).await.m()?;
            p.soft_delete(&user, &db).await?;
            p.change_n_comments(&db, -1).await.m()?;
        }
        _ => return Err(APIError::PcError(NotAllowed)),
    }

    Ok(json!({
        "code": 0
    }))
}
