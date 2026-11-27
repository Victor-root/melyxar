-- Whether a series is still made: continuing, ended or canceled, as its
-- provider says, and when that was asked.
--
-- Asked with the rest of a series when it is named, and asked again by the
-- pass that fills in what a named work is missing: soon for one still being
-- made, since that is the answer that changes, and seldom for one that is
-- over. Empty for every series named before this, which that pass asks about
-- on its next run.
ALTER TABLE works ADD COLUMN series_status TEXT;
ALTER TABLE works ADD COLUMN series_status_read_at TEXT;
