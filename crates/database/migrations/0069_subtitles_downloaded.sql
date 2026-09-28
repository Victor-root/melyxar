-- A subtitle downloaded rather than found on the disk: the name of its file in
-- the server's own folder of downloaded subtitles. A scan rewrites the
-- subtitles found next to a film and leaves these alone.
ALTER TABLE tracks ADD COLUMN downloaded_file TEXT;

-- What the server needs to ask OpenSubtitles: the key it gave the
-- administrator's application, and the administrator's own account, without
-- which a day allows only a handful of downloads.
ALTER TABLE server_settings ADD COLUMN opensubtitles_key TEXT;
ALTER TABLE server_settings ADD COLUMN opensubtitles_username TEXT;
ALTER TABLE server_settings ADD COLUMN opensubtitles_password TEXT;
