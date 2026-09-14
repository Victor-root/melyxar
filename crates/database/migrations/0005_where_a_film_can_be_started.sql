-- Where a film can be started, which is not everywhere.
--
-- Almost every picture in a film says only what changed since the one before
-- it. Every few seconds there is one that stands on its own, and those are the
-- only places a stream carried over untouched can begin: starting anywhere
-- else would mean starting from a change with nothing to change.
--
-- The playlist is written on a fixed grid of four seconds, which is true of a
-- picture the server rebuilds, because it puts the key frames there itself.
-- It is false of a picture carried over untouched, where they are wherever the
-- encoder left them: the server then announces a segment beginning at one
-- second of the film and hands over one beginning six seconds earlier. That is
-- what makes a jump land before where it was aimed, and it applies to almost
-- every film in a collection, because carrying a picture over is what costs
-- nothing.
--
-- Read once per file, away from anybody watching, and kept here. One row per
-- file rather than one per key frame: a two hour film holds a thousand of
-- them, they are never wanted one at a time, and a table of half a million
-- rows to answer "where can this film be cut" is a table nobody wanted.
CREATE TABLE media_source_key_frames (
    source_id    TEXT PRIMARY KEY REFERENCES media_sources (id) ON DELETE CASCADE,
    -- Ascending, in milliseconds, separated by commas. The first is where the
    -- picture itself begins, which is not always zero.
    positions_ms TEXT NOT NULL,
    -- How many there are, so a report can say so without reading the list.
    counted      INTEGER NOT NULL,
    read_at      TEXT NOT NULL
);
