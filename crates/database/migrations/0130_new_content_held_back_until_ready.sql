-- Whether a library keeps what arrives in it out of sight until the readings
-- that follow an arrival are done with it. On for every library, those already
-- here included: a film played before it is ready plays badly, and that reads
-- as the server going wrong.

ALTER TABLE libraries ADD COLUMN holds_back_until_ready INTEGER NOT NULL DEFAULT 1;

-- When the library last had nothing left to take in. What was added after it
-- is new; everything before it has been shown already and is never held back
-- again. Everything already here has been shown, so it starts at now. Empty for
-- a library that has never finished taking anything in, whose first import is
-- new from end to end.

ALTER TABLE libraries ADD COLUMN settled_at TEXT;
UPDATE libraries SET settled_at = strftime('%Y-%m-%dT%H:%M:%f', 'now') || '000000Z';
