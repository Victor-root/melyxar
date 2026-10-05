-- Whether the bar at the top steps aside while somebody reads down a page on
-- a phone, which is chosen apart from a larger screen: the bar costs a phone
-- far more of its window, so it leaves unless somebody said otherwise.
ALTER TABLE user_preferences ADD COLUMN header_hides_on_scroll_phone INTEGER NOT NULL DEFAULT 1;
