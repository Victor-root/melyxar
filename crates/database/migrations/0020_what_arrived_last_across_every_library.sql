-- What arrived last, across every library at once.
--
-- The home page leads with the newest works of the whole collection, not of
-- one library, and that is the one grid nothing indexed: every ordering index
-- on `works` begins with the library, which serves a grid inside a library and
-- cannot serve a question asked across all of them. So the newest of a hundred
-- thousand works meant sorting a hundred thousand works, measured at thirty
-- milliseconds of the fifty a whole page is allowed.
--
-- Only this one ordering, and only this one, because the interface asks for no
-- other across every library: a grid is always opened inside a library, where
-- the existing indexes already walk it. An index for an ordering nobody asks
-- for would be paid for on every write and read by nobody.
--
-- Partial, on exactly the works a grid shows. A season is opened from its
-- series and an episode from its season; neither is ever met on its own, so
-- neither belongs in the index a grid walks, and leaving them out of it is
-- what stops the engine reading the row of every episode to find out.
CREATE INDEX works_met_by_added_at ON works (added_at DESC, id DESC)
    WHERE kind IN ('movie', 'series', 'album')
       OR (kind = 'episode' AND parent_id IS NULL);
