-- Whether an account may make, rename and delete the collections of the
-- server, and put works in them. Off unless given; an administrator always
-- may.
ALTER TABLE users ADD COLUMN may_manage_collections INTEGER NOT NULL DEFAULT 0;
UPDATE users SET may_manage_collections = 1 WHERE is_administrator = 1;
