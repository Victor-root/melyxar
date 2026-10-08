-- The grid of a library of series, and the episodes counted on its cards.
--
-- A library of series holds its episodes too, ten thousand of them for two
-- hundred series, and the grid reads only the series. Read by title or by
-- arrival through an index of the whole library, the database walked past
-- every episode to find the two hundred series of a page, then sorted them
-- again on the identifier. Indexes holding only the works a grid shows, in
-- the grid's own order, hand the page over directly: the grid of a library
-- of two hundred series went from nine milliseconds to a third of one by
-- title, and from eleven to under half of one by arrival.
--
-- The condition is the one `met_on_its_own` writes, spelt the same way.
CREATE INDEX works_met_by_title ON works (library_id, sort_title, id)
    WHERE (kind IN ('movie', 'series')
          OR (kind IN ('episode', 'folder', 'video', 'photo')
              AND parent_id IS NULL));

CREATE INDEX works_met_in_a_library_by_added_at ON works (library_id, added_at DESC, id DESC)
    WHERE (kind IN ('movie', 'series')
          OR (kind IN ('episode', 'folder', 'video', 'photo')
              AND parent_id IS NULL));

-- Each card of a series counts its episodes, and those not watched yet. The
-- index of children by parent says nothing of their kind, so each episode
-- was read from the table to be told apart from a season: with its kind and
-- identifier in the index, the counts never leave it, and the cards of a
-- page of two hundred series are read in half the time.
CREATE INDEX works_by_parent_and_kind ON works (parent_id, kind, id);
