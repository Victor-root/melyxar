-- The works of one kind in title order, ties included.
--
-- The lists of albums and of artists are read by title and then by
-- identifier, so that two albums of the same name keep one order from one
-- page to the next. The index of works of a kind stopped at the title, and
-- the database sorted every album of the library again on the identifier:
-- reading the whole library, and working out each album's artist and last
-- arrival for every one of them, to send two hundred. With the identifier
-- at the end of the index the list comes out in order and the database
-- stops at the two hundredth: a page of albums went from eight milliseconds
-- to five on a library of a thousand.
DROP INDEX works_by_kind_and_title;

CREATE INDEX works_by_kind_and_title ON works (library_id, kind, sort_title, id);
