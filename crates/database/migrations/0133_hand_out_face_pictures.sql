-- Every account given a picture of its own once, in the order they were
-- made, the ones without a photo first: different from each other up to the
-- ten there are.
WITH ranked AS (
    SELECT id,
           (ROW_NUMBER() OVER (ORDER BY avatar_path IS NOT NULL, created_at, id) - 1) % 10 AS at
      FROM users
)
UPDATE users
   SET face_picture = (
       SELECT CASE at
                  WHEN 0 THEN 'comet'
                  WHEN 1 THEN 'rings'
                  WHEN 2 THEN 'orbits'
                  WHEN 3 THEN 'crystal'
                  WHEN 4 THEN 'eclipse'
                  WHEN 5 THEN 'aurora'
                  WHEN 6 THEN 'crescent'
                  WHEN 7 THEN 'spiral'
                  WHEN 8 THEN 'binary'
                  ELSE 'verdant'
              END
         FROM ranked
        WHERE ranked.id = users.id
   );
