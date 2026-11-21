-- A series ordered by when its newest episode arrived.
--
-- Ordered by its own arrival, a series that gets a new season stays where it
-- was the day it first appeared, and the new season shows among what arrived
-- last nowhere at all. Each work now keeps when the newest of what it holds
-- arrived: its own arrival for a film or an episode, the newest of its
-- episodes for a season and a series.
--
-- Kept up to date by the database itself: episodes and seasons are created
-- and moved in half a dozen places, and one of them forgetting would bring
-- the same fault back without a sound.
ALTER TABLE works ADD COLUMN content_added_at TEXT;

UPDATE works SET content_added_at = added_at;

-- Seasons from their episodes, then series from their seasons and from the
-- episodes that hang from them directly.
UPDATE works
   SET content_added_at = MAX(content_added_at,
                              (SELECT MAX(child.added_at) FROM works child WHERE child.parent_id = works.id))
 WHERE kind = 'season'
   AND EXISTS (SELECT 1 FROM works child WHERE child.parent_id = works.id);

UPDATE works
   SET content_added_at = MAX(content_added_at,
                              (SELECT MAX(child.content_added_at) FROM works child WHERE child.parent_id = works.id))
 WHERE kind = 'series'
   AND EXISTS (SELECT 1 FROM works child WHERE child.parent_id = works.id);

-- Every work starts from its own arrival.
CREATE TRIGGER works_content_arrives_with_the_work
AFTER INSERT ON works
WHEN NEW.content_added_at IS NULL
BEGIN
    UPDATE works SET content_added_at = NEW.added_at WHERE id = NEW.id;
END;

-- An episode or a season arriving under a season or a series makes them as
-- new as it is.
CREATE TRIGGER works_content_newer_with_an_arrival
AFTER INSERT ON works
WHEN NEW.kind IN ('episode', 'season') AND NEW.parent_id IS NOT NULL
BEGIN
    UPDATE works
       SET content_added_at = MAX(COALESCE(content_added_at, added_at), NEW.added_at)
     WHERE id = NEW.parent_id
        OR id = (SELECT parent_id FROM works WHERE id = NEW.parent_id);
END;

-- Moved under another season or series, it brings its own date and no other:
-- an old episode put back where it belongs is not something new.
CREATE TRIGGER works_content_newer_with_a_move
AFTER UPDATE OF parent_id ON works
WHEN NEW.kind IN ('episode', 'season')
 AND NEW.parent_id IS NOT NULL
 AND NEW.parent_id IS NOT OLD.parent_id
BEGIN
    UPDATE works
       SET content_added_at = MAX(COALESCE(content_added_at, added_at),
                                  COALESCE(NEW.content_added_at, NEW.added_at))
     WHERE id = NEW.parent_id
        OR id = (SELECT parent_id FROM works WHERE id = NEW.parent_id);
END;

-- The newest works read by their indexes, as before, on the new date. The
-- condition is the one `met_on_its_own` writes, spelt the same way.
DROP INDEX works_met_by_added_at;

CREATE INDEX works_met_by_content_added_at ON works (content_added_at DESC, id DESC)
    WHERE (kind IN ('movie', 'series')
          OR (kind IN ('episode', 'folder', 'video', 'photo')
              AND parent_id IS NULL));

DROP INDEX works_met_in_a_library_by_added_at;

CREATE INDEX works_met_in_a_library_by_content_added_at ON works (library_id, content_added_at DESC, id DESC)
    WHERE (kind IN ('movie', 'series')
          OR (kind IN ('episode', 'folder', 'video', 'photo')
              AND parent_id IS NULL));
