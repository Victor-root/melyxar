-- What the server puts in front of everybody, and what it can suggest fast.
--
-- Two things, because they are the two halves of the same row of the home
-- page: what an administrator chose to show, and what the server picks when
-- nobody chose anything.
--
-- Pinning is not a bookmark. Each person already has a list of their own,
-- which is the favourites table; this one belongs to the server and is the
-- same for everybody, which is why it names no account. An administrator
-- decides what it holds and in which order, and that order is what the hero
-- shows first.
--
-- The rank is written rather than worked out from the moment something was
-- pinned: a row somebody arranges by hand has an order that has nothing to do
-- with when each piece of it was added.

CREATE TABLE pinned_works (
    work_id     TEXT PRIMARY KEY NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    -- Lower comes first, which is how the hero reads it.
    rank        INTEGER NOT NULL,
    pinned_at   TEXT NOT NULL
) STRICT;

CREATE INDEX pinned_works_in_order ON pinned_works (rank);

-- And the index the suggestions walk.
--
-- A suggestion is the best rated thing this account has not started, kept to
-- the genres it watches most. Ordered by rating and stopped at a dozen, that
-- is an index walk that ends almost as soon as it begins, whatever the
-- collection holds. Without the index it is a full read of every work that
-- carries a rating, sorted, to print twelve cards.
--
-- Partial on purpose: a work with no rating can never be suggested, and a
-- season or an episode is never met on its own, so neither belongs in an
-- index that exists to be walked from one end.
CREATE INDEX works_well_rated
    ON works (community_rating DESC)
 WHERE community_rating IS NOT NULL AND kind IN ('movie', 'series');
