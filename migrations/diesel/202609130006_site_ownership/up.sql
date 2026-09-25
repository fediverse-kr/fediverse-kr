-- Editable data is separate from observations and the retained Phoenix snapshot.
CREATE TABLE directory_site_details (
    site_id uuid PRIMARY KEY REFERENCES directory_sites(id) ON DELETE CASCADE DEFERRABLE INITIALLY DEFERRED,
    owner_id uuid REFERENCES member_users(id) ON DELETE SET NULL DEFERRABLE INITIALLY DEFERRED,
    owner_method text,
    rules text,
    language text,
    tags text[],
    invite_only boolean,
    approval_required boolean,
    owner_comment text,
    revision bigint NOT NULL DEFAULT 0 CHECK (revision >= 0),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX directory_site_details_owner ON directory_site_details(owner_id);
CREATE TABLE directory_owner_challenges (
    id uuid PRIMARY KEY,
    member_id uuid NOT NULL REFERENCES member_users(id) ON DELETE CASCADE,
    session_id uuid NOT NULL REFERENCES member_sessions(id) ON DELETE CASCADE,
    domain text NOT NULL,
    code_hash bytea NOT NULL CHECK (octet_length(code_hash)=32),
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL DEFAULT now()+interval '15 minutes',
    CHECK (expires_at>created_at),
    UNIQUE(member_id,domain)
);
CREATE INDEX directory_owner_challenges_expiry ON directory_owner_challenges(expires_at);
CREATE TABLE directory_site_edits (
    id uuid PRIMARY KEY,
    site_id uuid NOT NULL REFERENCES directory_sites(id) ON DELETE CASCADE,
    actor_id uuid REFERENCES member_users(id) ON DELETE SET NULL,
    action text NOT NULL CHECK(action IN ('dns_claim','edit','resign','withdraw')),
    revision bigint NOT NULL,
    previous jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT now(),
    UNIQUE(site_id,revision)
);
