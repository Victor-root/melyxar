-- When a maintenance switched on takes hold. Until then every page shows how
-- long is left, and whoever is watching a film finishes the scene; from then
-- on only an administrator is served. Empty while maintenance is off.

ALTER TABLE server_settings ADD COLUMN maintenance_starts_at TEXT;
