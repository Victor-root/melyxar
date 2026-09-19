-- The names a work has been filed under, as read off the disk.
--
-- A provider renames a work to its own title and fills in its year, and the
-- scan that reads the same folder the next time no longer recognises it: it
-- writes a second work down, and the two only meet again at the provider,
-- once the whole collection has been asked about. Held here, the name the
-- folder gave stays the one the scan looks for, whatever the work is called
-- on the page.
--
-- Several names per work, because one series arriving on two disks is written
-- one way on one and another way on the other, and both have to lead back to
-- the work that is already there.
CREATE TABLE work_filing_names (
    work_id      TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    sort_title   TEXT NOT NULL,
    release_year INTEGER,
    PRIMARY KEY (work_id, sort_title)
) STRICT;

CREATE INDEX work_filing_names_lookup ON work_filing_names (sort_title);

-- What is already written down was filed under the name it still carries,
-- except where a provider has since renamed it, which nothing here can undo.
INSERT INTO work_filing_names (work_id, sort_title, release_year)
SELECT id, sort_title, release_year FROM works WHERE parent_id IS NULL;
