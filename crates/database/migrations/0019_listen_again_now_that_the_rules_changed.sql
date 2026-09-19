-- Everything already listened to is put back in the queue.
--
-- Three things changed in how a season is listened to since these rows were
-- written, and each one changes the answer for seasons that already have one:
-- an opening running up against the edge of what was read is now looked for
-- further in, a stretch under two seconds is read as coincidence rather than
-- as a very short opening, and an audio description is never the track a
-- season is compared on. A row here means never reading that season again, so
-- the only way those rules reach the seasons that need them is to forget what
-- the old ones concluded.
--
-- What a person said themselves stays untouched, here as everywhere: a
-- correction made by hand is not the analysis's to undo.
DELETE FROM media_segments WHERE origin = 'detected';
DELETE FROM media_source_openings;
