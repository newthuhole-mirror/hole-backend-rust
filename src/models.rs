#![allow(clippy::all)]

use chrono::NaiveDateTime;
use diesel::{insert_into, ExpressionMethods, QueryDsl, RunQueryDsl};

use crate::db_conn::Conn;
use crate::schema::*;

type MR<T> = Result<T, diesel::result::Error>;

no_arg_sql_function!(RANDOM, (), "Represents the sql RANDOM() function");

macro_rules! get {
    ($table:ident) => {
        pub fn get(conn: &Conn, id: i32) -> MR<Self> {
            $table::table.find(id).first(conn)
        }
    };
}

macro_rules! set_deleted {
    ($table:ident) => {
        pub fn set_deleted(&self, conn: &Conn) -> MR<()> {
            diesel::update(self)
                .set($table::is_deleted.eq(true))
                .execute(conn)?;
            Ok(())
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
    pub n_likes: i32,
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
pub struct NewPost<'a> {
    pub content: &'a str,
    pub cw: &'a str,
    pub author_hash: &'a str,
    pub author_title: &'a str,
    pub allow_search: bool,
    // TODO: tags
}

impl Post {
    get!(posts);

    set_deleted!(posts);

    pub fn gets_by_page(conn: &Conn, order_mode: u8, page: u32, page_size: u32) -> MR<Vec<Self>> {
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
            .load(conn)
    }

    pub fn get_comments(&self, conn: &Conn) -> MR<Vec<Comment>> {
        comments::table
            .filter(comments::post_id.eq(self.id))
            .load(conn)
    }

    pub fn create(conn: &Conn, new_post: NewPost) -> MR<usize> {
        // TODO: tags
        insert_into(posts::table).values(&new_post).execute(conn)
    }

    pub fn update_cw(&self, conn: &Conn, new_cw: &str) -> MR<usize> {
        diesel::update(self).set(posts::cw.eq(new_cw)).execute(conn)
    }

    pub fn after_add_comment(&self, conn: &Conn) -> MR<()> {
        diesel::update(self)
            .set(posts::n_comments.eq(posts::n_comments + 1))
            .execute(conn)?;
        // TODO: attention, hot_score
        Ok(())
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
    pub fn get_by_token(conn: &Conn, token: &str) -> Option<Self> {
        users::table.filter(users::token.eq(token)).first(conn).ok()
    }
}

#[derive(Queryable, Identifiable)]
pub struct Comment {
    pub id: i32,
    pub author_hash: String,
    pub author_title: String,
    pub content: String,
    pub create_time: NaiveDateTime,
    pub is_deleted: bool,
    pub post_id: i32,
}

#[derive(Insertable)]
#[table_name = "comments"]
pub struct NewComment<'a> {
    pub content: &'a str,
    pub author_hash: &'a str,
    pub author_title: &'a str,
    pub post_id: i32,
}

impl Comment {
    get!(comments);

    set_deleted!(comments);

    pub fn create(conn: &Conn, new_comment: NewComment) -> MR<usize> {
        insert_into(comments::table)
            .values(&new_comment)
            .execute(conn)
    }
}
