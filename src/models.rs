#![allow(clippy::all)]

use chrono::NaiveDateTime;
use diesel::{insert_into, ExpressionMethods, QueryDsl, RunQueryDsl};

use crate::schema::*;
use crate::db_conn::Conn;


type MR<T> = Result<T, diesel::result::Error>;

no_arg_sql_function!(RANDOM, (), "Represents the sql RANDOM() function");


#[derive(Queryable, Debug)]
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
    pub fn get(conn: &Conn, id: i32) -> MR<Self> {
        posts::table.find(id).first(conn)
    }

    pub fn gets_by_page(
        conn: &Conn,
        order_mode: u8,
        page: u32,
        page_size: u32,
        is_admin: bool,
    ) -> MR<Vec<Self>> {
        let mut query = posts::table.into_boxed();
        if !is_admin {
            query = query.filter(posts::is_deleted.eq(false));
            if order_mode > 0 {
                query = query.filter(posts::is_reported.eq(false))
            }
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
}

#[derive(Queryable, Debug)]
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

#[derive(Queryable, Debug)]
pub struct Comment {
    pub id: i32,
    pub author_hash: String,
    pub author_title: String,
    pub content: String,
    pub create_time: NaiveDateTime,
    pub is_deleted: bool,
    pub post_id: i32,
}

impl Comment {}
