-- The graphics card chosen to convert films, on a machine carrying several,
-- by the key the server knows it under: its path and where it sits on the
-- machine. Nothing until somebody chooses, and the card that does the most is
-- used meanwhile.
ALTER TABLE server_settings ADD COLUMN transcode_card TEXT;
