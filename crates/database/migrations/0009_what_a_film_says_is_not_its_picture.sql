-- Edges a film says are not part of its picture.
--
-- A film can carry its picture inside a larger frame and say, beside the
-- picture rather than inside it, how much of each edge to leave out. The size
-- such a file announces is the frame; the picture is what is left once these
-- are taken off, and the two are different shapes.
--
-- Read from the file and kept here because everything that shows or rebuilds a
-- picture has to work on the picture. Taking the announced size for the
-- picture turns a wide film into a tall one: the same film, stretched to fill
-- a frame it was never meant to fill.
--
-- Empty for the overwhelming majority of files, which say nothing of the kind.
-- Left null rather than nought so that a file described before this existed is
-- told apart from one that really has no margins.

ALTER TABLE tracks ADD COLUMN margin_top INTEGER;
ALTER TABLE tracks ADD COLUMN margin_bottom INTEGER;
ALTER TABLE tracks ADD COLUMN margin_left INTEGER;
ALTER TABLE tracks ADD COLUMN margin_right INTEGER;
