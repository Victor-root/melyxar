-- The newest works and the best rated ones, read by their indexes again.
--
-- A partial index is used only when the question asks for exactly what the
-- index holds, word for word. Both of these still held what a grid showed
-- when they were written, the first albums as well, the second films and
-- series alone, while the questions had since moved to the works a grid
-- shows now. The two no longer matched, and every row of newest works on the
-- home page and its suggestions read the whole table and sorted it, songs
-- and episodes included. Measured on a collection of three thousand films,
-- two hundred series and sixteen thousand songs: the row of what arrived
-- last across every library went from fourteen milliseconds to a tenth of
-- one, and the suggestions from six to a twentieth of one.
--
-- The condition is the one `met_on_its_own` writes, spelt the same way.
DROP INDEX works_met_by_added_at;

CREATE INDEX works_met_by_added_at ON works (added_at DESC, id DESC)
    WHERE (kind IN ('movie', 'series')
          OR (kind IN ('episode', 'folder', 'video', 'photo')
              AND parent_id IS NULL));

DROP INDEX works_well_rated;

CREATE INDEX works_well_rated ON works (community_rating DESC)
    WHERE (kind IN ('movie', 'series')
          OR (kind IN ('episode', 'folder', 'video', 'photo')
              AND parent_id IS NULL))
      AND community_rating IS NOT NULL;
