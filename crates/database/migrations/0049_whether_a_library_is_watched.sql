-- Whether a library's folders are watched, so a file added, moved, renamed or
-- removed is taken in at once rather than at the next scan.
--
-- Off by default, like the switch it is modelled on: watching costs the
-- system one watch for every folder, and a large collection can run into the
-- limit the host sets on them.

ALTER TABLE libraries ADD COLUMN watch_in_real_time INTEGER NOT NULL DEFAULT 0;
