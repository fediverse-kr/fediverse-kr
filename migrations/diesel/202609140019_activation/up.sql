-- Keep the import ledger forever; activation is a separate explicit approval.
CREATE TABLE legacy_runtime_activation (
 singleton boolean PRIMARY KEY CHECK(singleton) REFERENCES legacy_import_state(singleton),
 format_version integer NOT NULL CHECK(format_version=1),
 source_fingerprint text NOT NULL CHECK(source_fingerprint ~ '^[0-9a-f]{64}$'),
 database_name text NOT NULL CHECK(database_name ~ '^fedkr_live_[a-z0-9_]{1,32}$'),
 public_origin text NOT NULL CHECK(octet_length(public_origin) BETWEEN 1 AND 2048),
 asset_objects bigint NOT NULL CHECK(asset_objects>=0),
 asset_bytes bigint NOT NULL CHECK(asset_bytes>=0),
 activated_at timestamptz NOT NULL DEFAULT now()
);
