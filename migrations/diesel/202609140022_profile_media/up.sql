-- Live media is separate from the preserved Phoenix projection.
CREATE TABLE member_profile_media (
    member_id uuid PRIMARY KEY REFERENCES member_users(id) ON DELETE CASCADE,
    source_account_id uuid REFERENCES member_linked_accounts(id) ON DELETE SET NULL,
    avatar_key text,
    emojis jsonb NOT NULL DEFAULT '{}' CHECK (jsonb_typeof(emojis) = 'object'),
    revision bigint NOT NULL DEFAULT 0 CHECK (revision >= 0),
    request_id uuid,
    requested_at timestamptz,
    refreshed_at timestamptz,
    refresh_failed boolean NOT NULL DEFAULT false
);
