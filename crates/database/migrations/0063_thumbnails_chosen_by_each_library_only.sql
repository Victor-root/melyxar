-- Whether the pictures of the playback bar are made is chosen library by
-- library, and the switch that stood above them for the whole server goes.
-- A server that had it off keeps making none: every library is switched off
-- in its place, so nothing changes for whoever chose it.
UPDATE libraries SET make_thumbnails = 0
 WHERE (SELECT thumbnails_enabled FROM server_settings WHERE id = 1) = 0;
ALTER TABLE server_settings DROP COLUMN thumbnails_enabled;
