-- How much of the processor listening to a video takes: quiet, balanced or
-- maximum. Quiet is what it was before this was chosen.
ALTER TABLE server_settings ADD COLUMN speech_effort TEXT NOT NULL DEFAULT 'quiet'
    CHECK (speech_effort IN ('quiet', 'balanced', 'maximum'));
