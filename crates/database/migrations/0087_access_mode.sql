-- How the server is reached, chosen in the administration: behind a proxy
-- (plain connections only), with a certificate it signs itself, with one the
-- administrator provides, or with one obtained automatically. The paths are
-- those of the provided certificate and its key, in PEM.
ALTER TABLE server_settings ADD COLUMN access_mode TEXT NOT NULL DEFAULT 'proxy';
ALTER TABLE server_settings ADD COLUMN certificate_path TEXT;
ALTER TABLE server_settings ADD COLUMN private_key_path TEXT;
