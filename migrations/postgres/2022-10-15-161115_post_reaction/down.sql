-- This file should undo anything in `up.sql`
ALTER TABLE posts
DROP COLUMN up_votes,
DROP COLUMN down_votes
