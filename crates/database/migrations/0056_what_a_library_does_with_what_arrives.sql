-- Which heavy readings a library wants, and whether they are done as soon as
-- a file arrives rather than left to the scheduled tasks.
--
-- Each of the three is wanted to begin with: a library that does not want one
-- says so, and its task passes it by. The two switches that asked a scan to
-- read key frames and thumbnails itself go: key frames always follow an
-- arrival now, since they cost next to nothing, and a library that had asked
-- for its thumbnails straight away is a library that wants its files read as
-- they arrive.

ALTER TABLE libraries ADD COLUMN extract_subtitles INTEGER NOT NULL DEFAULT 1;
ALTER TABLE libraries ADD COLUMN make_thumbnails INTEGER NOT NULL DEFAULT 1;
ALTER TABLE libraries ADD COLUMN detect_openings INTEGER NOT NULL DEFAULT 1;
ALTER TABLE libraries ADD COLUMN process_on_arrival INTEGER NOT NULL DEFAULT 0;

UPDATE libraries SET process_on_arrival = thumbnails_during_scan;

ALTER TABLE libraries DROP COLUMN key_frames_during_scan;
ALTER TABLE libraries DROP COLUMN thumbnails_during_scan;
