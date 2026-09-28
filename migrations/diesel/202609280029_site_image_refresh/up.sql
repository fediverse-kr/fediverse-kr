-- Site-level completion also remembers a successful crawl with no usable image.
-- Existing bytes remain untouched; the leased worker refreshes at its normal pace.
ALTER TABLE directory_sites ADD COLUMN icon_collection_version SMALLINT NOT NULL DEFAULT 0 CHECK (icon_collection_version >= 0);
