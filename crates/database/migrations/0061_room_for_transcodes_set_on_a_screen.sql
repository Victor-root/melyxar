-- How much of the disk the segments of the films being converted may fill,
-- and how far behind each viewer they are kept whatever that ceiling says.
-- Empty means no ceiling, where every server starts; five minutes behind a
-- viewer covers any step back an account may set, several times over.
ALTER TABLE server_settings ADD COLUMN transcode_cache_megabytes INTEGER;
ALTER TABLE server_settings ADD COLUMN transcode_kept_behind_seconds INTEGER NOT NULL DEFAULT 300;
