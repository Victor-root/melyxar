-- The works of one kind in a library, in title order.
--
-- A library of music holds its albums, its artists and every one of its
-- songs as works, so a list of its albums read through the index of a whole
-- library walked sixteen thousand songs to find a thousand albums. With the
-- kind ahead of the title the database walks straight to the albums, already
-- in order: the letters of a library of music went from eighteen
-- milliseconds to one, and the lists of albums, artists and songs read
-- through it as well.
CREATE INDEX works_by_kind_and_title ON works (library_id, kind, sort_title);
