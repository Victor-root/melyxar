-- The points to look at that an administrator has marked as seen.
--
-- A point born of past events or of a count is quiet until something new
-- happens: the mark is how far it had got when it was seen, a count or the
-- instant it was seen in milliseconds, and the point speaks again once it goes
-- past it. A point that describes something still wrong has no mark and
-- cannot be silenced.
CREATE TABLE attention_seen (
    user_id TEXT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    item    TEXT NOT NULL,
    mark    INTEGER NOT NULL,
    PRIMARY KEY (user_id, item)
) STRICT;

-- The journal is read by kind as well as by date: what failed today, who was
-- refused today.
CREATE INDEX activity_log_by_kind ON activity_log (kind, occurred_at DESC);
