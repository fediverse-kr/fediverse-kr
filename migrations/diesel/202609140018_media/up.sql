-- Runtime logical keys, separately usable from the offline import ledger.
-- No public endpoint accepts these keys or their content hashes.
CREATE TABLE stored_files (
 object_key text PRIMARY KEY CHECK(octet_length(object_key) BETWEEN 1 AND 1024),
 sha256 text NOT NULL CHECK(sha256 ~ '^[0-9a-f]{64}$'),
 byte_count bigint NOT NULL CHECK(byte_count BETWEEN 0 AND 16777216)
);
CREATE INDEX stored_files_hash ON stored_files(sha256);
-- Existing quarantined bundles gain the same runtime projection. This does not
-- remove their quarantine; the offline tool still rechecks rows and file bytes.
INSERT INTO stored_files(object_key,sha256,byte_count)
 SELECT object_key,sha256,byte_count FROM legacy_asset_files;
