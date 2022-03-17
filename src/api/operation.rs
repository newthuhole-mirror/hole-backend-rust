use crate::api::{APIError, CurrentUser, PolicyError::*, API, UGC};
use crate::db_conn::DbConn;
use crate::models::*;
use rocket::form::Form;
use rocket::serde::json::{json, Value};

#[derive(FromForm)]
pub struct DeleteInput<'r> {
    #[field(name = "type")]
    id_type: &'r str,
    id: i32,
    note: &'r str,
}

#[post("/delete", data = "<di>")]
pub fn delete(di: Form<DeleteInput>, user: CurrentUser, conn: DbConn) -> API<Value> {
    match di.id_type {
        "cid" => {
            let c = Comment::get(&conn, di.id).map_err(APIError::from_db)?;
            c.soft_delete(&user, &conn)?;
        }
        "pid" => {
            let p = Post::get(&conn, di.id).map_err(APIError::from_db)?;
            p.soft_delete(&user, &conn)?;
        }
        _ => return Err(APIError::PcError(NotAllowed)),
    }

    Ok(json!({
        "code": 0
    }))
}
