-- What one client's own device was really measured decoding.
--
-- Not tied to an account: the same person watches from a laptop and a
-- television, and one measuring badly must never mark the other. The client
-- names itself, with a value it made up and keeps to itself, guarding nothing
-- and proving nothing about who is asking.
--
-- One row per codec this client was asked about, so a single codec can be
-- measured again on its own without disturbing what was already known about
-- the others. calibration_version travels on every row: a calibration made a
-- different way is not compared against one made this way, and a version
-- older than the one this server now writes is offered a fresh calibration
-- rather than trusted forever.
CREATE TABLE client_codec_calibrations (
    client_id           TEXT NOT NULL,
    codec               TEXT NOT NULL,
    calibration_version INTEGER NOT NULL,
    -- Whether this codec, rebuilt at tested_height, played without the
    -- decoder itself dropping pictures.
    usable              INTEGER NOT NULL,
    tested_height       INTEGER NOT NULL,
    -- The share of pictures the decoder dropped during the measurement, kept
    -- for a page that wants to say more than a plain yes or no.
    dropped_share       REAL NOT NULL,
    measured_at         TEXT NOT NULL,
    PRIMARY KEY (client_id, codec)
) STRICT;
