#![allow(clippy::unnecessary_lazy_evaluations)]

use crate::db_conn::Db;
use crate::models::*;
use crate::random_hasher::RandomHasher;
use crate::rds_conn::RdsConn;
use crate::rds_models::*;
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

/*
macro_rules! code1 {
    ($msg:expr) => (
        Ok(json!({
            "code": 1,
            "msg": $msg,
        }))
    );
}
*/

macro_rules! e2s {
    ($e:expr) => (json!({
        "code": -1,
        "msg": $e.to_string()
    }));
}

#[catch(401)]
pub fn catch_401_error() -> &'static str {
    "未登录或token过期"
}

#[catch(403)]
pub fn catch_403_error() -> &'static str {
    "可能被封禁了，等下次重置吧"
}

#[catch(404)]
pub fn catch_404_error() -> &'static str {
    "请更新前端版本"
}

pub struct CurrentUser {
    pub id: Option<i32>, // tmp user has no id, only for block
    namehash: String,
    is_admin: bool,
    is_candidate: bool,
    custom_title: Option<String>,
    title_secret: Option<String>,
    pub auto_block_rank: u8,
}

impl CurrentUser {
    pub async fn from_hash(rconn: &RdsConn, namehash: String) -> Self {
        let (custom_title, title_secret) = CustomTitle::get(rconn, &namehash)
            .await
            .ok()
            .unwrap_or((None, None));
        Self {
            id: None,
            is_admin: false,
            is_candidate: false,
            custom_title,
            title_secret,
            auto_block_rank: AutoBlockRank::get(rconn, &namehash).await.unwrap_or(2),
            namehash,
        }
    }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for CurrentUser {
    type Error = ();
    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let rh = request.rocket().state::<RandomHasher>().unwrap();
        let rconn = try_outcome!(request.guard::<RdsConn>().await);

        if let Some(user) = {
            if let Some(token) = request.headers().get_one("User-Token") {
                let sp = token.split('_').collect::<Vec<&str>>();
                if sp.len() == 2 && sp[0] == rh.get_tmp_token() {
                    Some(CurrentUser::from_hash(&rconn, rh.hash_with_salt(sp[1])).await)
                } else {
                    let db = try_outcome!(request.guard::<Db>().await);
                    if let Some(u) = User::get_by_token(&db, &rconn, token).await {
                        let namehash = rh.hash_with_salt(&u.name);
                        let user_base = CurrentUser::from_hash(&rconn, namehash).await;
                        Some(CurrentUser {
                            id: Some(u.id),
                            is_admin: u.is_admin
                                || is_elected_admin(&rconn, &user_base.custom_title)
                                    .await
                                    .unwrap(),
                            is_candidate: is_elected_candidate(&rconn, &user_base.custom_title)
                                .await
                                .unwrap(),
                            ..user_base
                        })
                    } else {
                        None
                    }
                }
            } else {
                None
            }
        } {
            if BannedUsers::has(&rconn, &user.namehash).await.unwrap() {
                Outcome::Failure((Status::Forbidden, ()))
            } else {
                Outcome::Success(user)
            }
        } else {
            Outcome::Failure((Status::Unauthorized, ()))
        }
    }
}

#[derive(Debug)]
pub enum PolicyError {
    IsReported,
    IsDeleted,
    NotAllowed,
    TitleUsed,
    TitleProtected,
    InvalidTitle,
    YouAreTmp,
    NoReason,
    UnknownPushEndpoint,
}

#[derive(Debug)]
pub enum ApiError {
    Db(diesel::result::Error),
    Rds(redis::RedisError),
    WebPush(web_push::WebPushError),
    Pc(PolicyError),
    IO(std::io::Error),
}

impl<'r> Responder<'r, 'static> for ApiError {
    fn respond_to(self, req: &'r Request<'_>) -> response::Result<'static> {
        match self {
            ApiError::Db(e) => e2s!(e).respond_to(req),
            ApiError::Rds(e) => e2s!(e).respond_to(req),
            ApiError::WebPush(e) => e2s!(e).respond_to(req),
            ApiError::IO(e) => e2s!(e).respond_to(req),
            ApiError::Pc(e) => json!({
                "code": -1,
                "msg": match e {
                    PolicyError::IsReported => "内容被举报，处理中",
                    PolicyError::IsDeleted => "内容被删除",
                    PolicyError::NotAllowed => "不允许的操作",
                    PolicyError::TitleUsed => "头衔已被使用",
                    PolicyError::TitleProtected => "头衔处于保护期",
                    PolicyError::InvalidTitle => "头衔包含不允许的符号",
                    PolicyError::YouAreTmp => "临时用户只可发布内容和进入单个洞",
                    PolicyError::NoReason => "未填写理由",
                    PolicyError::UnknownPushEndpoint => "未知的浏览器推送地址",
                }
            })
            .respond_to(req),
        }
    }
}

impl From<web_push::WebPushError> for ApiError {
    fn from(err: web_push::WebPushError) -> ApiError {
        ApiError::WebPush(err)
    }
}

impl From<diesel::result::Error> for ApiError {
    fn from(err: diesel::result::Error) -> ApiError {
        ApiError::Db(err)
    }
}

impl From<redis::RedisError> for ApiError {
    fn from(err: redis::RedisError) -> ApiError {
        ApiError::Rds(err)
    }
}

impl From<std::io::Error> for ApiError {
    fn from(err: std::io::Error) -> ApiError {
        ApiError::IO(err)
    }
}

impl From<PolicyError> for ApiError {
    fn from(err: PolicyError) -> ApiError {
        ApiError::Pc(err)
    }
}

pub type Api<T> = Result<T, ApiError>;
pub type JsonApi = Api<Value>;

#[rocket::async_trait]
pub trait Ugc {
    fn get_author_hash(&self) -> &str;
    fn get_is_deleted(&self) -> bool;
    fn get_is_reported(&self) -> bool;
    fn extra_delete_condition(&self) -> bool;
    async fn do_set_deleted(&mut self, db: &Db) -> Api<()>;
    fn check_permission(&self, user: &CurrentUser, mode: &str) -> Api<()> {
        if user.is_admin {
            return Ok(());
        }
        if mode.contains('r') && self.get_is_deleted() {
            return Err(ApiError::Pc(PolicyError::IsDeleted));
        }
        if mode.contains('o') && self.get_is_reported() {
            return Err(ApiError::Pc(PolicyError::IsReported));
        }
        if mode.contains('w') && self.get_author_hash() != user.namehash {
            return Err(ApiError::Pc(PolicyError::NotAllowed));
        }
        if mode.contains('d') && !self.extra_delete_condition() {
            return Err(ApiError::Pc(PolicyError::NotAllowed));
        }
        Ok(())
    }

    async fn soft_delete(&mut self, user: &CurrentUser, db: &Db) -> Api<()> {
        self.check_permission(user, "rwd")?;

        self.do_set_deleted(db).await?;
        Ok(())
    }
}

#[rocket::async_trait]
impl Ugc for Post {
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
        self.room_id != 42
    }
    async fn do_set_deleted(&mut self, db: &Db) -> Api<()> {
        update!(*self, posts, db, { is_deleted, to true });
        Ok(())
    }
}

#[rocket::async_trait]
impl Ugc for Comment {
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
    async fn do_set_deleted(&mut self, db: &Db) -> Api<()> {
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
pub mod reaction;
pub mod search;
pub mod systemlog;
pub mod upload;
pub mod vote;
