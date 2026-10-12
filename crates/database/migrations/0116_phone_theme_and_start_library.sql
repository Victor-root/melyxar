-- What a phone shows apart from the larger screens: its own colour scheme,
-- nothing meaning the same as there, and the library it opens on in place of
-- the home page, forgotten with the library.
ALTER TABLE user_preferences ADD COLUMN theme_mode_phone TEXT;
ALTER TABLE user_preferences ADD COLUMN start_library_phone TEXT REFERENCES libraries (id) ON DELETE SET NULL;
