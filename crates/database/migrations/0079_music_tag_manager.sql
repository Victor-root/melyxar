-- The tag manager: who may write into the files of songs, whether a library
-- lets its files be written at all, and whether each account sees what is
-- about to change before it is written.
ALTER TABLE users ADD COLUMN may_edit_tags INTEGER NOT NULL DEFAULT 0;
ALTER TABLE music_library_options ADD COLUMN tag_writing INTEGER NOT NULL DEFAULT 0;
ALTER TABLE music_preferences ADD COLUMN tag_preview INTEGER NOT NULL DEFAULT 1;
