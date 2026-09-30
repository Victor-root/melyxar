-- The right to put files into the libraries, from the interface. Held by
-- administrators to begin with, and by nobody else until they are given it.
ALTER TABLE users ADD COLUMN may_upload INTEGER NOT NULL DEFAULT 0;
UPDATE users SET may_upload = 1 WHERE is_administrator = 1;
