-- Your SQL goes here
CREATE TABLE users (
  id SERIAL PRIMARY KEY,
  name VARCHAR NOT NULL UNIQUE,
  token VARCHAR NOT NULL UNIQUE,
  is_admin BOOLEAN NOT NULL DEFAULT FALSE
);
