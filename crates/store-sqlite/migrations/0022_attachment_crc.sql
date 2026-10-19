-- Each attachment's CRC-32, which a zip of the demo carries, kept from when
-- it was attached so a download needn't read every file twice. NULL for
-- ones attached before; the hub reads those when it zips them.
ALTER TABLE attachments ADD COLUMN crc32 INTEGER;
