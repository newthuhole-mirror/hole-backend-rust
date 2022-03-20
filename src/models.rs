#![allow(clippy::all)]

use chrono::NaiveDateTime;
use diesel::{insert_into, ExpressionMethods, QueryDsl, QueryResult, RunQueryDsl};

use crate::db_conn::Db;
use crate::schema::*;

no_arg_sql_function!(RANDOM, (), "Represents the sql RANDOM() function");

macro_rules! get {
    ($table:ident) => {
        pub async fn get(db: &Db, id: i32) -> QueryResult<Self> {
            let pid = id;
            db.run(move |c| $table::table.find(pid).first(c)).await
        }
    };
}

macro_rules! get_multi {
    ($table:ident) => {
        pub async fn get_multi(db: &Db, ids: Vec<i32>) -> QueryResult<Vec<Self>> {
            db.run(move |c| $table::table.filter($table::id.eq_any(ids)).load(c))
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
                    .execute(c)
            })
            .await
        }
    };
}

#[derive(Queryable, Identifiable)]
pub struct Post {
    pub id: i32,
    pub author_hash: String,
    pub content: String,
    pub cw: String,
    pub author_title: String,
    pub is_tmp: bool,
    pub n_attentions: i32,
    pub n_comments: i32,
    pub create_time: NaiveDateTime,
    pub last_comment_time: NaiveDateTime,
    pub is_deleted: bool,
    pub is_reported: bool,
    pub hot_score: i32,
    pub allow_search: bool,
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

    pub async fn gets_by_page(
        db: &Db,
        order_mode: u8,
        page: u32,
        page_size: u32,
    ) -> QueryResult<Vec<Self>> {
        db.run(move |c| {
            let mut query = posts::table.into_boxed();
            query = query.filter(posts::is_deleted.eq(false));
            if order_mode > 0 {
                query = query.filter(posts::is_reported.eq(false))
            }

            match order_mode {
                1 => query = query.order(posts::last_comment_time.desc()),
                2 => query = query.order(posts::hot_score.desc()),
                3 => query = query.order(RANDOM),
                _ => query = query.order(posts::id.desc()),
            }

            query
                .offset(((page - 1) * page_size).into())
                .limit(page_size.into())
                .load(c)
        })
        .await
    }

    pub async fn create(db: &Db, new_post: NewPost) -> QueryResult<usize> {
        // TODO: tags
        db.run(move |c| insert_into(posts::table).values(&new_post).execute(c))
            .await
    }

    pub async fn update_cw(&self, db: &Db, new_cw: String) -> QueryResult<usize> {
        let pid = self.id;
        db.run(move |c| {
            diesel::update(posts::table.find(pid))
                .set(posts::cw.eq(new_cw))
                .execute(c)
        })
        .await
    }

    pub async fn change_n_comments(&self, db: &Db, delta: i32) -> QueryResult<usize> {
        let pid = self.id;
        db.run(move |c| {
            diesel::update(posts::table.find(pid))
                .set(posts::n_comments.eq(posts::n_comments + delta))
                .execute(c)
        })
        .await
    }

    pub async fn change_n_attentions(&self, db: &Db, delta: i32) -> QueryResult<usize> {
        let pid = self.id;
        db.run(move |c| {
            diesel::update(posts::table.find(pid))
                .set(posts::n_attentions.eq(posts::n_attentions + delta))
                .execute(c)
        })
        .await
    }
}

#[derive(Queryable, Identifiable)]
pub struct User {
    pub id: i32,
    pub name: String,
    pub token: String,
    pub is_admin: bool,
}

impl User {
    pub async fn get_by_token(db: &Db, token: &str) -> Option<Self> {
        let token = token.to_string();
        db.run(move |c| users::table.filter(users::token.eq(token)).first(c))
            .await
            .ok()
    }
}

#[derive(Queryable, Identifiable)]
pub struct Comment {
    pub id: i32,
    pub author_hash: String,
    pub author_title: String,
    pub is_tmp: bool,
    pub content: String,
    pub create_time: NaiveDateTime,
    pub is_deleted: bool,
    pub post_id: i32,
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

    pub async fn create(db: &Db, new_comment: NewComment) -> QueryResult<usize> {
        db.run(move |c| insert_into(comments::table).values(&new_comment).execute(c))
            .await
    }

    pub async fn gets_by_post_id(db: &Db, post_id: i32) -> QueryResult<Vec<Self>> {
        let pid = post_id;
        db.run(move |c| comments::table.filter(comments::post_id.eq(pid)).load(c))
            .await
    }
}
