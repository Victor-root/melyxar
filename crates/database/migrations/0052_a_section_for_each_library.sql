-- The rows of the newest works of each kind of library become sections of
-- their own, each moved and hidden apart, where they were one section for
-- all of them. And they now come before the row of everything newest until
-- somebody chooses otherwise.

-- An account still on the order it was given moves with the new one; one
-- that chose its own keeps it.
UPDATE user_preferences
SET home_sections = 'band,carry_on,up_next,libraries,recently_added'
WHERE home_sections = 'band,carry_on,up_next,recently_added,libraries';

-- The one section becomes a section per kind, in the order the account
-- already gave its kinds, at the place the one section had. Hidden, all of
-- them stay hidden.
UPDATE user_preferences
SET home_sections = replace(
        home_sections, 'libraries', 'newest:' || replace(home_order, ',', ',newest:')),
    hidden_home_sections = replace(
        hidden_home_sections, 'libraries', 'newest:' || replace(home_order, ',', ',newest:'));
