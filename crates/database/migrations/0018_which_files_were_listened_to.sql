-- Which files have been listened to for the opening and closing titles.
--
-- Unlike the three readings beside it, this one cannot answer about a file on
-- its own: what marks an opening is that every episode of the season holds the
-- same one, so a file is only ever listened to as part of its season. What is
-- written down is still per file, because that is what a file is: the season
-- is how the work is done, the file is what the work is about.
--
-- Nought is an answer, and the important one. A season whose episodes open
-- straight into the story shares nothing, and every one of its files is
-- written down as listened to with nothing found, so the same season is not
-- read through again every single night for the same nothing. The same goes
-- for a file holding no sound at all, and for a season of one episode, which
-- has nothing to be compared against and never will have.
--
-- A file with no row here holds its whole season waiting, which is what puts
-- a season back in the queue the day an episode is added to it.
CREATE TABLE media_source_openings (
    source_id   TEXT PRIMARY KEY REFERENCES media_sources (id) ON DELETE CASCADE,
    -- How many stretches this listening wrote down for this file.
    found       INTEGER NOT NULL,
    listened_at TEXT NOT NULL
) STRICT;
