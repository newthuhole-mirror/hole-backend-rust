#![allow(clippy::all)]

use crate::cache::*;
use crate::db_conn::{Conn, Db};
use crate::libs::diesel_logger::LoggingConnection;
use crate::rds_conn::RdsConn;
use crate::schema::*;
use chrono::{offset::Utc, DateTime};
use diesel::dsl::any;
use diesel::sql_types::*;
use diesel::{
    insert_into, BoolExpressionMethods, ExpressionMethods, QueryDsl, QueryResult, RunQueryDsl,
    TextExpressionMethods,
};
use rocket::futures::{future, join};
use rocket::serde::{Deserialize, Serialize};
use std::collections::HashMap;

no_arg_sql_function!(RANDOM, (), "Represents the sql RANDOM() function");
sql_function!(fn floor(x: Float) -> Int4);
sql_function!(fn float4(x: Int4) -> Float);

macro_rules! _get {
    ($table:ident) => {
        async fn _get(db: &Db, id: i32) -> QueryResult<Self> {
            let pid = id;
            db.run(move |c| $table::table.find(pid).first(with_log!((c))))
                .await
        }
    };
}

macro_rules! _get_multi {
    ($table:ident) => {
        async fn _get_multi(db: &Db, ids: Vec<i32>) -> QueryResult<Vec<Self>> {
            if ids.is_empty() {
                return Ok(vec![]);
            }
            // eq(any()) is only for postgres
            db.run(move |c| {
                $table::table
                    .filter($table::id.eq(any(ids)))
                    .filter($table::is_deleted.eq(false))
                    .load(with_log!(c))
            })
            .await
        }
    };
}

macro_rules! op_to_col_expr {
    ($col_obj:expr, to $v:expr) => {
        $v
    };
    ($col_obj:expr, add $v:expr) => {
        $col_obj + $v
    };
}

macro_rules! update {
    ($obj:expr, $table:ident, $db:expr, $({ $col:ident, $op:ident $v:expr }), + ) => {{
        let id = $obj.id;
        $obj = $db
            .run(move |c| {
                diesel::update(schema::$table::table.find(id))
                    .set((
                        $(schema::$table::$col.eq(op_to_col_expr!(schema::$table::$col, $op $v))), +
                    ))
                    .get_result(with_log!(c))
            })
            .await?;

        }};
}

macro_rules! base_query {
    ($table:ident) => {
        $table::table
            .into_boxed()
            .filter($table::is_deleted.eq(false))
    };
}

macro_rules! with_log {
    ($c: expr) => {
        &LoggingConnection::new($c)
    };
}

#[derive(Queryable, Insertable, Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
pub struct Comment {
    pub id: i32,
    pub author_hash: String,
    pub author_title: String,
    pub is_tmp: bool,
    pub content: String,
    pub create_time: DateTime<Utc>,
    pub is_deleted: bool,
    pub allow_search: bool,
    pub post_id: i32,
}

#[derive(Queryable, Insertable, Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
pub struct Post {
    pub id: i32,
    pub author_hash: String,
    pub content: String,
    pub cw: String,
    pub author_title: String,
    pub is_tmp: bool,
    pub n_attentions: i32,
    pub n_comments: i32,
    pub create_time: DateTime<Utc>,
    pub last_comment_time: DateTime<Utc>,
    pub is_deleted: bool,
    pub is_reported: bool,
    pub hot_score: i32,
    pub allow_search: bool,
}

#[derive(Queryable, Insertable, Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
pub struct User {
    pub id: i32,
    pub name: String,
    pub token: String,
    pub is_admin: bool,
}

#[derive(Insertable)]
#[table_name = "posts"]
pub struct NewPost {
    pub content: String,
    pub cw: String,
    pub author_hash: String,
    pub author_title: String,
    pub is_tmp: bool,
    pub n_attentions: i32,
    pub allow_search: bool,
}

impl Post {
    _get!(posts);

    _get_multi!(posts);

