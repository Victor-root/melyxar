-- The button on now jumps thirty seconds until somebody chooses: an account
-- still on the ten seconds everybody was given moves with it.
UPDATE user_preferences SET step_on_seconds = 30 WHERE step_on_seconds = 10;

-- How far back a film starts from where it was left, nought for where it was
-- left, which is what every account had.
ALTER TABLE user_preferences ADD COLUMN resume_rewind_seconds INTEGER NOT NULL DEFAULT 0;

-- When a work left partway counts as started, as watched, or as too short to
-- come back to: below the smallest share it starts again from the beginning,
-- from the largest it is watched, and one shorter than the length is never
-- offered to carry on.
ALTER TABLE user_preferences ADD COLUMN resume_min_percent INTEGER NOT NULL DEFAULT 5;
ALTER TABLE user_preferences ADD COLUMN resume_max_percent INTEGER NOT NULL DEFAULT 90;
ALTER TABLE user_preferences ADD COLUMN resume_min_seconds INTEGER NOT NULL DEFAULT 120;

-- Whether each kind of library has rules of its own, and those given to each,
-- written kind:smallest:largest:seconds and parted by commas.
ALTER TABLE user_preferences ADD COLUMN resume_rules_per_kind INTEGER NOT NULL DEFAULT 0;
ALTER TABLE user_preferences ADD COLUMN resume_rules_by_kind TEXT NOT NULL DEFAULT '';
