use crate::models::{Comment, Post};
use crate::rds_conn::RdsConn;
use redis::{AsyncCommands, RedisResult};
use rocket::serde::json::serde_json;
// can use rocket::serde::json::to_string in master version

const INSTANCE_EXPIRE_TIME: usize = 60 * 60;

pub struct Attention {
    key: String,
    rconn: RdsConn,
}

impl Attention {
    pub fn init(namehash: &str, rconn: &RdsConn) -> Self {
        Attention {
            key: format!("hole_v2:attention:{}", namehash),
            rconn: rconn.clone(),
        }
    }

    pub async fn add(&mut self, pid: i32) -> RedisResult<()> {
        self.rconn.sadd(&self.key, pid).await
    }

    pub async fn remove(&mut self, pid: i32) -> RedisResult<()> {
        self.rconn.srem(&self.key, pid).await
    }

    pub async fn has(&mut self, pid: i32) -> RedisResult<bool> {
        self.rconn.sismember(&self.key, pid).await
    }

    pub async fn all(&mut self) -> RedisResult<Vec<i32>> {
        self.rconn.smembers(&self.key).await
    }
}

pub struct PostCache {
    key: String,
    rconn: RdsConn,
}

impl PostCache {
    pub fn init(pid: &i32, rconn: &RdsConn) -> Self {
        PostCache {
            key: format!("hole_v2:cache:post:{}", pid),
            rconn: rconn.clone(),
        }
    }

    pub async fn set(&mut self, p: &Post) {
        self.rconn
            .set_ex(
                &self.key,
                serde_json::to_string(p).unwrap(),
                INSTANCE_EXPIRE_TIME,
            )
            .await
            .unwrap_or_else(|e| {
                warn!("set post cache failed: {}, {}", e, p.id);
            })
    }

    pub async fn get(&mut self) -> Option<Post> {
        let rds_result = self.rconn.get::<&String, String>(&self.key).await;
        if let Ok(s) = rds_result {
            debug!("hint post cache: {}", &s);
            self.rconn
                .expire::<&String, bool>(&self.key, INSTANCE_EXPIRE_TIME)
                .await
                .unwrap_or_else(|e| {
                    warn!(
                        "get post cache, set new expire failed: {}, {}, {} ",
                        e, &self.key, &s
                    );
                    false
                });
            serde_json::from_str(&s).unwrap_or_else(|e| {
                warn!("get post cache, decode failed {}, {}", e, s);
                None
            })
        } else {
            None
        }
    }
}