    pub async fn get_multi(db: &Db, rconn: &RdsConn, ids: &Vec<i32>) -> QueryResult<Vec<Self>> {
        let mut cacher = PostCache::init(&rconn);
        let mut cached_posts = cacher.gets(ids).await;
        let mut id2po = HashMap::<i32, &mut Option<Post>>::new();

        // dbg!(&cached_posts);

        let missing_ids = ids
            .iter()
            .zip(cached_posts.iter_mut())
            .filter_map(|(pid, p)| match p {
                None => {
                    id2po.insert(pid.clone(), p);
                    Some(pid)
                }
                _ => None,
            })
            .copied()
            .collect();

        // dbg!(&missing_ids);
        let missing_ps = Self::_get_multi(db, missing_ids).await?;
        // dbg!(&missing_ps);

        cacher.sets(&missing_ps.iter().collect()).await;

        for p in missing_ps.into_iter() {
            if let Some(op) = id2po.get_mut(&p.id) {
                **op = Some(p);
            }
        }
        // dbg!(&cached_posts);
        Ok(cached_posts
            .into_iter()
            .filter_map(|p| p.filter(|p| !p.is_deleted))
            .collect())
    }

    pub async fn get(db: &Db, rconn: &RdsConn, id: i32) -> QueryResult<Self> {
        // 注意即使is_deleted也应该缓存和返回
        let mut cacher = PostCache::init(&rconn);
        if let Some(p) = cacher.get(&id).await {
            Ok(p)
        } else {
            let p = Self::_get(db, id).await?;
            cacher.sets(&vec![&p]).await;
            Ok(p)
        }
    }

    pub async fn get_comments(&self, db: &Db, rconn: &RdsConn) -> QueryResult<Vec<Comment>> {
        let mut cacher = PostCommentCache::init(self.id, rconn);
        if let Some(cs) = cacher.get().await {
            Ok(cs)
        } else {
            let cs = Comment::gets_by_post_id(db, self.id).await?;
            cacher.set(&cs).await;
            Ok(cs)
        }
    }

    pub async fn clear_comments_cache(&self, rconn: &RdsConn) {
        PostCommentCache::init(self.id, rconn).clear().await;
    }

    pub async fn gets_by_page(
        db: &Db,
        rconn: &RdsConn,
        order_mode: u8,
        start: i64,
        limit: i64,
    ) -> QueryResult<Vec<Self>> {
        let mut cacher = PostListCommentCache::init(order_mode, &rconn);
        if cacher.need_fill().await {
            let pids =
                Self::_get_ids_by_page(db, order_mode.clone(), 0, cacher.i64_minlen()).await?;
            let ps = Self::get_multi(db, rconn, &pids).await?;
            cacher.fill(&ps).await;
        }
        let pids = if start + limit > cacher.i64_len() {
            Self::_get_ids_by_page(db, order_mode, start, limit).await?
        } else {
            cacher.get_pids(start, limit).await
        };

        Self::get_multi(db, rconn, &pids).await
    }
    async fn _get_ids_by_page(
        db: &Db,
        order_mode: u8,
        start: i64,
        limit: i64,
    ) -> QueryResult<Vec<i32>> {
        db.run(move |c| {
            let mut query = base_query!(posts).select(posts::id);
            if order_mode > 0 {
                query = query.filter(posts::is_reported.eq(false))
            }

            query = match order_mode {
                0 => query.order(posts::id.desc()),
                1 => query.order(posts::last_comment_time.desc()),
                2 => query.order(posts::hot_score.desc()),
                3 => query.order(RANDOM),
                _ => panic!("Wrong order mode!"),
            };

            query.offset(start).limit(limit).load(with_log!(c))
        })
        .await
    }

