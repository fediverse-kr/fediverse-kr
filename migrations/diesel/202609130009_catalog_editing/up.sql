-- Preserve the imported catalog rows and their source column contract unchanged.
CREATE TABLE catalog_software_state (
    software_id bigint PRIMARY KEY REFERENCES catalog_software(id) ON DELETE CASCADE,
    revision bigint NOT NULL DEFAULT 0 CHECK (revision >= 0),
    locked boolean NOT NULL DEFAULT false
);
CREATE TABLE catalog_software_edits (
    software_id bigint NOT NULL REFERENCES catalog_software(id) ON DELETE CASCADE,
    revision bigint NOT NULL CHECK (revision >= 0),
    actor_id uuid REFERENCES member_users(id) ON DELETE SET NULL,
    action text NOT NULL CHECK (action IN ('baseline','create','edit','restore')),
    summary text NOT NULL CHECK (length(summary) <= 200),
    snapshot jsonb NOT NULL CHECK (octet_length(snapshot::text) <= 1048576),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (software_id, revision)
);
CREATE INDEX catalog_edits_member_time ON catalog_software_edits(actor_id, created_at);
