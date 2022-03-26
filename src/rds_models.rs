use crate::rds_conn::RdsConn;
use chrono::{offset::Local, DateTime};
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
    ($ktype:ty, $formatter:expr) => {
        pub fn init(k: $ktype, rconn: &RdsConn) -> Self {
            Self {
                key: format!($formatter, k),
                rconn: rconn.clone(),
            }
        }
    };
    ($k1type:ty, $k2type:ty, $formatter:expr) => {
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
        pub async fn add(&mut self, v: $vtype) -> RedisResult<()> {
            self.rconn.sadd(&self.key, v).await
        }
    };
}

const KEY_SYSTEMLOG: &str = "hole_v2:systemlog_list";
const KEY_BANNED_USERS: &str = "hole_v2:banned_user_hash_list";
const KEY_BLOCKED_COUNTER: &str = "hole_v2:blocked_counter";
const KEY_DANGEROUS_USERS: &str = "hole_thu:dangerous_users"; //兼容一下旧版
const KEY_CUSTOM_TITLE: &str = "hole_v2:title";

const SYSTEMLOG_MAX_LEN: isize = 1000;
pub const BLOCK_THRESHOLD: i32 = 10;

pub struct Attention {
    key: String,
    rconn: RdsConn,
}

impl Attention {
    init!(&str, "hole_v2:attention:{}");

    add!(i32);

    has!(i32);

    pub async fn remove(&mut self, pid: i32) -> RedisResult<()> {
        self.rconn.srem(&self.key, pid).await
    }

    pub async fn all(&mut self) -> RedisResult<Vec<i32>> {
        self.rconn.smembers(&self.key).await
    }

    // TODO: clear all
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
pub enum LogType {
    AdminDelete,
    Report,
    Ban,
}

impl LogType {
    pub fn contains_ugc(&self) -> bool {
        match self {
            Self::Report => true,
            _ => false,
        }
    }
}

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

    pub async fn clear(rconn: &RdsConn) -> RedisResult<()> {
        rconn.clone().del(KEY_BANNED_USERS).await
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

    pub async fn check_blocked(
        rconn: &RdsConn,
        viewer_id: Option<i32>,
        viewer_hash: &str,
        author_hash: &str,
    ) -> RedisResult<bool> {
        Ok(match viewer_id {
            Some(id) => Self::init(id, rconn).has(author_hash).await?,
            None => false,
        } || (DangerousUser::has(rconn, author_hash).await?
            && !DangerousUser::has(rconn, viewer_hash).await?))
    }
}

pub struct BlockCounter;

impl BlockCounter {
    pub async fn count_incr(rconn: &RdsConn, namehash: &str) -> RedisResult<i32> {
        rconn.clone().hincr(KEY_BLOCKED_COUNTER, namehash, 1).await
    }

    pub async fn get_count(rconn: &RdsConn, namehash: &str) -> RedisResult<i32> {
        rconn.clone().hget(KEY_BLOCKED_COUNTER, namehash).await
    }
}

pub struct DangerousUser;

impl DangerousUser {
    pub async fn add(rconn: &RdsConn, namehash: &str) -> RedisResult<()> {
        rconn
            .clone()
            .sadd::<&str, &str, ()>(KEY_DANGEROUS_USERS, namehash)
            .await
    }

    pub async fn has(rconn: &RdsConn, namehash: &str) -> RedisResult<bool> {
        rconn.clone().sismember(KEY_DANGEROUS_USERS, namehash).await
    }
}

pub struct CustomTitle;

impl CustomTitle {
    // return false if title exits
    pub async fn set(rconn: &RdsConn, namehash: &str, title: &str) -> RedisResult<bool> {
        let mut rconn = rconn.clone();
        if rconn.hexists(KEY_CUSTOM_TITLE, title).await? {
            Ok(false)
        } else {
            rconn.hset(KEY_CUSTOM_TITLE, namehash, title).await?;
            rconn.hset(KEY_CUSTOM_TITLE, title, namehash).await?;
            Ok(true)
        }
    }

    pub async fn get(rconn: &RdsConn, namehash: &str) -> RedisResult<Option<String>> {
        rconn.clone().hget(KEY_CUSTOM_TITLE, namehash).await
    }

    pub async fn clear(rconn: &RdsConn) -> RedisResult<()> {
        rconn.clone().del(KEY_CUSTOM_TITLE).await
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

pub(crate) use init;
