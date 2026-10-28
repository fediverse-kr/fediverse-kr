DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM catalog_software_state
        WHERE header_key IS NOT NULL OR logo_metadata IS NOT NULL OR header_metadata IS NOT NULL
    ) THEN
        RAISE EXCEPTION 'software media data must be migrated before downgrade';
    END IF;
END $$;
ALTER TABLE catalog_admin_events DROP CONSTRAINT catalog_admin_events_action_check;
ALTER TABLE catalog_admin_events ADD CONSTRAINT catalog_admin_events_action_check
    CHECK (action IN ('locked','brand_color','featured','display_order','category_create','category_edit','logo'));
ALTER TABLE catalog_software_state
    DROP CONSTRAINT catalog_software_state_header_key_scope,
    DROP CONSTRAINT catalog_software_state_logo_metadata_size,
    DROP CONSTRAINT catalog_software_state_header_metadata_size,
    DROP COLUMN header_key,
    DROP COLUMN logo_metadata,
    DROP COLUMN header_metadata;
