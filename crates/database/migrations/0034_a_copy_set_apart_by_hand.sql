-- A work made of a copy somebody took away from the work it sat on.
--
-- Whoever did it said the rules that read names put that file in the wrong
-- place. Those same rules must never put it back there on their own, which is
-- what re-filing an episode that belongs to nothing would otherwise do on the
-- very next scan.
ALTER TABLE works ADD COLUMN set_apart_by_hand INTEGER NOT NULL DEFAULT 0;
