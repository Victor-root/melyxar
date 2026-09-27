-- The codecs a converted film may come out in, set from the administration
-- rather than from the configuration file, best first and parted by commas.
-- All three to begin with, as a server nobody configured always offered.
ALTER TABLE server_settings ADD COLUMN transcode_video_codecs TEXT NOT NULL DEFAULT 'av1,hevc,h264';
