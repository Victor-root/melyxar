-- Subtitles written by listening are translated into French by a task of their
-- own, after the listening and never during it.
--
-- Which files have been translated already, for the same reason as the files
-- listened to: a file whose translation gave nothing must not be translated
-- again for the same nothing. Nought means the file still waits for it.
ALTER TABLE media_source_speech ADD COLUMN translated INTEGER NOT NULL DEFAULT 0;

-- The ninth scheduled task, at the time and in the state of the listening it
-- follows.
INSERT INTO scheduled_tasks (task, runs_on_schedule, at_utc_minutes)
SELECT 'translation', runs_on_schedule, at_utc_minutes
  FROM scheduled_tasks
 WHERE task = 'speech';
