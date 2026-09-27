-- How many films the server may convert at once, set from the administration
-- rather than from the configuration file. Empty means no ceiling at all,
-- which is where every server starts: how many a machine can carry depends
-- on its card and on what is watched, not on a number written in advance.
ALTER TABLE server_settings ADD COLUMN max_transcoding_sessions INTEGER;
