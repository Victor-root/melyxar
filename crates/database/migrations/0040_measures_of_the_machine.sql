-- What the machine spent, one line a minute for a week, then one an hour for a
-- year.
--
-- A line is an average over its span, written once the span is over. The
-- instant is where the span starts, written without a fraction of a second, so
-- that lines put side by side as text are put in order.
--
-- Everything the machine may not let the server read is allowed to be absent:
-- a container that hides its temperature has a week of lines without one, not
-- a week of zeroes.
CREATE TABLE system_measures (
    at            TEXT NOT NULL,
    span_seconds  INTEGER NOT NULL,
    processor     REAL,
    memory_used   INTEGER NOT NULL,
    memory_total  INTEGER NOT NULL,
    load          REAL,
    received      REAL NOT NULL,
    sent          REAL NOT NULL,
    card          REAL,
    temperature   REAL,
    PRIMARY KEY (span_seconds, at)
) STRICT;
