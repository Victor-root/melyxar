-- What the page of one person says beyond their name: the provider is asked
-- the first time somebody opens it, and the answer is kept here.
--
-- described_at is when it was asked and answered. Empty means never, and the
-- page asks: a person nobody opens costs the provider nothing.
ALTER TABLE people ADD COLUMN biography TEXT;
ALTER TABLE people ADD COLUMN born_on TEXT;
ALTER TABLE people ADD COLUMN died_on TEXT;
ALTER TABLE people ADD COLUMN birthplace TEXT;
ALTER TABLE people ADD COLUMN described_at TEXT;
