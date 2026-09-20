-- An index behind every foreign key that a removal follows.
--
-- What this is for: taking a library away is one DELETE, and the database
-- follows it into every table that points at what went. Following it means
-- looking for the rows that point at one identifier, and without an index on
-- the pointing column that is a walk through the whole table, once per row
-- removed. A hundred thousand works against twenty thousand resume positions
-- is two thousand million rows read to remove twenty thousand.
--
-- Measured on the invented library of a hundred thousand works: three minutes
-- and six seconds before, twenty two seconds after, and the whole of that
-- difference was the walking. It is the single writer that waits, so those
-- three minutes were three minutes in which nothing else could be written:
-- no resume position, no scan, nothing.
--
-- Every column listed here is one a removal follows and nothing else indexed.
-- The rule this writes down is SQLite's own: a foreign key wants an index on
-- the side that points, and the side that is pointed at already has one by
-- being a primary key.

CREATE INDEX playback_progress_by_work ON playback_progress (work_id);
CREATE INDEX progress_counters_by_work ON progress_counters (work_id);
CREATE INDEX favorites_by_work ON favorites (work_id);
CREATE INDEX watchlist_by_work ON watchlist (work_id);
CREATE INDEX playlist_items_by_work ON playlist_items (work_id);
CREATE INDEX activity_log_by_work ON activity_log (work_id);
CREATE INDEX work_studios_by_studio ON work_studios (studio_id);
CREATE INDEX extra_videos_by_root ON extra_videos (root_id);
CREATE INDEX user_library_access_by_library ON user_library_access (library_id);
