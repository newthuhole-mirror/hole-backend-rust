use crate::api::{Api, CurrentUser, PolicyError};
use crate::random_hasher::random_string;
use crate::rds_conn::RdsConn;
use chrono::{offset::Local, DateTime};
use futures_util::stream::StreamExt;
use redis::{AsyncCommands, RedisResult};
use rocket::serde::json::serde_json;
use rocket::serde::{Deserialize, Serialize};

macro_rules! init {
    () => {
        pub fn init(rconn: &RdsConn) -> Self {
            Self {
                rconn: rconn.clone(),
            }
        }
    };
    ($ktype:ty, $formatter:literal) => {
        pub fn init(k: $ktype, rconn: &RdsConn) -> Self {
            Self {
                key: format!($formatter, k),
                rconn: rconn.clone(),
            }
        }
    };
    ($k1type:ty, $k2type:ty, $formatter:literal) => {
        pub fn init(k1: $k1type, k2: $k2type, rconn: &RdsConn) -> Self {
            Self {
                key: format!($formatter, k1, k2),
                rconn: rconn.clone(),
            }
        }
    };
}

macro_rules! has {
    ($vtype:ty) => {
        pub async fn has(&mut self, v: $vtype) -> RedisResult<bool> {
            self.rconn.sismember(&self.key, v).await
        }
    };
}

macro_rules! add {
    ($vtype:ty) => {
        pub async fn add(&mut self, v: $vtype) -> RedisResult<usize> {
            self.rconn.sadd(&self.key, v).await
        }
    };
}

macro_rules! rem {
    ($vtype:ty) => {
        pub async fn rem(&mut self, v: $vtype) -> RedisResult<usize> {
            self.rconn.srem(&self.key, v).await
        }
    };
}

macro_rules! clear_all {
    ($pattern:literal) => {
        pub async fn clear_all(rconn: &mut RdsConn) {
            let keys: Vec<String> = rconn
                .scan_match::<&str, String>($pattern)
                .await
                .unwrap()
                .collect::<Vec<String>>()
                .await;

            rconn
                .del(keys)
                .await
                .unwrap_or_else(|e| warn!("clear all fail, pattern: {} , {}", $pattern, e));
        }
    };
}

const KEY_SYSTEMLOG: &str = "hole_v2:systemlog_list";
const KEY_BANNED_USERS: &str = "hole_v2:banned_user_hash_list";
const KEY_BLOCKED_COUNTER: &str = "hole_v2:blocked_counter";
const KEY_CUSTOM_TITLE: &str = "hole_v2:title";
const CUSTOM_TITLE_KEEP_TIME: usize = 7 * 24 * 60 * 60;
macro_rules! KEY_TITLE_SECRET {
    ($title: expr) => {
        format!("hole_v2:title_secret:{}", $title)
    };
}
const KEY_AUTO_BLOCK_RANK: &str = "hole_v2:auto_block_rank"; // rank * 5: 自动过滤的拉黑数阈值
const KEY_ANNOUNCEMENT: &str = "hole_v2:announcement";
const KEY_CANDIDATE: &str = "hole_v2:candidate";
const KEY_ADMIN: &str = "hole_v2:admin";

const SYSTEMLOG_MAX_LEN: isize = 1000;

pub struct Attention {
    key: String,
    rconn: RdsConn,
}

impl Attention {
    init!(&str, "hole_v2:attention:{}");

    add!(i32);

    has!(i32);

    clear_all!("hole_v2:attention:*");

    pub async fn remove(&mut self, pid: i32) -> RedisResult<()> {
        self.rconn.srem(&self.key, pid).await
    }

    pub async fn all(&mut self) -> RedisResult<Vec<i32>> {
        self.rconn.smembers(&self.key).await
    }
}

pub struct Reaction {
    key: String,
    rconn: RdsConn,
}

impl Reaction {
    init!(i32, i32, "hole_v2:reaction:{}:{}");

    add!(&str);

    rem!(&str);

    has!(&str);
}

