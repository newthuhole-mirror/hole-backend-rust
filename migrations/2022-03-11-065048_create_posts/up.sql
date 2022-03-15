-- Your SQL goes here

CREATE TABLE posts (
  id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
  author_hash VARCHAR NOT NULL,
  content TEXT NOT NULL,
  cw VARCHAR NOT NULL DEFAULT '',
  author_title VARCHAR NOT NULL DEFAULT '',
  n_likes INTEGER NOT NULL DEFAULT 0,
  n_comments INTEGER NOT NULL DEFAULT 0,
  create_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  last_comment_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
  is_reported BOOLEAN NOT NULL DEFAULT FALSE,
  hot_score INTEGER NOT NULL DEFAULT 0,
  allow_search BOOLEAN NOT NULL DEFAULT ''
);
CREATE INDEX posts_last_comment_time_idx ON posts (`last_comment_time`);
CREATE INDEX posts_hot_idx ON posts (`hot_score`)

