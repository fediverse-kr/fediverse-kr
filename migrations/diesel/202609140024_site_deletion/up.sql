-- Site deletion uses the existing no-FK moderation ledger. The record carries
-- only a stable target ID, revision/count summary and bounded administrator note.
ALTER TABLE moderation_events DROP CONSTRAINT moderation_events_target_kind_check;
ALTER TABLE moderation_events ADD CONSTRAINT moderation_events_target_kind_check
    CHECK(target_kind IN ('report','comment','member','admin_role','site'));
ALTER TABLE moderation_events DROP CONSTRAINT moderation_events_action_check;
ALTER TABLE moderation_events ADD CONSTRAINT moderation_events_action_check
    CHECK(action IN ('resolve','dismiss','reopen','delete_comment','ban','unban','grant','revoke','delete_site'));
