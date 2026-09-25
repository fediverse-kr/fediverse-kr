-- A committed removal of a logical file reference must survive HTTP cancellation
-- and process restart. This queue contains no member IDs or original paths.
CREATE TABLE media_deletions (
 sha256 text PRIMARY KEY CHECK(sha256 ~ '^[0-9a-f]{64}$'),
 byte_count bigint NOT NULL CHECK(byte_count BETWEEN 0 AND 16777216),
 attempts integer NOT NULL DEFAULT 0 CHECK(attempts BETWEEN 0 AND 65535),
 available_at timestamptz NOT NULL DEFAULT now(),
 created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX media_deletions_due ON media_deletions(available_at);
