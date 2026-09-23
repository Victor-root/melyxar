-- The browser a device is, as its own page found it.
--
-- The line a browser sends about itself is kept in the name, and it cannot
-- tell every browser apart: Brave sends exactly what Chrome sends, on
-- purpose. The page can ask the browser itself, and says what it found here.
-- Absent until it has, which is every device signed in before this column.
ALTER TABLE devices ADD COLUMN browser TEXT;
