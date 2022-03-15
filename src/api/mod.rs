use crate::models::*;
use crate::random_hasher::RandomHasher;
use rocket::http::Status;
use rocket::request::{self, FromRequest, Request};
use rocket::response::{self, Responder};
use rocket::serde::json::json;

#[catch(401)]
pub fn catch_401_error() -> &'static str {
    "未登录或token过期"
}

pub struct CurrentUser {
    id: Option<i32>, // tmp user has no id, only for block
    namehash: String,
    is_admin: bool,
    custom_title: String,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for CurrentUser {
    type Error = ();
    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let rh = request.rocket().state::<RandomHasher>().unwrap();
        let mut cu: Option<CurrentUser> = None;

        if let Some(token) = request.headers().get_one("User-Token") {
            let sp = token.split('_').collect::<Vec<&str>>();
            if sp.len() == 2 && sp[0] == rh.get_tmp_token() {
                let namehash = rh.hash_with_salt(sp[1]);
                cu = Some(CurrentUser {
                    id: None,
                    custom_title: format!("TODO: {}", &namehash),
                    namehash: namehash,
                    is_admin: false,
                });
            } else {
                let conn = establish_connection();
                if let Some(user) = User::get_by_token(&conn, token) {
                    let namehash = rh.hash_with_salt(&user.name);
                    cu = Some(CurrentUser {
                        id: Some(user.id),
                        custom_title: format!("TODO: {}", &namehash),
                        namehash: namehash,
                        is_admin: user.is_admin,
                    });
                }
            }
        }
        match cu {
            Some(u) => request::Outcome::Success(u),
            None => request::Outcome::Failure((Status::Unauthorized, ())),
        }
    }
}

pub enum PolicyError {
    IsReported,
    IsDeleted,
    NotAllowed,
}

pub enum APIError {
    DbError(diesel::result::Error),
    PcError(PolicyError),
}

impl APIError {
    fn from_db(err: diesel::result::Error) -> APIError {
        APIError::DbError(err)
    }
}

impl<'r> Responder<'r, 'static> for APIError {
    fn respond_to(self, req: &'r Request<'_>) -> response::Result<'static> {
        match self {
            APIError::DbError(e) => json!({
                "code": -1,
                "msg": e.to_string()
            })
            .respond_to(req),
            APIError::PcError(e) => json!({
                "code": -1,
                "msg": match e {
                    PolicyError::IsReported => "内容被举报，处理中",
                    PolicyError::IsDeleted => "内容被删除",
                    PolicyError::NotAllowed => "不允许的操作",
                }
            })
            .respond_to(req),
        }
    }
}

macro_rules! look {
    ($s:expr) => {
        format!("{}...{}", &$s[..2], &$s[$s.len() - 2..])
    };
}

pub type API<T> = Result<T, APIError>;

pub mod comment;
pub mod post;
pub mod systemlog;
