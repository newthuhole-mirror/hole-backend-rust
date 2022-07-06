use crate::api::CurrentUser;
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
        pub async fn add(&mut self, v: $vtype) -> RedisResult<usize> {
            self.rconn.sadd(&self.key, v).await
        }
    };
}

const KEY_SYSTEMLOG: &str = "hole_v2:systemlog_list";
const KEY_BANNED_USERS: &str = "hole_v2:banned_user_hash_list";
const KEY_BLOCKED_COUNTER: &str = "hole_v2:blocked_counter";
const KEY_CUSTOM_TITLE: &str = "hole_v2:title";
const KEY_AUTO_BLOCK_RANK: &str = "hole_v2:auto_block_rank"; // rank * 5: 自动过滤的拉黑数阈值

const SYSTEMLOG_MAX_LEN: isize = 1000;

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

    pub async fn clear_all(rconn: &RdsConn) {
        let mut rconn = rconn.clone();
        let mut keys = rconn
            .scan_match::<&str, String>("hole_v2:attention:*")
            .await
            .unwrap();

        let mut ks_for_del = Vec::new();
        while let Some(key) = keys.next_item().await {
            ks_for_del.push(key);
        }
        if ks_for_del.is_empty() {
            return;
        }
        rconn
            .del(ks_for_del)
            .await
            .unwrap_or_else(|e| warn!("clear all post cache fail, {}", e));
    }
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
    pub async fn count_incr(rconn: &RdsConn, namehash: &str) -> RedisResult<usize> {
        rconn.clone().hincr(KEY_BLOCKED_COUNTER, namehash, 1).await
    }

    pub async fn get_count(rconn: &RdsConn, namehash: &str) -> RedisResult<Option<i32>> {
        rconn.clone().hget(KEY_BLOCKED_COUNTER, namehash).await
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

    pub async fn clear(rconn: &RdsConn) -> RedisResult<()> {
        rconn.clone().del(KEY_AUTO_BLOCK_RANK).await
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

pub async fn clear_outdate_redis_data(rconn: &RdsConn) {
    BannedUsers::clear(rconn).await.unwrap();
    CustomTitle::clear(rconn).await.unwrap();
    AutoBlockRank::clear(rconn).await.unwrap();
    Attention::clear_all(rconn).await;
}

pub(crate) use init;
