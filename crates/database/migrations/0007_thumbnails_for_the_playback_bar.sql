-- The little pictures shown while somebody drags along the playback bar.
--
-- A film of two hours holds seven hundred of them at one every ten seconds.
-- Kept one to a file they would be seven hundred requests to drag along a bar,
-- dozens of them a second while the finger moves, so they are gathered a
-- hundred at a time into sheets: one request covers a thousand seconds of film
-- and the next sheet is fetched long before it is reached.
--
-- One row per file rather than one per thumbnail, for the same reason as the
-- key frames: they are never wanted one at a time, and where any one of them
-- sits follows from the shape written here.
--
-- The shape is kept alongside the count because it is what tells a film made
-- to the settings in force from one made to older ones. Change the interval
-- and every film stops matching, which is exactly when they have to be made
-- again.
CREATE TABLE media_source_thumbnails (
    source_id         TEXT PRIMARY KEY REFERENCES media_sources (id) ON DELETE CASCADE,
    -- How far apart in the film two thumbnails stand.
    every_ms          INTEGER NOT NULL,
    -- Size of one thumbnail, measured on a sheet rather than worked out: a
    -- film is not always the shape its pixel count suggests.
    thumbnail_width   INTEGER NOT NULL,
    thumbnail_height  INTEGER NOT NULL,
    columns_per_sheet INTEGER NOT NULL,
    rows_per_sheet    INTEGER NOT NULL,
    -- How many came out of the reading, which is not what the running time
    -- suggests. Nought is an answer: a file in a film folder that holds no
    -- picture, written down so it is never read through again for the same
    -- nothing.
    counted           INTEGER NOT NULL,
    sheets            INTEGER NOT NULL,
    made_at           TEXT NOT NULL
);
