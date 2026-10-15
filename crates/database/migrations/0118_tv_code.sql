-- The four-digit code an account can give itself for its televisions, kept
-- as a password is, in its stored form and never as it was typed. It never
-- opens an account anywhere new: it only lets a television where the account
-- already signed in with its password open it again.
ALTER TABLE users ADD COLUMN tv_code_hash TEXT;
