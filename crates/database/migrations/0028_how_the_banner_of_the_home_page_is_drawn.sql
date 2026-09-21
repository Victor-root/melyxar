-- What each person decides about the banner their home page opens on.
--
-- Its height is a share of the screen's width rather than of its height,
-- because what it really sets is how much of the picture behind it survives:
-- those pictures are sixteen by nine, so a third of the width keeps about two
-- thirds of one. Where the band is cut runs from nought at the top of the
-- picture to one at its foot.
--
-- Both were fixed numbers written into the stylesheet, measured over a shelf
-- of real pictures. A library is not a shelf, so they are numbers anybody can
-- move rather than numbers only a rebuild can change.
--
-- The third says whether the banner draws a fresh handful every time the page
-- is opened, in place of what was left halfway and what has just arrived.
-- What an administrator put there by hand still comes first either way: a
-- choice made deliberately is not something a shuffle undoes.
ALTER TABLE user_preferences ADD COLUMN banner_height REAL NOT NULL DEFAULT 0.33;
ALTER TABLE user_preferences ADD COLUMN banner_cut REAL NOT NULL DEFAULT 0.25;
ALTER TABLE user_preferences ADD COLUMN banner_at_random INTEGER NOT NULL DEFAULT 0;
