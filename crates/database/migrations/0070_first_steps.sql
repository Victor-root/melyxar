-- Whether whoever set this server up has been through its first steps in
-- the browser: the libraries it is to hold, after the first account. A server
-- that already has accounts was set up before these steps existed.
ALTER TABLE server_settings ADD COLUMN first_steps_done INTEGER NOT NULL DEFAULT 0;
UPDATE server_settings SET first_steps_done = 1 WHERE EXISTS (SELECT 1 FROM users);
