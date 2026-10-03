-- A device's calibration, kept whole or not at all.
--
-- The table it replaces kept one row per codec, written as each codec was
-- measured and as films were watched: a run stopped halfway left half a
-- calibration behind, and one slow film could rule a codec out for good.
-- What it held was measured the old way and is not carried over.
DROP TABLE client_codec_calibrations;

CREATE TABLE device_calibrations (
    client_id           TEXT PRIMARY KEY,
    calibration_version INTEGER NOT NULL,
    measured_at         TEXT NOT NULL,
    -- Every codec the server offered, with the tallest height each played
    -- cleanly and what each height measured, as written by the server.
    results             TEXT NOT NULL
) STRICT;
