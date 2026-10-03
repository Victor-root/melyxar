-- Whether a film the chosen card cannot take goes to another card that can,
-- rather than to the processor. Off until somebody turns it on.
ALTER TABLE server_settings ADD COLUMN transcode_card_fallback INTEGER NOT NULL DEFAULT 0;
