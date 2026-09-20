-- The decade a work belongs to, written down rather than worked out.
--
-- Asking for a decade was asking for a stretch of years, and a stretch cannot
-- be read in title order: the index gives the works of the nineties in the
-- order their years put them in, so the whole of that decade had to be read
-- and sorted before the first hundred cards could be sent. At a hundred
-- thousand works that was seventeen milliseconds, and at five hundred
-- thousand eighty two, over a budget of thirty. It grows with the collection,
-- which is the shape this server exists to avoid.
--
-- A decade is one value, not a stretch, so with the value itself in the index
-- ahead of the title the database walks straight to the works of that decade
-- already in title order and stops at the hundredth. What it costs then is
-- the hundred cards, whatever the collection holds.
--
-- Worked out by the database from the year and never written by anybody, so
-- it cannot fall out of step with the year it comes from, and it costs no
-- room in the table: only the index holds it.

ALTER TABLE works
    ADD COLUMN decade INTEGER GENERATED ALWAYS AS ((release_year / 10) * 10) VIRTUAL;

CREATE INDEX works_by_decade ON works (library_id, decade, sort_title);
