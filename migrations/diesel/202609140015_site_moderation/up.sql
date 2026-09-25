ALTER TABLE directory_site_edits DROP CONSTRAINT directory_site_edits_action_check;
ALTER TABLE directory_site_edits ADD CONSTRAINT directory_site_edits_action_check
    CHECK (action IN ('dns_claim','api_claim','edit','resign','withdraw','admin_edit','admin_refresh'));
ALTER TABLE directory_site_edits ADD COLUMN admin_note text;
ALTER TABLE directory_site_edits ADD COLUMN admin_change jsonb;
ALTER TABLE directory_site_edits ADD CONSTRAINT directory_site_edits_admin_check CHECK (
    CASE WHEN action IN ('admin_edit','admin_refresh') THEN
        admin_note IS NOT NULL AND char_length(admin_note) BETWEEN 1 AND 1000
        AND octet_length(admin_note)<=4000 AND admin_change IS NOT NULL
        AND jsonb_typeof(admin_change)='object' AND octet_length(admin_change::text)<=8192
    ELSE admin_note IS NULL AND admin_change IS NULL END
);
