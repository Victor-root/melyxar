-- How much of the film a client really showed, beside how much it dropped.
--
-- Dropping pictures is only one of the two ways a decoder fails, and it turned
-- out to be the rarer one. A decoder that cannot keep up at all does not drop
-- pictures: it never produces them in the first place, shows three a second
-- where the film wants twenty four, and reports nothing dropped because
-- nothing it produced was ever thrown away. Measured against a hidden picture
-- that is the whole of what is seen, and a codec no machine could play reads
-- as a codec that played perfectly.
--
-- So what a row now carries is both: the share of pictures the decoder threw
-- away, and the share of the pictures the film asked for that ever appeared at
-- all. A calibration is only good news when both of them are.
ALTER TABLE client_codec_calibrations
    ADD COLUMN shown_share REAL NOT NULL DEFAULT 0;
