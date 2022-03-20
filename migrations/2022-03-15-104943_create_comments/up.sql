-- Your SQL goes here

CREATE TABLE comments (
  id INTEGER NOT NULL PRIMARY KEY,
  author_hash VARCHAR NOT NULL,
  author_title VARCHAR(10) NOT NULL DEFAULT '',
  is_tmp BOOLEAN NOT NULL DEFAULT FALSE,
  content TEXT NOT NULL,
  create_time TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
  is_deleted BOOLEAN NOT NULL DEFAULT FALSE,
  post_id INTEGER NOT NULL,
  FOREIGN KEY(post_id) REFERENCES posts(id)
);
CREATE INDEX comments_postId_idx ON comments (`post_id`);

