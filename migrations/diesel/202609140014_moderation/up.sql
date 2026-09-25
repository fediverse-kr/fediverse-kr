-- Roles belong to internal members, never a self-asserted federated handle.
CREATE TABLE member_admin_roles (
    member_id uuid PRIMARY KEY REFERENCES member_users(id) ON DELETE CASCADE,
    granted_at timestamptz NOT NULL DEFAULT now()
);
-- Deliberately no FK on target/report: preserve the event after withdrawal,
-- without locking another member while holding a comment/report lock.
CREATE TABLE moderation_events (
    id uuid PRIMARY KEY,
    actor_id uuid REFERENCES member_users(id) ON DELETE SET NULL,
    target_kind text NOT NULL CHECK(target_kind IN ('report','comment','member','admin_role')),
    target_id uuid NOT NULL,
    report_id uuid,
    action text NOT NULL CHECK(action IN ('resolve','dismiss','reopen','delete_comment','ban','unban','grant','revoke')),
    before_state text NOT NULL CHECK(octet_length(before_state)<=100),
    after_state text NOT NULL CHECK(octet_length(after_state)<=100),
    note text NOT NULL CHECK(char_length(note)<=1000 AND octet_length(note)<=4000),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX moderation_events_report ON moderation_events(report_id,created_at DESC,id DESC);
CREATE INDEX community_reports_status_page ON community_reports(status,inserted_at DESC,id DESC);
