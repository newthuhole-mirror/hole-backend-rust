use crate::rds_conn::RdsConn;
use redis::{AsyncCommands, RedisResult};

pub struct Attention {
    key: String,
    rconn: RdsConn,
}

impl Attention {
    pub fn init(namehash: &str, rconn: RdsConn) -> Self {
        Attention {
            key: format!("hole_v2:attention:{}", namehash),
            rconn: rconn,
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
