-- How many wrong passwords in a row an account takes before it is held back
-- for a minute, set from the administration. Ten was the fixed number before.
ALTER TABLE server_settings ADD COLUMN sign_in_tries INTEGER NOT NULL DEFAULT 10;
