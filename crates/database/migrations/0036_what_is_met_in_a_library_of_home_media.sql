-- The newest works across every library, now that a library of home media is
-- met through its folders.
--
-- The index this replaces was partial on exactly the works a grid shows, and
-- a folder, a video and a photo at the root of such a library are shown too.
-- Its condition has to say what the grids ask, or the newest works of the
-- whole collection are sorted by hand again.
DROP INDEX works_met_by_added_at;

CREATE INDEX works_met_by_added_at ON works (added_at DESC, id DESC)
    WHERE kind IN ('movie', 'series', 'album')
       OR (kind IN ('episode', 'folder', 'video', 'photo') AND parent_id IS NULL);
