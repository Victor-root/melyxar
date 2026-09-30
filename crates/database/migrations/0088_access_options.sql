-- Two more choices about how the server is reached: whether a request in the
-- clear is sent to the encrypted address once the server encrypts, and the
-- names (domain or address) the certificate signed by the server must carry,
-- as a list separated by commas.
ALTER TABLE server_settings ADD COLUMN redirect_to_https INTEGER NOT NULL DEFAULT 1;
ALTER TABLE server_settings ADD COLUMN public_names TEXT;
