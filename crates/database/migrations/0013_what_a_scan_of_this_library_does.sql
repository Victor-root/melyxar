-- What a scan of one library is allowed to do in a single sitting.
--
-- Two readings of a film are far heavier than everything else a scan does, and
-- both read every file from end to end: the places its picture can be started,
-- and the thumbnails somebody drags along the playback bar. Done inside the
-- scan, a library of several hundred films turns a scan of minutes into one of
-- days. Left out of it, the scan ends quickly and the readings belong to the
-- upkeep that runs of a night, where nobody is waiting on them.
--
-- Off for both to begin with, which is the answer for the library big enough
-- to need the setting at all. A small library on a machine with time to spare
-- ticks them and has everything in one go.
--
-- Stored on the library rather than in the configuration file because the
-- answer differs from one library to the next on the same server, and because
-- nobody should have to open a terminal to change it.

ALTER TABLE libraries ADD COLUMN key_frames_during_scan INTEGER NOT NULL DEFAULT 0;
ALTER TABLE libraries ADD COLUMN thumbnails_during_scan INTEGER NOT NULL DEFAULT 0;
