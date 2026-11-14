-- The libraries each account chose not to see while it browses, as the
-- order of the home page is written: identifiers separated by commas.
ALTER TABLE user_preferences ADD COLUMN hidden_libraries TEXT NOT NULL DEFAULT '';
