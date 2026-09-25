-- A downgrade cannot silently destroy contributions or moderation locks.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM catalog_software_edits) OR EXISTS (SELECT 1 FROM catalog_software_state) THEN
        RAISE EXCEPTION 'Catalog editing data exists; explicit archival is required before downgrade';
    END IF;
END $$;
DROP TABLE catalog_software_edits;
DROP TABLE catalog_software_state;
