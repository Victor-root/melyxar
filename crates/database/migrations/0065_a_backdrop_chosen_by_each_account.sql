-- What is drawn behind the pages, chosen by each account: one of the
-- paintings of light, numbered from one, or nothing but the plain surface.
--
-- The first painting is the one every page wore until now, so nobody sees a
-- change until they choose one.
ALTER TABLE user_preferences ADD COLUMN backdrop TEXT NOT NULL DEFAULT 'light';
ALTER TABLE user_preferences ADD COLUMN backdrop_light INTEGER NOT NULL DEFAULT 1;
