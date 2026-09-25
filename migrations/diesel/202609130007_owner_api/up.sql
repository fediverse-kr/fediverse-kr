ALTER TABLE directory_site_edits DROP CONSTRAINT directory_site_edits_action_check;
ALTER TABLE directory_site_edits ADD CONSTRAINT directory_site_edits_action_check
    CHECK (action IN ('dns_claim','api_claim','edit','resign','withdraw'));
