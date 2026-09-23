-- Whether the home page opens on its banner at all.
--
-- Hidden, the page starts with its rows. It is shown unless the account said
-- otherwise, like every other setting of the banner.
ALTER TABLE user_preferences ADD COLUMN banner_shown INTEGER NOT NULL DEFAULT 1;
