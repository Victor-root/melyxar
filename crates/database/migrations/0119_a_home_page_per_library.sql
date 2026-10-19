-- The home page lays out one tile and one row per library rather than per
-- kind: a library made apart, films kept for trials say, stays apart rather
-- than mixing with the others of its kind.

-- The order of the kinds becomes an order of libraries: the libraries of a
-- kind where the kind was, by name. A kind the order did not name comes
-- after, in the order everybody starts with, as it always did.
UPDATE user_preferences
SET home_order = coalesce((
    SELECT group_concat(id, ',' ORDER BY
        coalesce(
            nullif(instr(',' || user_preferences.home_order || ',', ',' || kind || ','), 0),
            1000 + instr(',movies,series,anime,home_media,shows,music,', ',' || kind || ',')),
        name, id)
    FROM libraries), '');

-- The row of a kind becomes the rows of its libraries, at the same place and
-- by name, hidden where it was hidden. A kind holding no library leaves an
-- empty place, closed below.
UPDATE user_preferences
SET home_sections = replace(home_sections, 'newest:movies', coalesce((
        SELECT group_concat('newest:' || id, ',' ORDER BY name, id) FROM libraries
        WHERE kind = 'movies'), '')),
    hidden_home_sections = replace(hidden_home_sections, 'newest:movies', coalesce((
        SELECT group_concat('newest:' || id, ',' ORDER BY name, id) FROM libraries
        WHERE kind = 'movies'), ''));

UPDATE user_preferences
SET home_sections = replace(home_sections, 'newest:series', coalesce((
        SELECT group_concat('newest:' || id, ',' ORDER BY name, id) FROM libraries
        WHERE kind = 'series'), '')),
    hidden_home_sections = replace(hidden_home_sections, 'newest:series', coalesce((
        SELECT group_concat('newest:' || id, ',' ORDER BY name, id) FROM libraries
        WHERE kind = 'series'), ''));

UPDATE user_preferences
SET home_sections = replace(home_sections, 'newest:anime', coalesce((
        SELECT group_concat('newest:' || id, ',' ORDER BY name, id) FROM libraries
        WHERE kind = 'anime'), '')),
    hidden_home_sections = replace(hidden_home_sections, 'newest:anime', coalesce((
        SELECT group_concat('newest:' || id, ',' ORDER BY name, id) FROM libraries
        WHERE kind = 'anime'), ''));

UPDATE user_preferences
SET home_sections = replace(home_sections, 'newest:home_media', coalesce((
        SELECT group_concat('newest:' || id, ',' ORDER BY name, id) FROM libraries
        WHERE kind = 'home_media'), '')),
    hidden_home_sections = replace(hidden_home_sections, 'newest:home_media', coalesce((
        SELECT group_concat('newest:' || id, ',' ORDER BY name, id) FROM libraries
        WHERE kind = 'home_media'), ''));

UPDATE user_preferences
SET home_sections = replace(home_sections, 'newest:shows', coalesce((
        SELECT group_concat('newest:' || id, ',' ORDER BY name, id) FROM libraries
        WHERE kind = 'shows'), '')),
    hidden_home_sections = replace(hidden_home_sections, 'newest:shows', coalesce((
        SELECT group_concat('newest:' || id, ',' ORDER BY name, id) FROM libraries
        WHERE kind = 'shows'), ''));

UPDATE user_preferences
SET home_sections = replace(home_sections, 'newest:music', coalesce((
        SELECT group_concat('newest:' || id, ',' ORDER BY name, id) FROM libraries
        WHERE kind = 'music'), '')),
    hidden_home_sections = replace(hidden_home_sections, 'newest:music', coalesce((
        SELECT group_concat('newest:' || id, ',' ORDER BY name, id) FROM libraries
        WHERE kind = 'music'), ''));

-- The empty places closed: six kinds holding nothing in a row leave seven
-- commas, which three passes bring down to one.
UPDATE user_preferences
SET home_sections = trim(replace(replace(replace(
        home_sections, ',,', ','), ',,', ','), ',,', ','), ','),
    hidden_home_sections = trim(replace(replace(replace(
        hidden_home_sections, ',,', ','), ',,', ','), ',,', ','), ',');
