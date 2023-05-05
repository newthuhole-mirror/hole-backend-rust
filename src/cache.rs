use crate::api::{Api, CurrentUser};
use crate::db_conn::Db;
use crate::models::{Comment, Post, User};
use crate::rds_conn::RdsConn;
use crate::rds_models::{clear_all, init, BlockedUsers};
use rand::Rng;
use redis::{AsyncCommands, RedisError, RedisResult};
use rocket::serde::json::serde_json;
// can use rocket::serde::json::to_string in master version
use futures_util::stream::StreamExt;
use rocket::futures::future;
use std::collections::HashMap;

const KEY_USER_COUNT: &str = "hole_v2:cache:user_count";
const USER_COUNT_EXPIRE_TIME: usize = 5 * 60;

const INSTANCE_EXPIRE_TIME: usize = 60 * 60;

const MIN_LENGTH: isize = 200;
const MAX_LENGTH: isize = 900;
const CUT_LENGTH: isize = 100;

macro_rules! post_cache_key {
    ($id: expr) => {
        format!("hole_v2:cache:post:{}:v2", $id)
    };
}

pub struct PostCache {
    rconn: RdsConn,
}

impl PostCache {
    init!();

    clear_all!("hole_v2:cache::post:*:v2");

    pub async fn sets(&mut self, ps: &[&Post]) {
        if ps.is_empty() {
            return;
        }
        let kvs: Vec<(String, String)> = ps
            .iter()
            .map(|p| (post_cache_key!(p.id), serde_json::to_string(p).unwrap()))
            .collect();
        self.rconn.mset(&kvs).await.unwrap_or_else(|e| {
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
                warn!("try to get post cache, connect rds failed, {}", e);
                None
            });

        rds_result.and_then(|s| {
            serde_json::from_str(&s).unwrap_or_else(|e| {
                warn!("get post cache, decode failed {}, {}", e, s);
                None
            })
        })
    }

    pub async fn gets(&mut self, pids: &[i32]) -> Vec<Option<Post>> {
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
                        warn!("try to get posts cache, connect rds failed, {}", e);
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
    init!(i32, "hole_v2:cache:post_comments:{}");

    pub async fn set(&mut self, cs: &[Comment]) {
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

pub struct PostListCache {
    key: String,
    mode: u8,
    rconn: RdsConn,
    length: isize,
}

impl PostListCache {
    pub fn init(room_id: Option<i32>, mode: u8, rconn: &RdsConn) -> Self {
        Self {
            key: format!(
                "hole_v2:cache:post_list:{}:{}",
                match room_id {
                    Some(i) => i.to_string(),
                    None => "".to_owned(),
                },
                &mode
            ),
            mode,
            rconn: rconn.clone(),
            length: 0,
        }
    }

    async fn set_and_check_length(&mut self) {
        let mut l = self.rconn.zcard(&self.key).await.unwrap();
        if l > MAX_LENGTH {
            self.rconn
                .zremrangebyrank::<&String, ()>(&self.key, MAX_LENGTH - CUT_LENGTH, -1)
                .await
                .unwrap_or_else(|e| {
                    warn!("cut list cache failed, {}, {}", e, &self.key);
                });
            l = MIN_LENGTH;
        }
        self.length = l;
    }

    pub async fn need_fill(&mut self) -> bool {
        self.set_and_check_length().await;
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
                4 => (-p.n_attentions).into(),
                _ => panic!("wrong mode"),
            },
            p.id,
        )
    }

    pub async fn fill(&mut self, ps: &[Post]) {
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
        if p.is_deleted || (self.mode > 0 && p.is_reported) {
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

    pub async fn clear(&mut self) {
        self.rconn.del(&self.key).await.unwrap_or_else(|e| {
            warn!("clear post list cache failed, {}", e);
        });
    }
}

pub struct UserCache {
    key: String,
    rconn: RdsConn,
}

impl UserCache {
    init!(&str, "hole_v2:cache:user:{}");

    clear_all!("hole_v2:cache:user:*");

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

pub struct BlockDictCache {
    key: String,
    rconn: RdsConn,
}

impl BlockDictCache {
    // namehash, pid
    init!(&str, i32, "hole_v2:cache:block_dict:{}:{}");

    pub async fn get_or_create(
        &mut self,
        user: &CurrentUser,
        hash_list: &[&String],
    ) -> RedisResult<HashMap<String, bool>> {
        let mut block_dict = self
            .rconn
            .hgetall::<&String, HashMap<String, bool>>(&self.key)
            .await?;

        //dbg!(&self.key, &block_dict);

        let missing: Vec<(String, bool)> =
            future::try_join_all(hash_list.iter().filter_map(|hash| {
                (!block_dict.contains_key(&hash.to_string())).then_some(async {
                    Ok::<(String, bool), RedisError>((
                        hash.to_string(),
                        BlockedUsers::check_if_block(&self.rconn, user, hash).await?,
                    ))
                })
            }))
            .await?;

        if !missing.is_empty() {
            self.rconn.hset_multiple(&self.key, &missing).await?;
            self.rconn.expire(&self.key, INSTANCE_EXPIRE_TIME).await?;
            block_dict.extend(missing.into_iter());
        }

        //dbg!(&block_dict);

        Ok(block_dict)
    }

    pub async fn clear(&mut self) -> RedisResult<()> {
        self.rconn.del(&self.key).await
    }
}

pub async fn cached_user_count(db: &Db, rconn: &mut RdsConn) -> Api<i64> {
    let cnt: Option<i64> = rconn.get(KEY_USER_COUNT).await?;
    if let Some(x) = cnt {
        Ok(x)
    } else {
        let x = User::get_count(db).await?;
        rconn
            .set_ex(KEY_USER_COUNT, x, USER_COUNT_EXPIRE_TIME)
            .await?;
        Ok(x)
    }
}
