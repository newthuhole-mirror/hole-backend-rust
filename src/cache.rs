use crate::models::{Comment, Post, User};
use crate::rds_conn::RdsConn;
use rand::Rng;
use redis::AsyncCommands;
use rocket::serde::json::serde_json;
// can use rocket::serde::json::to_string in master version

const INSTANCE_EXPIRE_TIME: usize = 60 * 60;

const MIN_LENGTH: isize = 200;
const MAX_LENGTH: isize = 900;

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
            .map(|p| (post_cache_key!(p.id), serde_json::to_string(p).unwrap()))
            .collect();
        self.rconn.set_multiple(&kvs).await.unwrap_or_else(|e| {
            warn!("set post cache failed: {}", e);
            dbg!(&kvs);
        });
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

pub struct PostCommentCache {
    key: String,
    rconn: RdsConn,
}

impl PostCommentCache {
    pub fn init(pid: i32, rconn: &RdsConn) -> Self {
        PostCommentCache {
            key: format!("hole_v2:cache:post_comments:{}", pid),
            rconn: rconn.clone(),
        }
    }

    pub async fn set(&mut self, cs: &Vec<Comment>) {
        self.rconn
            .set_ex(
                &self.key,
                serde_json::to_string(cs).unwrap(),
                INSTANCE_EXPIRE_TIME,
            )
            .await
            .unwrap_or_else(|e| {
                warn!("set comments cache failed: {}", e);
                dbg!(cs);
            })
    }

    pub async fn get(&mut self) -> Option<Vec<Comment>> {
        let rds_result = self.rconn.get::<&String, String>(&self.key).await;
        // dbg!(&rds_result);
        if let Ok(s) = rds_result {
            self.rconn
                .expire::<&String, bool>(&self.key, INSTANCE_EXPIRE_TIME)
                .await
                .unwrap_or_else(|e| {
                    warn!(
                        "get comments cache, set new expire failed: {}, {}, {} ",
                        e, &self.key, &s
                    );
                    false
                });
            serde_json::from_str(&s).unwrap_or_else(|e| {
                warn!("get comments cache, decode failed {}, {}", e, s);
                None
            })
        } else {
            None
        }
    }

    pub async fn clear(&mut self) {
        self.rconn.del(&self.key).await.unwrap_or_else(|e| {
            warn!("clear commenrs cache fail, {}", e);
        });
    }
}

pub struct PostListCommentCache {
    key: String,
    mode: u8,
    rconn: RdsConn,
    length: isize,
}

impl PostListCommentCache {
    pub async fn init(mode: u8, rconn: &RdsConn) -> Self {
        let mut cacher = PostListCommentCache {
            key: format!("hole_v2:cache:post_list:{}", &mode),
            mode: mode,
            rconn: rconn.clone(),
            length: 0,
        };
        cacher.set_and_check_length().await;
        cacher
    }

    async fn set_and_check_length(&mut self) {
        let mut l = self.rconn.zcard(&self.key).await.unwrap();
        if l > MAX_LENGTH {
            self.rconn
                .zremrangebyrank::<&String, ()>(&self.key, MIN_LENGTH, -1)
                .await
                .unwrap_or_else(|e| {
                    warn!("cut list cache failed, {}, {}", e, &self.key);
                });
            l = MIN_LENGTH;
        }
        self.length = l;
    }

    pub fn need_fill(&self) -> bool {
        self.length < MIN_LENGTH
    }

    pub fn i64_len(&self) -> i64 {
        self.length.try_into().unwrap()
    }

    pub fn i64_minlen(&self) -> i64 {
        MIN_LENGTH.try_into().unwrap()
    }

    fn p2pair(&self, p: &Post) -> (i64, i32) {
        (
            match self.mode {
                0 => (-p.id).into(),
                1 => -p.last_comment_time.timestamp(),
                2 => (-p.hot_score).into(),
                3 => rand::thread_rng().gen_range(0..i64::MAX),
                _ => panic!("wrong mode"),
            },
            p.id,
        )
    }

    pub async fn fill(&mut self, ps: &Vec<Post>) {
        let items: Vec<(i64, i32)> = ps.iter().map(|p| self.p2pair(p)).collect();
        self.rconn
            .zadd_multiple(&self.key, &items)
            .await
            .unwrap_or_else(|e| {
                warn!("fill list cache failed, {} {}", e, &self.key);
            });

        self.set_and_check_length().await;
    }

    pub async fn put(&mut self, p: &Post) {
        // 其他都是加到最前面的，但热榜不是。可能导致MIN_LENGTH到MAX_LENGTH之间的数据不可靠
        // 影响不大，先不管了
        if p.is_deleted {
            self.rconn.zrem(&self.key, p.id).await.unwrap_or_else(|e| {
                warn!(
                    "remove from list cache failed, {} {} {}",
                    e, &self.key, p.id
                );
            });
        } else {
            let (s, m) = self.p2pair(p);
            self.rconn.zadd(&self.key, m, s).await.unwrap_or_else(|e| {
                warn!(
                    "put into list cache failed, {} {} {} {}",
                    e, &self.key, m, s
                );
            });
        }
    }

    pub async fn get_pids(&mut self, start: i64, limit: i64) -> Vec<i32> {
        self.rconn
            .zrange(
                &self.key,
                start.try_into().unwrap(),
                (start + limit - 1).try_into().unwrap(),
            )
            .await
            .unwrap()
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
                warn!("set user cache failed: {}", e);
                dbg!(u);
            })
    }

    pub async fn get(&mut self) -> Option<User> {
        let rds_result = self.rconn.get::<&String, String>(&self.key).await;
        if let Ok(s) = rds_result {
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
