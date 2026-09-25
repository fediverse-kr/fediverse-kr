DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM catalog_admin_events) OR EXISTS(SELECT 1 FROM catalog_category_state)
       OR EXISTS(SELECT 1 FROM catalog_software_edits WHERE action IN ('admin_edit','admin_settings')) THEN
        RAISE EXCEPTION 'Refusing to discard catalog administration history';
    END IF;
END $$;
DROP TABLE catalog_admin_events;
DROP TABLE catalog_category_state;
ALTER TABLE catalog_software_edits DROP CONSTRAINT catalog_software_edits_action_check;
ALTER TABLE catalog_software_edits ADD CONSTRAINT catalog_software_edits_action_check
    CHECK(action IN ('baseline','create','edit','restore'));
