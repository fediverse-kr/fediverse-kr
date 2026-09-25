-- Private asset bundle index, not permission to publish retained profile media.
CREATE TABLE legacy_asset_manifest (
    singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton)
        REFERENCES legacy_import_state(singleton),
    source_fingerprint text NOT NULL CHECK(source_fingerprint ~ '^[0-9a-f]{64}$'),
    object_count bigint NOT NULL CHECK(object_count>=0),
    byte_count bigint NOT NULL CHECK(byte_count>=0),
    verified_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE legacy_asset_files (
    object_key text PRIMARY KEY CHECK(octet_length(object_key) BETWEEN 1 AND 1024),
    manifest boolean NOT NULL DEFAULT true CHECK(manifest)
        REFERENCES legacy_asset_manifest(singleton) DEFERRABLE INITIALLY DEFERRED,
    sha256 text NOT NULL CHECK(sha256 ~ '^[0-9a-f]{64}$'),
    byte_count bigint NOT NULL CHECK(byte_count BETWEEN 0 AND 16777216)
);
CREATE INDEX legacy_asset_files_hash ON legacy_asset_files(sha256);
