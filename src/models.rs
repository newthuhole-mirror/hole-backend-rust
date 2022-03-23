#![allow(clippy::all)]

use crate::db_conn::Db;
use crate::libs::diesel_logger::LoggingConnection;
use crate::rds_conn::RdsConn;
use crate::rds_models::PostCache;
use crate::schema::*;
use chrono::{offset::Utc, DateTime};
use diesel::{
    insert_into, BoolExpressionMethods, ExpressionMethods, QueryDsl, QueryResult, RunQueryDsl,
    TextExpressionMethods,
};
use rocket::serde::{Deserialize, Serialize};

no_arg_sql_function!(RANDOM, (), "Represents the sql RANDOM() function");

macro_rules! get {
    ($table:ident) => {
        pub async fn get(db: &Db, id: i32) -> QueryResult<Self> {
            let pid = id;
            db.run(move |c| $table::table.find(pid).first(with_log!((c))))
                .await
        }
    };
}

macro_rules! get_multi {
    ($table:ident) => {
        pub async fn get_multi(db: &Db, ids: Vec<i32>) -> QueryResult<Vec<Self>> {
            // can use eq(any()) for postgres
            db.run(move |c| {
                $table::table
                    .filter($table::id.eq_any(ids))
                    .filter($table::is_deleted.eq(false))
                    .order($table::id.desc())
                    .load(with_log!(c))
            })
            .await
        }
    };
}

macro_rules! set_deleted {
    ($table:ident) => {
        pub async fn set_deleted(&self, db: &Db) -> QueryResult<usize> {
            let pid = self.id;
            db.run(move |c| {
                diesel::update($table::table.find(pid))
                    .set($table::is_deleted.eq(true))
                    .execute(with_log!(c))
            })
            .await
        }
    };
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

#[derive(Queryable, Insertable)]
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

#[derive(Queryable, Insertable, Serialize, Deserialize)]
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

#[derive(Queryable, Insertable)]
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
    // TODO: tags
}

impl Post {
    get!(posts);

    get_multi!(posts);

    set_deleted!(posts);

    pub async fn get_with_cache(db: &Db, rconn: &RdsConn, id: i32) -> QueryResult<Self> {
        let mut cacher = PostCache::init(&id, &rconn);
        if let Some(p) = cacher.get().await {
            dbg!("hint and use post cache");
            Ok(p)
        } else {
            let p = Self::get(db, id).await?;
            cacher.set(&p).await;
            Ok(p)
        }
    }

    pub async fn gets_by_page(
        db: &Db,
        order_mode: u8,
        start: i64,
        limit: i64,
    ) -> QueryResult<Vec<Self>> {
        db.run(move |c| {
            let mut query = base_query!(posts);
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
        search_mode: u8,
        search_text: String,
        start: i64,
        limit: i64,
    ) -> QueryResult<Vec<Self>> {
        let search_text2 = search_text.replace("%", "\\%");
        db.run(move |c| {
            let pat;
            let mut query = base_query!(posts)
                .distinct()
                .left_join(comments::table);
            // 先用搜索+缓存，性能有问题了再真的做tag表
            query = match search_mode {
                0 => {
                    pat = format!("%#{}%", &search_text2);
                    query
                        .filter(posts::cw.eq(&search_text))
                        .or_filter(posts::cw.eq(format!("#{}", &search_text)))
                        .or_filter(posts::content.like(&pat))
                        .or_filter(comments::content.like(&pat).and(comments::is_deleted.eq(false)))
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
        .await
    }

    pub async fn create(db: &Db, new_post: NewPost) -> QueryResult<Self> {
        // TODO: tags
        db.run(move |c| {
            insert_into(posts::table)
                .values(&new_post)
                .get_result(with_log!(c))
        })
        .await
    }

    pub async fn update_cw(&self, db: &Db, new_cw: String) -> QueryResult<usize> {
        let pid = self.id;
        db.run(move |c| {
            diesel::update(posts::table.find(pid))
                .set(posts::cw.eq(new_cw))
                .execute(with_log!(c))
        })
        .await
    }

    pub async fn change_n_comments(&self, db: &Db, delta: i32) -> QueryResult<Self> {
        let pid = self.id;
        db.run(move |c| {
            diesel::update(posts::table.find(pid))
                .set(posts::n_comments.eq(posts::n_comments + delta))
                .get_result(with_log!(c))
        })
        .await
    }

    pub async fn change_n_attentions(&self, db: &Db, delta: i32) -> QueryResult<Self> {
        let pid = self.id;
        db.run(move |c| {
            diesel::update(posts::table.find(pid))
                .set(posts::n_attentions.eq(posts::n_attentions + delta))
                .get_result(with_log!(c))
        })
        .await
    }

    pub async fn change_hot_score(&self, db: &Db, delta: i32) -> QueryResult<Self> {
        let pid = self.id;
        db.run(move |c| {
            diesel::update(posts::table.find(pid))
                .set(posts::hot_score.eq(posts::hot_score + delta))
                .get_result(with_log!(c))
        })
        .await
    }

    pub async fn set_instance_cache(&self, rconn: &RdsConn) {
        PostCache::init(&self.id, rconn).set(self).await;
    }
    pub async fn refresh_cache(&self, rconn: &RdsConn, is_new: bool) {
        self.set_instance_cache(rconn).await;
    }
}

impl User {
    pub async fn get_by_token(db: &Db, token: &str) -> Option<Self> {
        let token = token.to_string();
        db.run(move |c| {
            users::table
                .filter(users::token.eq(token))
                .first(with_log!(c))
        })
        .await
        .ok()
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
    get!(comments);

    set_deleted!(comments);

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
