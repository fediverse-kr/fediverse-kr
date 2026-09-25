-- Do not silently discard administrator audit data on downgrade.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM directory_site_edits WHERE action IN ('admin_edit','admin_refresh')) THEN
        RAISE EXCEPTION 'Site administrator history must be preserved before downgrade';
    END IF;
END $$;
ALTER TABLE directory_site_edits DROP CONSTRAINT directory_site_edits_admin_check;
ALTER TABLE directory_site_edits DROP COLUMN admin_note, DROP COLUMN admin_change;
ALTER TABLE directory_site_edits DROP CONSTRAINT directory_site_edits_action_check;
ALTER TABLE directory_site_edits ADD CONSTRAINT directory_site_edits_action_check
    CHECK (action IN ('dns_claim','api_claim','edit','resign','withdraw'));
