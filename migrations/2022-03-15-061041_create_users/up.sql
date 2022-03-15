-- Your SQL goes here

CREATE TABLE users (
  id INTEGER NOT NULL PRIMARY KEY,
  name VARCHAR NOT NULL UNIQUE,
  token VARCHAR NOT NULL UNIQUE,
  is_admin BOOLEAN NOT NULL DEFAULT FALSE
);
CREATE INDEX users_toekn_idx ON users (`token`);
