use crate::models::{Comment, Post, User};
use crate::rds_conn::RdsConn;
use redis::AsyncCommands;
use rocket::serde::json::serde_json;
// can use rocket::serde::json::to_string in master version

const INSTANCE_EXPIRE_TIME: usize = 60 * 60;

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
            debug!("hint user cache");
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

pub struct UserCache {
    key: String,
    rconn: RdsConn,
}

impl UserCache {
    pub fn init(token: &str, rconn: &RdsConn) -> Self {
        UserCache {
            key: format!("hole_v2:cache:user:{}", token),
            rconn: rconn.clone(),
        }
    }

    pub async fn set(&mut self, u: &User) {
        self.rconn
            .set_ex(
                &self.key,
                serde_json::to_string(u).unwrap(),
                INSTANCE_EXPIRE_TIME,
            )
            .await
            .unwrap_or_else(|e| {
                warn!("set user cache failed: {}, {}, {}", e, u.id, u.name);
            })
    }

    pub async fn get(&mut self) -> Option<User> {
        let rds_result = self.rconn.get::<&String, String>(&self.key).await;
        if let Ok(s) = rds_result {
            debug!("hint post cache");
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
