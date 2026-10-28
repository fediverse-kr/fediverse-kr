ALTER TABLE catalog_admin_events DROP CONSTRAINT catalog_admin_events_action_check;
ALTER TABLE catalog_admin_events
    ADD CONSTRAINT catalog_admin_events_action_check
    CHECK (action IN ('locked','brand_color','featured','display_order','category_create','category_edit','logo','header','media_import'));

ALTER TABLE catalog_software_state
    ADD COLUMN header_key text,
    ADD COLUMN logo_metadata jsonb,
    ADD COLUMN header_metadata jsonb;

ALTER TABLE catalog_software_state
    ADD CONSTRAINT catalog_software_state_header_key_scope
        CHECK (header_key IS NULL OR header_key LIKE 'software-headers/%'),
    ADD CONSTRAINT catalog_software_state_logo_metadata_size
        CHECK (logo_metadata IS NULL OR octet_length(logo_metadata::text) <= 16384),
    ADD CONSTRAINT catalog_software_state_header_metadata_size
        CHECK (header_metadata IS NULL OR octet_length(header_metadata::text) <= 16384);
