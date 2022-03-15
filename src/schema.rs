table! {
    posts (id) {
        id -> Integer,
        author_hash -> Text,
        content -> Text,
        cw -> Text,
        author_title -> Text,
        n_likes -> Integer,
        n_comments -> Integer,
        create_time -> Timestamp,
        last_comment_time -> Timestamp,
        is_deleted -> Bool,
        is_reported -> Bool,
        hot_score -> Integer,
        allow_search -> Bool,
    }
}

table! {
    users (id) {
        id -> Integer,
        name -> Text,
        token -> Text,
        is_admin -> Bool,
    }
}

allow_tables_to_appear_in_same_query!(
    posts,
    users,
);
