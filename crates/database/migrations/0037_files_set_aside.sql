-- Files taken out of a library while they stay on the disk.
--
-- Somebody who removes a film from the library and not from the disk has
-- said they do not want it here. Without a note of it, the next scan finds
-- the file and writes it down again, and the removal lasts until then. A
-- path noted here is walked past; a file renamed or moved is another path,
-- and comes back.
CREATE TABLE set_aside_files (
    root_id       TEXT NOT NULL REFERENCES library_roots (id) ON DELETE CASCADE,
    relative_path TEXT NOT NULL,
    set_aside_at  TEXT NOT NULL,
    PRIMARY KEY (root_id, relative_path)
) STRICT;
