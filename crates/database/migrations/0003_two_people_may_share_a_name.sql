-- Two people are allowed to have the same name, because they do.
--
-- A person is identified by what the provider calls them, which is why every
-- look up goes through person_external_ids. The index below asserted instead
-- that no two people share a name, and the first pair of namesakes in a
-- library stopped every identification with a constraint failure.
--
-- The index stays, because reading people in name order is what a page does.
-- Only the claim that a name is unique goes.
DROP INDEX people_by_sort_name;
CREATE INDEX people_by_sort_name ON people (sort_name, name);
