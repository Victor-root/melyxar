-- The titles waiting for a name are counted as a grid shows them, without
-- the episodes of series already named: a mark taken on the old count would
-- keep the point quiet until the new one passed it. Seen again once.
DELETE FROM attention_seen WHERE item = 'unidentified';
