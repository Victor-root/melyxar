-- Every instant written again with all nine digits after the second.
--
-- Instants are compared as text, and the form they were written in dropped
-- the zeros at the end of the fraction: 01.51Z sorted after 01.515Z, which is
-- the wrong way round. Written at a fixed width, the order of the text is the
-- order of time. Only what has the shape of an instant in UTC is touched, and
-- only when it is not already in the new form.

UPDATE libraries SET created_at = substr(created_at, 1, 19) || '.'
    || substr(CASE WHEN substr(created_at, 20, 1) = '.' THEN substr(created_at, 21, length(created_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(created_at) <> 30;

UPDATE libraries SET updated_at = substr(updated_at, 1, 19) || '.'
    || substr(CASE WHEN substr(updated_at, 20, 1) = '.' THEN substr(updated_at, 21, length(updated_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE updated_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(updated_at) <> 30;

UPDATE library_roots SET access_checked_at = substr(access_checked_at, 1, 19) || '.'
    || substr(CASE WHEN substr(access_checked_at, 20, 1) = '.' THEN substr(access_checked_at, 21, length(access_checked_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE access_checked_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(access_checked_at) <> 30;

UPDATE users SET created_at = substr(created_at, 1, 19) || '.'
    || substr(CASE WHEN substr(created_at, 20, 1) = '.' THEN substr(created_at, 21, length(created_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(created_at) <> 30;

UPDATE devices SET created_at = substr(created_at, 1, 19) || '.'
    || substr(CASE WHEN substr(created_at, 20, 1) = '.' THEN substr(created_at, 21, length(created_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(created_at) <> 30;

UPDATE devices SET last_seen_at = substr(last_seen_at, 1, 19) || '.'
    || substr(CASE WHEN substr(last_seen_at, 20, 1) = '.' THEN substr(last_seen_at, 21, length(last_seen_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE last_seen_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(last_seen_at) <> 30;

UPDATE server_settings SET maintenance_until = substr(maintenance_until, 1, 19) || '.'
    || substr(CASE WHEN substr(maintenance_until, 20, 1) = '.' THEN substr(maintenance_until, 21, length(maintenance_until) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE maintenance_until GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(maintenance_until) <> 30;

UPDATE server_settings SET updated_at = substr(updated_at, 1, 19) || '.'
    || substr(CASE WHEN substr(updated_at, 20, 1) = '.' THEN substr(updated_at, 21, length(updated_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE updated_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(updated_at) <> 30;

UPDATE works SET added_at = substr(added_at, 1, 19) || '.'
    || substr(CASE WHEN substr(added_at, 20, 1) = '.' THEN substr(added_at, 21, length(added_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE added_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(added_at) <> 30;

UPDATE works SET updated_at = substr(updated_at, 1, 19) || '.'
    || substr(CASE WHEN substr(updated_at, 20, 1) = '.' THEN substr(updated_at, 21, length(updated_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE updated_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(updated_at) <> 30;

UPDATE work_locked_fields SET locked_at = substr(locked_at, 1, 19) || '.'
    || substr(CASE WHEN substr(locked_at, 20, 1) = '.' THEN substr(locked_at, 21, length(locked_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE locked_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(locked_at) <> 30;

UPDATE work_field_provenance SET fetched_at = substr(fetched_at, 1, 19) || '.'
    || substr(CASE WHEN substr(fetched_at, 20, 1) = '.' THEN substr(fetched_at, 21, length(fetched_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE fetched_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(fetched_at) <> 30;

UPDATE media_sources SET modified_at = substr(modified_at, 1, 19) || '.'
    || substr(CASE WHEN substr(modified_at, 20, 1) = '.' THEN substr(modified_at, 21, length(modified_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE modified_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(modified_at) <> 30;

UPDATE media_sources SET added_at = substr(added_at, 1, 19) || '.'
    || substr(CASE WHEN substr(added_at, 20, 1) = '.' THEN substr(added_at, 21, length(added_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE added_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(added_at) <> 30;

UPDATE media_sources SET analysed_at = substr(analysed_at, 1, 19) || '.'
    || substr(CASE WHEN substr(analysed_at, 20, 1) = '.' THEN substr(analysed_at, 21, length(analysed_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE analysed_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(analysed_at) <> 30;

UPDATE extra_videos SET created_at = substr(created_at, 1, 19) || '.'
    || substr(CASE WHEN substr(created_at, 20, 1) = '.' THEN substr(created_at, 21, length(created_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(created_at) <> 30;

UPDATE media_segments SET created_at = substr(created_at, 1, 19) || '.'
    || substr(CASE WHEN substr(created_at, 20, 1) = '.' THEN substr(created_at, 21, length(created_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(created_at) <> 30;

UPDATE people SET created_at = substr(created_at, 1, 19) || '.'
    || substr(CASE WHEN substr(created_at, 20, 1) = '.' THEN substr(created_at, 21, length(created_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(created_at) <> 30;

UPDATE people SET described_at = substr(described_at, 1, 19) || '.'
    || substr(CASE WHEN substr(described_at, 20, 1) = '.' THEN substr(described_at, 21, length(described_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE described_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(described_at) <> 30;

UPDATE collections SET created_at = substr(created_at, 1, 19) || '.'
    || substr(CASE WHEN substr(created_at, 20, 1) = '.' THEN substr(created_at, 21, length(created_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(created_at) <> 30;

UPDATE playlists SET created_at = substr(created_at, 1, 19) || '.'
    || substr(CASE WHEN substr(created_at, 20, 1) = '.' THEN substr(created_at, 21, length(created_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(created_at) <> 30;

UPDATE playlists SET updated_at = substr(updated_at, 1, 19) || '.'
    || substr(CASE WHEN substr(updated_at, 20, 1) = '.' THEN substr(updated_at, 21, length(updated_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE updated_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(updated_at) <> 30;

UPDATE images SET created_at = substr(created_at, 1, 19) || '.'
    || substr(CASE WHEN substr(created_at, 20, 1) = '.' THEN substr(created_at, 21, length(created_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(created_at) <> 30;

UPDATE playback_progress SET reported_at = substr(reported_at, 1, 19) || '.'
    || substr(CASE WHEN substr(reported_at, 20, 1) = '.' THEN substr(reported_at, 21, length(reported_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE reported_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(reported_at) <> 30;

UPDATE playback_progress SET last_played_at = substr(last_played_at, 1, 19) || '.'
    || substr(CASE WHEN substr(last_played_at, 20, 1) = '.' THEN substr(last_played_at, 21, length(last_played_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE last_played_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(last_played_at) <> 30;

UPDATE favorites SET created_at = substr(created_at, 1, 19) || '.'
    || substr(CASE WHEN substr(created_at, 20, 1) = '.' THEN substr(created_at, 21, length(created_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(created_at) <> 30;

UPDATE watchlist SET created_at = substr(created_at, 1, 19) || '.'
    || substr(CASE WHEN substr(created_at, 20, 1) = '.' THEN substr(created_at, 21, length(created_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(created_at) <> 30;

UPDATE progress_counters SET latest_child_added_at = substr(latest_child_added_at, 1, 19) || '.'
    || substr(CASE WHEN substr(latest_child_added_at, 20, 1) = '.' THEN substr(latest_child_added_at, 21, length(latest_child_added_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE latest_child_added_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(latest_child_added_at) <> 30;

UPDATE activity_log SET occurred_at = substr(occurred_at, 1, 19) || '.'
    || substr(CASE WHEN substr(occurred_at, 20, 1) = '.' THEN substr(occurred_at, 21, length(occurred_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE occurred_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(occurred_at) <> 30;

UPDATE jobs SET created_at = substr(created_at, 1, 19) || '.'
    || substr(CASE WHEN substr(created_at, 20, 1) = '.' THEN substr(created_at, 21, length(created_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE created_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(created_at) <> 30;

UPDATE jobs SET started_at = substr(started_at, 1, 19) || '.'
    || substr(CASE WHEN substr(started_at, 20, 1) = '.' THEN substr(started_at, 21, length(started_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE started_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(started_at) <> 30;

UPDATE jobs SET finished_at = substr(finished_at, 1, 19) || '.'
    || substr(CASE WHEN substr(finished_at, 20, 1) = '.' THEN substr(finished_at, 21, length(finished_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE finished_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(finished_at) <> 30;

UPDATE media_source_key_frames SET read_at = substr(read_at, 1, 19) || '.'
    || substr(CASE WHEN substr(read_at, 20, 1) = '.' THEN substr(read_at, 21, length(read_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE read_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(read_at) <> 30;

UPDATE media_source_thumbnails SET made_at = substr(made_at, 1, 19) || '.'
    || substr(CASE WHEN substr(made_at, 20, 1) = '.' THEN substr(made_at, 21, length(made_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE made_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(made_at) <> 30;

UPDATE client_codec_calibrations SET measured_at = substr(measured_at, 1, 19) || '.'
    || substr(CASE WHEN substr(measured_at, 20, 1) = '.' THEN substr(measured_at, 21, length(measured_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE measured_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(measured_at) <> 30;

UPDATE media_source_subtitles SET read_at = substr(read_at, 1, 19) || '.'
    || substr(CASE WHEN substr(read_at, 20, 1) = '.' THEN substr(read_at, 21, length(read_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE read_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(read_at) <> 30;

UPDATE media_source_openings SET listened_at = substr(listened_at, 1, 19) || '.'
    || substr(CASE WHEN substr(listened_at, 20, 1) = '.' THEN substr(listened_at, 21, length(listened_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE listened_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(listened_at) <> 30;

UPDATE pinned_works SET pinned_at = substr(pinned_at, 1, 19) || '.'
    || substr(CASE WHEN substr(pinned_at, 20, 1) = '.' THEN substr(pinned_at, 21, length(pinned_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE pinned_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(pinned_at) <> 30;

UPDATE set_aside_files SET set_aside_at = substr(set_aside_at, 1, 19) || '.'
    || substr(CASE WHEN substr(set_aside_at, 20, 1) = '.' THEN substr(set_aside_at, 21, length(set_aside_at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE set_aside_at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(set_aside_at) <> 30;

UPDATE system_measures SET at = substr(at, 1, 19) || '.'
    || substr(CASE WHEN substr(at, 20, 1) = '.' THEN substr(at, 21, length(at) - 21) ELSE '' END || '000000000', 1, 9)
    || 'Z'
WHERE at GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*Z' AND length(at) <> 30;
