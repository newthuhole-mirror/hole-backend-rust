use crate::db_conn::Db;
use crate::libs::diesel_logger::LoggingConnection;
use crate::models::*;
use crate::random_hasher::RandomHasher;
use crate::rds_conn::RdsConn;
use crate::rds_models::BannedUsers;
use crate::schema;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use rocket::http::Status;
use rocket::outcome::try_outcome;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::response::{self, Responder};
use rocket::serde::json::{json, Value};

macro_rules! code0 {
    () => (
        Ok(json!({"code": 0}))
    );

    ($data:expr) => (
        Ok(json!({
            "code": 0,
            "data": $data,
        }))
    );
}

#[catch(401)]
pub fn catch_401_error() -> &'static str {
    "未登录或token过期"
}

#[catch(403)]
pub fn catch_403_error() -> &'static str {
    "可能被封禁了，等下次重置吧"
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
    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let rh = request.rocket().state::<RandomHasher>().unwrap();
        let rconn = try_outcome!(request.guard::<RdsConn>().await);

        let mut id = None;
        let mut namehash = None;
        let mut is_admin = false;

        if let Some(token) = request.headers().get_one("User-Token") {
            let sp = token.split('_').collect::<Vec<&str>>();
            if sp.len() == 2 && sp[0] == rh.get_tmp_token() {
                namehash = Some(rh.hash_with_salt(sp[1]));
                id = None;
                is_admin = false;
            } else {
                let db = try_outcome!(request.guard::<Db>().await);
                if let Some(u) = User::get_by_token(&db, &rconn, token).await {
                    id = Some(u.id);
                    namehash = Some(rh.hash_with_salt(&u.name));
                    is_admin = u.is_admin;
                }
            }
        }
        match namehash {
            Some(nh) => {
                if BannedUsers::has(&rconn, &nh).await.unwrap() {
                    Outcome::Failure((Status::Forbidden, ()))
                } else {
                    Outcome::Success(CurrentUser {
                        id: id,
                        custom_title: format!("title todo: {}", &nh),
                        namehash: nh,
                        is_admin: is_admin,
                    })
                }
            }
            None => Outcome::Failure((Status::Unauthorized, ())),
        }
    }
}

#[derive(Debug)]
pub enum PolicyError {
    IsReported,
    IsDeleted,
    NotAllowed,
    TitleUsed,
    YouAreTmp,
}

#[derive(Debug)]
pub enum APIError {
    DbError(diesel::result::Error),
    RdsError(redis::RedisError),
    PcError(PolicyError),
}

impl<'r> Responder<'r, 'static> for APIError {
    fn respond_to(self, req: &'r Request<'_>) -> response::Result<'static> {
        match self {
            APIError::DbError(e) => json!({
                "code": -1,
                "msg": e.to_string()
            })
            .respond_to(req),
            APIError::RdsError(e) => json!({
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
                    PolicyError::TitleUsed => "头衔已被使用",
                    PolicyError::YouAreTmp => "临时用户只可发布内容和进入单个洞"
                }
            })
            .respond_to(req),
        }
    }
}

impl From<diesel::result::Error> for APIError {
    fn from(err: diesel::result::Error) -> APIError {
        APIError::DbError(err)
    }
}

impl From<redis::RedisError> for APIError {
    fn from(err: redis::RedisError) -> APIError {
        APIError::RdsError(err)
    }
}

impl From<PolicyError> for APIError {
    fn from(err: PolicyError) -> APIError {
        APIError::PcError(err)
    }
}

pub type API<T> = Result<T, APIError>;
pub type JsonAPI = API<Value>;

#[rocket::async_trait]
pub trait UGC {
    fn get_author_hash(&self) -> &str;
    fn get_is_deleted(&self) -> bool;
    fn get_is_reported(&self) -> bool;
    fn extra_delete_condition(&self) -> bool;
    async fn do_set_deleted(&mut self, db: &Db) -> API<()>;
    fn check_permission(&self, user: &CurrentUser, mode: &str) -> API<()> {
        if user.is_admin {
            return Ok(());
        }
        if mode.contains('r') && self.get_is_deleted() {
            return Err(APIError::PcError(PolicyError::IsDeleted));
        }
        if mode.contains('o') && self.get_is_reported() {
            return Err(APIError::PcError(PolicyError::IsReported));
        }
        if mode.contains('w') && self.get_author_hash() != user.namehash {
            return Err(APIError::PcError(PolicyError::NotAllowed));
        }
        if mode.contains('d') && !self.extra_delete_condition() {
            return Err(APIError::PcError(PolicyError::NotAllowed));
        }
        Ok(())
    }

    async fn soft_delete(&mut self, user: &CurrentUser, db: &Db) -> API<()> {
        self.check_permission(user, "rwd")?;

        self.do_set_deleted(db).await?;
        Ok(())
    }
}

#[rocket::async_trait]
impl UGC for Post {
    fn get_author_hash(&self) -> &str {
        &self.author_hash
    }
    fn get_is_reported(&self) -> bool {
        self.is_reported
    }
    fn get_is_deleted(&self) -> bool {
        self.is_deleted
    }
    fn extra_delete_condition(&self) -> bool {
        self.n_comments == 0
    }
    async fn do_set_deleted(&mut self, db: &Db) -> API<()> {
        update!(*self, posts, db, { is_deleted, to true });
        Ok(())
    }
}

#[rocket::async_trait]
impl UGC for Comment {
    fn get_author_hash(&self) -> &str {
        &self.author_hash
    }
    fn get_is_reported(&self) -> bool {
        false
    }
    fn get_is_deleted(&self) -> bool {
        self.is_deleted
    }
    fn extra_delete_condition(&self) -> bool {
        true
    }
    async fn do_set_deleted(&mut self, db: &Db) -> API<()> {
        update!(*self, comments, db, { is_deleted, to true });
        Ok(())
    }
}

macro_rules! look {
    ($s:expr) => {
        format!("{}...{}", &$s[..2], &$s[$s.len() - 2..])
    };
}

pub mod attention;
pub mod comment;
pub mod operation;
pub mod post;
pub mod search;
pub mod systemlog;
pub mod vote;
