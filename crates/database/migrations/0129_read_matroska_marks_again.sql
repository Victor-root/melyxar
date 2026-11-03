-- Where the picture of every Matroska film can be started, read again.
--
-- A mark of the film's own index can stand for several tracks at once, and
-- only the first was looked at: a picture listed after a line of words
-- beginning on the same instant lost its boundary. The playlist then counted
-- one segment fewer than the tool cut, and every segment after it held the one
-- before it. Read again from that index, which costs a few thousand bytes a
-- film, the next pass of the upkeep puts every one of them right.
DELETE FROM media_source_key_frames
 WHERE source_id IN (SELECT id FROM media_sources WHERE container LIKE 'matroska%');
