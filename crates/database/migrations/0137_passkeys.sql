-- The passkeys an account signs in with, besides its password. Only what the
-- device made public is kept: the credential's identifier and its public key
-- with its counter, as the passkey library writes them down. Nothing here
-- opens anything on its own.
CREATE TABLE passkeys (
    id            TEXT PRIMARY KEY NOT NULL,
    user_id       TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    -- What the device calls the key, which it hands back on every use.
    credential_id BLOB NOT NULL UNIQUE,
    -- The name of the site the key was made under: it serves there alone.
    rp_id         TEXT NOT NULL,
    -- What the account calls it, to tell its keys apart.
    name          TEXT NOT NULL,
    -- The public key and its counter, as the library serialises them.
    credential    TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    used_at       TEXT
) STRICT;

CREATE INDEX passkeys_by_user ON passkeys (user_id, created_at);
