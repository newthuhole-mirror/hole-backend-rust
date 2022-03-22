table! {
    comments (id) {
        id -> Int4,
        author_hash -> Varchar,
        author_title -> Varchar,
        is_tmp -> Bool,
        content -> Text,
        create_time -> Timestamptz,
        is_deleted -> Bool,
        allow_search -> Bool,
        post_id -> Int4,
    }
}

table! {
    posts (id) {
        id -> Int4,
        author_hash -> Varchar,
        content -> Text,
        cw -> Varchar,
        author_title -> Varchar,
        is_tmp -> Bool,
        n_attentions -> Int4,
        n_comments -> Int4,
        create_time -> Timestamptz,
        last_comment_time -> Timestamptz,
        is_deleted -> Bool,
        is_reported -> Bool,
        hot_score -> Int4,
        allow_search -> Bool,
    }
}

table! {
    users (id) {
        id -> Int4,
        name -> Varchar,
        token -> Varchar,
        is_admin -> Bool,
    }
}

joinable!(comments -> posts (post_id));

allow_tables_to_appear_in_same_query!(comments, posts, users,);
