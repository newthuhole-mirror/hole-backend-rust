use crate::random_hasher::RandomHasher;
use rocket::http::Status;
use rocket::request::{self, FromRequest, Request};
use rocket::serde::json::{Value, json};
use rocket::response::{self, Responder};

#[catch(401)]
pub fn catch_401_error() -> Value {
    json!({
        "code": -1,
        "msg": "未登录或token过期"
    })
}

pub struct CurrentUser {
    namehash: String,
    is_admin: bool,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for CurrentUser {
    type Error = ();
    async fn from_request(request: &'r Request<'_>) -> request::Outcome<Self, Self::Error> {
        let token = request.headers().get_one("User-Token");
        match token {
            Some(t) => request::Outcome::Success(CurrentUser {
                namehash: request
                    .rocket()
                    .state::<RandomHasher>()
                    .unwrap()
                    .hash_with_salt(t),
                is_admin: t == "admin",  // TODO
            }),
            None => request::Outcome::Failure((Status::Unauthorized, ())),
        }
    }
}

pub enum PolicyError {
    IsReported,
    IsDeleted,
    NotAllowed
}


pub enum APIError {
    DbError(diesel::result::Error),
    PcError(PolicyError)
}

impl APIError {
    fn from_db(err: diesel::result::Error) -> APIError {
        APIError::DbError(err)
    }
}

impl<'r> Responder<'r, 'static> for APIError {
    fn respond_to(self, req: &'r Request<'_>) -> response::Result<'static> {
        match self {
            APIError::DbError(e) =>
                json!({
                    "code": -1,
                    "msg": e.to_string()
                }).respond_to(req),
            APIError::PcError(e) =>
                json!({
                    "code": -1,
                    "msg": match e {
                        PolicyError::IsReported => "内容被举报，处理中",
                        PolicyError::IsDeleted => "内容被删除",
                        PolicyError::NotAllowed => "不允许的操作",
                    }
                }).respond_to(req),
        }
    }
}

pub type API<T> = Result<T, APIError>;

pub mod post;
