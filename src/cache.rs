use crate::models::{Comment, Post, User};
use crate::rds_conn::RdsConn;
use redis::AsyncCommands;
use rocket::serde::json::serde_json;
// can use rocket::serde::json::to_string in master version

const INSTANCE_EXPIRE_TIME: usize = 60 * 60;
macro_rules! post_cache_key {
    ($id: expr) => {
        format!("hole_v2:cache:post:{}", $id)
    };
}

pub struct PostCache {
    rconn: RdsConn,
}

impl PostCache {
    pub fn init(rconn: &RdsConn) -> Self {
        PostCache {
            rconn: rconn.clone(),
        }
    }

    pub async fn sets(&mut self, ps: &Vec<&Post>) {
        if ps.is_empty() {
            return;
        }
        let kvs: Vec<(String, String)> = ps
            .iter()
            .map(|p| (
                post_cache_key!(p.id), 
                serde_json::to_string(p).unwrap(),
            ) ).collect();
        dbg!(&kvs);
        let ret = self.rconn
            .set_multiple(&kvs)
            .await
            .unwrap_or_else(|e| {
                warn!("set post cache failed: {}", e);
                "x".to_string()
            });
        dbg!(ret);
    }

    pub async fn get(&mut self, pid: &i32) -> Option<Post> {
        let key = post_cache_key!(pid);
        let rds_result: Option<String> = self
            .rconn
            .get::<String, Option<String>>(key)
            .await
            .unwrap_or_else(|e| {
                warn!("try to get post cache, connect rds fail, {}", e);
                None
            });

        rds_result.and_then(|s| {
            serde_json::from_str(&s).unwrap_or_else(|e| {
                warn!("get post cache, decode failed {}, {}", e, s);
                None
            })
        })
    }

    pub async fn gets(&mut self, pids: &Vec<i32>) -> Vec<Option<Post>> {
        // 长度为1时会走GET而非MGET，返回值格式不兼容。愚蠢的设计。
        match pids.len() {
            0 => vec![],
            1 => vec![self.get(&pids[0]).await],
            _ => {
                let ks: Vec<String> = pids.iter().map(|pid| post_cache_key!(pid)).collect();
                // dbg!(&ks);
                // Vec is single arg, while &Vec is not. Seems a bug.
                let rds_result: Vec<Option<String>> = self
                    .rconn
                    .get::<Vec<String>, Vec<Option<String>>>(ks)
                    .await
                    .unwrap_or_else(|e| {
                        warn!("try to get posts cache, connect rds fail, {}", e);
                        vec![None; pids.len()]
                    });
                // dbg!(&rds_result);

                // 定期热度衰减的时候会清空缓存，这里设不设置过期时间影响不大

                rds_result
                    .into_iter()
                    .map(|x| {
                        // dbg!(&x);
                        x.and_then(|s| {
                            serde_json::from_str(&s).unwrap_or_else(|e| {
                                warn!("get post cache, decode failed {}, {}", e, s);
                                None
                            })
                        })
                    })
                    .collect()
            }
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
            debug!("hint user cache");
            self.rconn
                .expire::<&String, bool>(&self.key, INSTANCE_EXPIRE_TIME)
                .await
                .unwrap_or_else(|e| {
                    warn!(
                        "get user cache, set new expire failed: {}, {}, {} ",
                        e, &self.key, &s
                    );
                    false
                });
            serde_json::from_str(&s).unwrap_or_else(|e| {
                warn!("get user cache, decode failed {}, {}", e, s);
                None
            })
        } else {
            None
        }
    }
}
