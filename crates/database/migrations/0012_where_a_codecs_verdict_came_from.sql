-- Where a codec's verdict came from, because the two sources are not equal.
--
-- A test plays a film this server generates for the purpose. However hard that
-- film is made to look, it is not a real one: measured on real hardware, the
-- same codec at the same size and the same bitrate lost under one picture in a
-- hundred of the generated film and a third of a real one. The cost of
-- decoding does not follow the number of bits, it follows the coding tools a
-- real scene forces an encoder to reach for, and no generated picture asks for
-- those the way a filmed one does.
--
-- So what a real film did on this machine outranks what the test did, and the
-- row says which it was. A test never overwrites what watching established;
-- forgetting this device's calibration is what clears the slate.
ALTER TABLE client_codec_calibrations
    ADD COLUMN found_by TEXT NOT NULL DEFAULT 'test';
