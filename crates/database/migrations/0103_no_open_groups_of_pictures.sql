-- Takes back the column of the migration before.
--
-- A picture in open groups lost one picture per segment because the segments
-- were written with an index whose timing moved the key picture onto the
-- picture after it. Written without that index the picture is copied whole,
-- so nothing needs to know any more how a picture is built.
ALTER TABLE media_source_key_frames DROP COLUMN open_groups;