    pub async fn search(
        db: &Db,
        rconn: &RdsConn,
        search_mode: u8,
        search_text: String,
        start: i64,
        limit: i64,
    ) -> QueryResult<Vec<Self>> {
        let search_text2 = search_text.replace("%", "\\%");
        let pids = db
            .run(move |c| {
                let pat;
                let mut query = base_query!(posts)
                    .select(posts::id)
                    .distinct()
                    .left_join(comments::table)
                    .filter(posts::is_reported.eq(false));
                // 先用搜索+缓存，性能有问题了再真的做tag表
                query = match search_mode {
                    0 => {
                        pat = format!("%#{}%", &search_text2);
                        query
                            .filter(posts::cw.eq(&search_text))
                            .or_filter(posts::cw.eq(format!("#{}", &search_text)))
                            .or_filter(posts::content.like(&pat))
                            .or_filter(
                                comments::content
                                    .like(&pat)
                                    .and(comments::is_deleted.eq(false)),
                            )
                    }
                    1 => {
                        pat = format!("%{}%", search_text2.replace(" ", "%"));
                        query
                            .filter(posts::content.like(&pat).or(comments::content.like(&pat)))
                            .filter(posts::allow_search.eq(true))
                    }
                    2 => query
                        .filter(posts::author_title.eq(&search_text))
                        .or_filter(comments::author_title.eq(&search_text)),
                    _ => panic!("Wrong search mode!"),
                };

                query
                    .order(posts::id.desc())
                    .offset(start)
                    .limit(limit)
                    .load(with_log!(c))
            })
            .await?;
        Self::get_multi(db, rconn, &pids).await
    }

    pub async fn create(db: &Db, new_post: NewPost) -> QueryResult<Self> {
        db.run(move |c| {
            insert_into(posts::table)
                .values(&new_post)
                .get_result(with_log!(c))
        })
        .await
    }

    pub async fn set_instance_cache(&self, rconn: &RdsConn) {
        PostCache::init(rconn).sets(&vec![self]).await;
    }
    pub async fn refresh_cache(&self, rconn: &RdsConn, is_new: bool) {
        join!(
            self.set_instance_cache(rconn),
            future::join_all((if is_new { 0..4 } else { 1..4 }).map(|mode| async move {
                PostListCommentCache::init(mode, &rconn.clone())
                    .put(self)
                    .await
            })),
        );
    }

    pub async fn annealing(mut c: Conn, rconn: &RdsConn) {
        info!("Time for annealing!");
        diesel::update(posts::table.filter(posts::hot_score.gt(10)))
            .set(posts::hot_score.eq(floor(float4(posts::hot_score) * 0.9)))
            .execute(with_log!(&mut c))
            .unwrap();

        PostCache::init(&rconn).clear_all().await;
        PostListCommentCache::init(2, rconn).clear().await
    }
}

impl User {
    async fn _get_by_token(db: &Db, token: &str) -> Option<Self> {
        let token = token.to_string();
        db.run(move |c| {
            users::table
                .filter(users::token.eq(token))
                .first(with_log!(c))
        })
        .await
        .ok()
    }

    pub async fn get_by_token(db: &Db, rconn: &RdsConn, token: &str) -> Option<Self> {
        let mut cacher = UserCache::init(token, &rconn);
        if let Some(u) = cacher.get().await {
            Some(u)
        } else {
            let u = Self::_get_by_token(db, token).await?;
            cacher.set(&u).await;
            Some(u)
        }
    }
}

#[derive(Insertable)]
#[table_name = "comments"]
pub struct NewComment {
    pub content: String,
    pub author_hash: String,
    pub author_title: String,
    pub is_tmp: bool,
    pub post_id: i32,
}

impl Comment {
    _get!(comments);

    pub async fn get(db: &Db, id: i32) -> QueryResult<Self> {
        // no cache for single comment
        Self::_get(db, id).await
    }

    pub async fn create(db: &Db, new_comment: NewComment) -> QueryResult<Self> {
        db.run(move |c| {
            insert_into(comments::table)
                .values(&new_comment)
                .get_result(with_log!(c))
        })
        .await
    }

    pub async fn gets_by_post_id(db: &Db, post_id: i32) -> QueryResult<Vec<Self>> {
        let pid = post_id;
        db.run(move |c| {
            comments::table
                .filter(comments::post_id.eq(pid))
                .order(comments::id)
                .load(with_log!(c))
        })
        .await
    }
}

pub(crate) use {op_to_col_expr, update, with_log};
