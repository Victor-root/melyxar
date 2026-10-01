-- Subtitles written by listening to the sound of a personal video.
--
-- A library asks for them with one switch, off to begin with: listening is the
-- heaviest thing the server does to a file, and only a library of videos
-- somebody filmed has any use for it. A film or a series already has its
-- subtitles from elsewhere.
ALTER TABLE libraries ADD COLUMN generate_subtitles INTEGER NOT NULL DEFAULT 0;

-- A subtitle nobody wrote is said so wherever it is shown: what was heard can
-- be wrong, and a viewer should not take it for a transcript.
ALTER TABLE tracks ADD COLUMN is_generated INTEGER NOT NULL DEFAULT 0;

-- The model that listens, chosen by the administrator among the ones the
-- server offers and has downloaded. Nothing until one is chosen.
ALTER TABLE server_settings ADD COLUMN speech_model TEXT;

-- Which files have been listened to already, for the same reason as the key
-- frames and the thumbnails: listening reads a whole file, and a file that
-- gave nothing must not be listened to again for the same nothing.
--
-- Nought is an answer. A video with no voice in it, or one the model could
-- make nothing of, settles the question for that file.
CREATE TABLE media_source_speech (
    source_id TEXT PRIMARY KEY REFERENCES media_sources (id) ON DELETE CASCADE,
    -- How many lines of subtitle came out of it.
    lines     INTEGER NOT NULL,
    made_at   TEXT NOT NULL
);

-- The eighth scheduled task, at the time and in the state of the extraction of
-- the subtitles inside a file, which it follows.
INSERT INTO scheduled_tasks (task, runs_on_schedule, at_utc_minutes)
SELECT 'speech', runs_on_schedule, at_utc_minutes
  FROM scheduled_tasks
 WHERE task = 'subtitles';
