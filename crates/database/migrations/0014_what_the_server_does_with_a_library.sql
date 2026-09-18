-- When the upkeep runs, and what shape the thumbnails of the bar are made in.
--
-- Both were settings of the configuration file, which means both needed a
-- terminal, a text editor and a restart of the server to change. Neither is a
-- property of the machine: the hour somebody wants their films read at, and
-- how close together the little pictures of the playback bar stand, are
-- preferences of whoever runs the server. The file keeps what really depends
-- on the machine, which is where things live and how much may run at once.
--
-- The values are the ones the file used to carry, so a server that comes up on
-- this migration behaves exactly as it did the moment before.
--
-- The hour is kept in minutes since midnight, in UTC. In UTC because it is the
-- only clock a server can read with certainty; in minutes because an offset is
-- not always a whole hour, so a half past three chosen in one place has to be
-- storable as it lands here.
--
-- Changing the shape of the thumbnails puts every film of every library back
-- in front of the upkeep, since what is already made no longer matches what is
-- asked for. That is on purpose and the screen says so before the change.

ALTER TABLE server_settings ADD COLUMN upkeep_nightly INTEGER NOT NULL DEFAULT 1;
ALTER TABLE server_settings ADD COLUMN upkeep_at_utc_minutes INTEGER NOT NULL DEFAULT 180;
ALTER TABLE server_settings ADD COLUMN thumbnails_enabled INTEGER NOT NULL DEFAULT 1;
ALTER TABLE server_settings ADD COLUMN thumbnails_every_seconds INTEGER NOT NULL DEFAULT 10;
ALTER TABLE server_settings ADD COLUMN thumbnails_height INTEGER NOT NULL DEFAULT 180;
ALTER TABLE server_settings ADD COLUMN thumbnails_columns INTEGER NOT NULL DEFAULT 10;
ALTER TABLE server_settings ADD COLUMN thumbnails_rows INTEGER NOT NULL DEFAULT 10;
