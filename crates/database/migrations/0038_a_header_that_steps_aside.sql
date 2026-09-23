-- Whether the bar at the top steps aside while somebody reads down a page.
--
-- Going down, it slides away and leaves the whole window to the page; the
-- first move back up brings it back, and it stays until the page is read
-- down again. Some people would rather it never moved, so it is theirs to
-- choose, and it moves unless they said otherwise.
ALTER TABLE user_preferences ADD COLUMN header_hides_on_scroll INTEGER NOT NULL DEFAULT 1;
