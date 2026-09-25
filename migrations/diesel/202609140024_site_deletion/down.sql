DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM moderation_events WHERE target_kind='site' OR action='delete_site') THEN
        RAISE EXCEPTION 'Site deletion audit must be preserved before downgrade';
    END IF;
END $$;
ALTER TABLE moderation_events DROP CONSTRAINT moderation_events_target_kind_check;
ALTER TABLE moderation_events ADD CONSTRAINT moderation_events_target_kind_check
    CHECK(target_kind IN ('report','comment','member','admin_role'));
ALTER TABLE moderation_events DROP CONSTRAINT moderation_events_action_check;
ALTER TABLE moderation_events ADD CONSTRAINT moderation_events_action_check
    CHECK(action IN ('resolve','dismiss','reopen','delete_comment','ban','unban','grant','revoke'));
