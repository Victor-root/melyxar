-- Whether wide gamut colour is ever converted for a viewer who cannot show it,
-- everywhere on this server.
--
-- The conversion redraws every pixel of every frame, which is what turns a
-- film that would otherwise be served as it lies on the disk into a full
-- rebuild. On a processor too slow to keep up with that, the automatic rule
-- alone can mean nothing plays at a usable speed. This is the operator's own
-- switch to skip it everywhere rather than film by film, at the cost of a
-- washed out, grey picture on a wide gamut film. Dolby Vision without a
-- compatible base layer is never affected by it: left unconverted it looks
-- broken rather than merely washed out, so it is still converted regardless.
--
-- Off by default, since the automatic rule is the right answer for the
-- server this project is built for.

ALTER TABLE server_settings ADD COLUMN tone_mapping_disabled INTEGER NOT NULL DEFAULT 0;
