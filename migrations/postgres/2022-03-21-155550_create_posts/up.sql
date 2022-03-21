-- Your SQL goes here
CREATE TABLE posts (
  id SERIAL PRIMARY KEY,
  author_hash VARCHAR NOT NULL,
  content TEXT NOT NULL,
  cw VARCHAR NOT NULL DEFAULT '',
  author_title VARCHAR NOT NULL DEFAULT '',
  is_tmp BOOLEAN NOT NULL DEFAULT FALSE,
  n_attentions INTEGER NOT NULL DEFAULT 0,
  n_comments INTEGER NOT NULL DEFAULT 0,
  create_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  last_comment_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
  is_reported BOOLEAN NOT NULL DEFAULT FALSE,
  hot_score INTEGER NOT NULL DEFAULT 0,
  allow_search BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE INDEX posts_last_comment_time_idx ON posts (last_comment_time);
CREATE INDEX posts_hot_idx ON posts (hot_score);
CREATE INDEX posts_author_idx ON posts (author_title);
CREATE INDEX posts_cw_idx ON posts (cw);
CREATE INDEX posts_search_text_trgm_idx ON posts USING gin(content gin_trgm_ops);
