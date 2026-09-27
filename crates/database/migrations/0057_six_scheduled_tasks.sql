-- One row for each scheduled task, each valid for every library: whether it
-- runs on its own each day, at what time, and how its last run went.
--
-- Six tasks where there was one nightly run of four readings shown library by
-- library: the scan and the look up had no schedule at all, and sixteen lines
-- for four readings were read by nobody. Every task starts where the one
-- nightly run stood, switched on or off and at the same time of day.
--
-- The last run is kept here rather than read off the jobs: a task runs over
-- every library in turn, so its run is several jobs, and none of them alone
-- says when the task began or how long the whole of it took.

CREATE TABLE scheduled_tasks (
    task                TEXT PRIMARY KEY NOT NULL,
    runs_on_schedule    INTEGER NOT NULL,
    -- Minutes since midnight, UTC: the one clock a server reads with
    -- certainty. A screen turns it into the time of whoever looks at it.
    at_utc_minutes      INTEGER NOT NULL,
    last_started_at     TEXT,
    last_finished_at    TEXT,
    -- succeeded, failed or cancelled, once a run has ended.
    last_state          TEXT
) STRICT;

INSERT INTO scheduled_tasks (task, runs_on_schedule, at_utc_minutes)
SELECT tasks.column1, settings.upkeep_nightly, settings.upkeep_at_utc_minutes
  FROM (VALUES ('scan'), ('identify'), ('key_frames'), ('subtitles'), ('thumbnails'), ('openings'))
       AS tasks,
       server_settings AS settings
 WHERE settings.id = 1;

ALTER TABLE server_settings DROP COLUMN upkeep_nightly;
ALTER TABLE server_settings DROP COLUMN upkeep_at_utc_minutes;
