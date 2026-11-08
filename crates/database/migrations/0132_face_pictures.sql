-- The picture an account without a photo of its own wears, kept with it.
--
-- It was settled by the name alone, so two accounts of a household could wear
-- the same one. The server now hands out the one the fewest wear when an
-- account is made; the next migration gives those already there theirs.
ALTER TABLE users ADD COLUMN face_picture TEXT NOT NULL DEFAULT 'comet';
