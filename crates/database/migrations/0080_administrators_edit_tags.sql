-- An administrator holds every right, and the right to edit the tags of
-- songs came after the accounts that were already administrators.
UPDATE users SET may_edit_tags = 1 WHERE is_administrator = 1;
