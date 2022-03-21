-- Your SQL goes here
CREATE TABLE comments (
  id SERIAL PRIMARY KEY,
  author_hash VARCHAR NOT NULL,
  author_title VARCHAR NOT NULL DEFAULT '',
  is_tmp BOOLEAN NOT NULL DEFAULT FALSE,
  content TEXT NOT NULL,
  create_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
  allow_search BOOLEAN NOT NULL DEFAULT FALSE,
  post_id INTEGER NOT NULL REFERENCES posts(id)
);
CREATE INDEX comments_postId_idx ON comments (post_id);

