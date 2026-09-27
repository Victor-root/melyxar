-- The entries of the account's menu become buttons that can be put on the
-- bar too, so what is kept is now which buttons are on the bar rather than
-- which ones left it: an empty list would otherwise put all of them there.
--
-- Every account starts again from the usual bar. The choice being replaced
-- was only a day old, and its order is kept.
ALTER TABLE user_preferences
    ADD COLUMN buttons_in_the_bar TEXT NOT NULL DEFAULT 'search,favourites,watch_later,notifications';
ALTER TABLE user_preferences DROP COLUMN buttons_in_the_menu;
