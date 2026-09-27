-- Whether a library keeps where each account stopped, so a work is picked up
-- from there, and whether it keeps which works each account has watched.
--
-- Two answers rather than one, because they are asked for apart: a library of
-- home videos may want neither, a library of short clips only the marks. On
-- for both, which is what every library did until now.
--
-- Turned off, nothing is erased: what was kept before stays in the progress of
-- each account and simply goes unshown, and turning it back on shows it again.

ALTER TABLE libraries ADD COLUMN keeps_resume_points INTEGER NOT NULL DEFAULT 1;
ALTER TABLE libraries ADD COLUMN keeps_watched_marks INTEGER NOT NULL DEFAULT 1;
