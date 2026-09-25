-- Shared software revisions serialize member edits and administrative settings.
ALTER TABLE catalog_software_edits DROP CONSTRAINT catalog_software_edits_action_check;
ALTER TABLE catalog_software_edits ADD CONSTRAINT catalog_software_edits_action_check
    CHECK(action IN ('baseline','create','edit','restore','admin_edit','admin_settings'));
CREATE TABLE catalog_category_state (
    category_id bigint PRIMARY KEY REFERENCES catalog_categories(id) ON DELETE CASCADE,
    revision bigint NOT NULL DEFAULT 0 CHECK(revision >= 0)
);
-- Reasons and actor identities are private, unlike software content summaries.
CREATE TABLE catalog_admin_events (
    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    software_id bigint REFERENCES catalog_software(id) ON DELETE CASCADE,
    category_id bigint REFERENCES catalog_categories(id) ON DELETE CASCADE,
    revision bigint NOT NULL CHECK(revision > 0),
    actor_id uuid REFERENCES member_users(id) ON DELETE SET NULL,
    action text NOT NULL CHECK(action IN ('locked','brand_color','featured','display_order','category_create','category_edit')),
    note text NOT NULL CHECK(char_length(note) BETWEEN 1 AND 1000 AND octet_length(note)<=4000),
    before_value jsonb NOT NULL CHECK(octet_length(before_value::text)<=1048576),
    after_value jsonb NOT NULL CHECK(octet_length(after_value::text)<=1048576),
    created_at timestamptz NOT NULL DEFAULT now(),
    CHECK(num_nonnulls(software_id,category_id)=1),
    UNIQUE(software_id,revision), UNIQUE(category_id,revision)
);
CREATE INDEX catalog_admin_events_actor_time ON catalog_admin_events(actor_id,created_at);
