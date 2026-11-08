-- Whether somebody may make their own account at the door, and whether each
-- one made that way waits for an administrator before it may sign in.
-- Closed, and held for approval, until an administrator says otherwise.
ALTER TABLE server_settings ADD COLUMN sign_ups_open INTEGER NOT NULL DEFAULT 0;
ALTER TABLE server_settings ADD COLUMN sign_ups_held INTEGER NOT NULL DEFAULT 1;

-- An account made at the door: when, and whether it still waits to be let in.
ALTER TABLE users ADD COLUMN signed_up_at TEXT;
ALTER TABLE users ADD COLUMN awaiting_approval INTEGER NOT NULL DEFAULT 0;
