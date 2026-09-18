-- Which films have had their subtitles made of words pulled out already.
--
-- Pulling one out is not expensive to write, a few tens of kilobytes of text.
-- It is expensive to read: the words are interleaved with the picture from end
-- to end, so the whole file has to go past. Measured at three quarters of a
-- minute on a 4K film, and it used to happen when somebody opened that film.
--
-- One row per file rather than one per track, for the same reason as the key
-- frames and the thumbnails: every track of a film comes out of the one
-- reading, so there is one answer per file and it is about the reading.
--
-- Nought is an answer. A film whose every track was already in the cache, and
-- a film that gave nothing usable, both settle the question for that file and
-- are written down so it is never read through again for the same nothing.
CREATE TABLE media_source_subtitles (
    source_id  TEXT PRIMARY KEY REFERENCES media_sources (id) ON DELETE CASCADE,
    -- How many tracks this reading put into the cache. Tracks already there
    -- are not counted again: what is recorded is what the reading did.
    pulled_out INTEGER NOT NULL,
    read_at    TEXT NOT NULL
);
