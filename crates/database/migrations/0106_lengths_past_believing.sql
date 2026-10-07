-- A file that says it lasts more than a week is taken as not saying how long
-- it lasts. Files read before that rule keep what they said until they are
-- read again, and a film or a song cut into pieces by a length of years asks
-- for a list of them without end: forgotten here, once.
UPDATE media_sources SET duration_ms = NULL WHERE duration_ms > 604800000;