pub async fn get_user_post_reaction_status(
    rconn: &RdsConn,
    pid: i32,
    namehash: &str,
) -> RedisResult<i32> {
    for rt in [-1, 1] {
        if Reaction::init(pid, rt, rconn).has(namehash).await? {
            return Ok(rt);
        }
    }
    Ok(0)
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
pub enum LogType {
    AdminDelete,
    Report,
    Ban,
}

/*
impl LogType {
    pub fn contains_ugc(&self) -> bool {
        match self {
            Self::Report => true,
            _ => false,
        }
    }
}
*/

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
pub struct Systemlog {
    pub user_hash: String,
    pub action_type: LogType,
    pub target: String,
    pub detail: String,
    pub time: DateTime<Local>,
}

impl Systemlog {
    pub async fn create(&self, rconn: &RdsConn) -> RedisResult<()> {
        let mut rconn = rconn.clone();
        if rconn.llen::<&str, isize>(KEY_SYSTEMLOG).await? > SYSTEMLOG_MAX_LEN {
            rconn.ltrim(KEY_SYSTEMLOG, 0, SYSTEMLOG_MAX_LEN - 1).await?;
        }
        rconn
            .lpush(KEY_SYSTEMLOG, serde_json::to_string(&self).unwrap())
            .await
    }

    pub async fn get_list(rconn: &RdsConn, limit: isize) -> RedisResult<Vec<Self>> {
        let rds_result = rconn
            .clone()
            .lrange::<&str, Vec<String>>(KEY_SYSTEMLOG, 0, limit)
            .await?;
        Ok(rds_result
            .iter()
            .map(|s| serde_json::from_str(s).unwrap())
            .collect())
    }
}

pub struct BannedUsers;

impl BannedUsers {
    pub async fn add(rconn: &RdsConn, namehash: &str) -> RedisResult<()> {
        rconn
            .clone()
            .sadd::<&str, &str, ()>(KEY_BANNED_USERS, namehash)
            .await
    }

    pub async fn has(rconn: &RdsConn, namehash: &str) -> RedisResult<bool> {
        rconn.clone().sismember(KEY_BANNED_USERS, namehash).await
    }

    pub async fn clear(rconn: &mut RdsConn) -> RedisResult<()> {
        rconn.del(KEY_BANNED_USERS).await
    }
}

pub struct BlockedUsers {
    pub key: String,
    rconn: RdsConn,
}

impl BlockedUsers {
    init!(i32, "hole_v2:blocked_users:{}");

    add!(&str);

    has!(&str);

    clear_all!("hole_v2:blocked_users:*");

    pub async fn check_if_block(
        rconn: &RdsConn,
        user: &CurrentUser,
        hash: &str,
    ) -> RedisResult<bool> {
        Ok(match user.id {
            Some(id) => BlockedUsers::init(id, rconn).has(hash).await?,
            None => false,
        } || BlockCounter::get_count(rconn, hash).await?.unwrap_or(0)
            >= i32::from(user.auto_block_rank) * 5)
    }
}

pub struct BlockCounter;

impl BlockCounter {
    pub async fn count_incr(rconn: &RdsConn, namehash: &str) -> RedisResult<i32> {
        rconn.clone().hincr(KEY_BLOCKED_COUNTER, namehash, 1).await
    }

    pub async fn get_count(rconn: &RdsConn, namehash: &str) -> RedisResult<Option<i32>> {
        rconn.clone().hget(KEY_BLOCKED_COUNTER, namehash).await
    }
}

pub struct CustomTitle;

impl CustomTitle {
    async fn gen_and_set_secret(rconn: &RdsConn, title: &str) -> RedisResult<String> {
        let secret = random_string(8);
        rconn
            .clone()
            .set_ex(KEY_TITLE_SECRET!(&title), &secret, CUSTOM_TITLE_KEEP_TIME)
            .await?;
        Ok(secret)
    }

    // return false if title exits
    pub async fn set(rconn: &RdsConn, namehash: &str, title: &str, secret: &str) -> Api<String> {
        let mut rconn = rconn.clone();
        if rconn.hexists(KEY_CUSTOM_TITLE, title).await? {
            Err(PolicyError::TitleUsed)?
        } else {
            let ori_secret: Option<String> = rconn.get(KEY_TITLE_SECRET!(title)).await?;
            if ori_secret.is_none() {
                clear_title_from_admins(&rconn, title).await?;
            }
            ori_secret
                .map_or(Some(()), |s| (s.eq(&secret).then_some(())))
                .ok_or(PolicyError::TitleProtected)?;

            let old_title: Option<String> = rconn.hget(KEY_CUSTOM_TITLE, namehash).await?;
            if let Some(t) = old_title {
                clear_title_from_admins(&rconn, &t).await?;
            }
            rconn.hset(KEY_CUSTOM_TITLE, namehash, title).await?;
            rconn.hset(KEY_CUSTOM_TITLE, title, namehash).await?;
            Ok(Self::gen_and_set_secret(&rconn, title).await?)
        }
    }

    pub async fn get(
        rconn: &RdsConn,
        namehash: &str,
    ) -> RedisResult<(Option<String>, Option<String>)> {
        let t: Option<String> = rconn.clone().hget(KEY_CUSTOM_TITLE, namehash).await?;
        Ok(if let Some(title) = t {
            let s: Option<String> = rconn.clone().get(KEY_TITLE_SECRET!(title)).await?;
            let secret = if let Some(ss) = s {
                rconn
                    .clone()
                    .expire(KEY_TITLE_SECRET!(title), CUSTOM_TITLE_KEEP_TIME)
                    .await?;
                ss
            } else {
                Self::gen_and_set_secret(rconn, &title).await?
            };
            (Some(title), Some(secret))
        } else {
            (None, None)
        })
    }

    pub async fn clear(rconn: &mut RdsConn) -> RedisResult<()> {
        rconn.del(KEY_CUSTOM_TITLE).await
    }
}

pub struct AutoBlockRank;

impl AutoBlockRank {
    pub async fn set(rconn: &RdsConn, namehash: &str, rank: u8) -> RedisResult<usize> {
        rconn
            .clone()
            .hset(KEY_AUTO_BLOCK_RANK, namehash, rank)
            .await
    }

    pub async fn get(rconn: &RdsConn, namehash: &str) -> RedisResult<u8> {
        let rank: Option<u8> = rconn.clone().hget(KEY_AUTO_BLOCK_RANK, namehash).await?;
        Ok(rank.unwrap_or(4))
    }

    pub async fn clear(rconn: &mut RdsConn) -> RedisResult<()> {
        rconn.del(KEY_AUTO_BLOCK_RANK).await
    }
}

pub struct PollOption {
    key: String,
    rconn: RdsConn,
}

impl PollOption {
    init!(i32, "hole_thu:poll_opts:{}");

    pub async fn set_list(&mut self, v: &Vec<String>) -> RedisResult<()> {
        self.rconn.del(&self.key).await?;
        self.rconn.rpush(&self.key, v).await
    }

    pub async fn get_list(&mut self) -> RedisResult<Vec<String>> {
        self.rconn.lrange(&self.key, 0, -1).await
    }
}

pub struct PollVote {
    key: String,
    rconn: RdsConn,
}

impl PollVote {
    init!(i32, usize, "hole_thu:poll_votes:{}:{}");

    add!(&str);

    has!(&str);

    pub async fn count(&mut self) -> RedisResult<usize> {
        self.rconn.scard(&self.key).await
    }
}

pub async fn clear_outdate_redis_data(rconn: &mut RdsConn) {
    BannedUsers::clear(rconn).await.unwrap();
    CustomTitle::clear(rconn).await.unwrap();
    AutoBlockRank::clear(rconn).await.unwrap();
    Attention::clear_all(rconn).await;
    BlockedUsers::clear_all(rconn).await;
}

pub async fn get_announcement(rconn: &RdsConn) -> RedisResult<Option<String>> {
    rconn.clone().get(KEY_ANNOUNCEMENT).await
}

pub async fn is_elected_candidate(rconn: &RdsConn, title: &Option<String>) -> RedisResult<bool> {
    if let Some(t) = title {
        rconn.clone().sismember(KEY_CANDIDATE, t).await
    } else {
        Ok(false)
    }
}

pub async fn is_elected_admin(rconn: &RdsConn, title: &Option<String>) -> RedisResult<bool> {
    if let Some(t) = title {
        rconn.clone().sismember(KEY_ADMIN, t).await
    } else {
        Ok(false)
    }
}

pub async fn get_admin_list(rconn: &RdsConn) -> RedisResult<Vec<String>> {
    rconn.clone().smembers(KEY_ADMIN).await
}

pub async fn get_candidate_list(rconn: &RdsConn) -> RedisResult<Vec<String>> {
    rconn.clone().smembers(KEY_CANDIDATE).await
}

pub async fn clear_title_from_admins(rconn: &RdsConn, title: &str) -> RedisResult<()> {
    let mut rconn = rconn.clone();
    rconn.srem(KEY_CANDIDATE, title).await?;
    rconn.srem(KEY_ADMIN, title).await?;
    Ok(())
}

pub(crate) use clear_all;
pub(crate) use init;
