-- Whether a film the card refuses in the middle of its playback is handed to
-- the processor, rather than stopped. Off until somebody turns it on.
ALTER TABLE server_settings ADD COLUMN transcode_processor_fallback INTEGER NOT NULL DEFAULT 0;
