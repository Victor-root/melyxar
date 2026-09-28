-- The ratings a work carries from elsewhere than its provider: IMDb's, read
-- from the file of ratings it publishes each day, and Rotten Tomatoes', asked
-- of OMDb with the administrator's own key.
--
-- A row with no value is a work asked about that had none: it is kept so the
-- same question is not asked again before its time.
CREATE TABLE work_ratings (
    work_id     TEXT NOT NULL REFERENCES works (id) ON DELETE CASCADE,
    -- imdb, rotten_tomatoes.
    source      TEXT NOT NULL,
    -- Out of ten for IMDb, out of a hundred for Rotten Tomatoes.
    value       REAL,
    votes       INTEGER,
    checked_at  TEXT NOT NULL,
    PRIMARY KEY (work_id, source)
) STRICT;

-- The key OMDb gave the administrator, without which nothing is asked of it.
ALTER TABLE server_settings ADD COLUMN omdb_key TEXT;

-- The seventh scheduled task, at the time and in the state of the look up it
-- follows.
INSERT INTO scheduled_tasks (task, runs_on_schedule, at_utc_minutes)
SELECT 'ratings', runs_on_schedule, at_utc_minutes
  FROM scheduled_tasks
 WHERE task = 'identify';
