DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM catalog_admin_events WHERE action='logo') THEN
  RAISE EXCEPTION 'Logo audit records require migration 021; no history was discarded';
 END IF;
END $$;
ALTER TABLE catalog_admin_events DROP CONSTRAINT catalog_admin_events_action_check;
ALTER TABLE catalog_admin_events ADD CONSTRAINT catalog_admin_events_action_check
 CHECK(action IN ('locked','brand_color','featured','display_order','category_create','category_edit'));
