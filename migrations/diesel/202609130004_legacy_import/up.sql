-- Offline Phoenix snapshot retention. No HTTP endpoint reads these tables.
-- Original columns, NULLs, UUIDs and UTC-naive Ecto timestamps are retained.
CREATE TABLE legacy_members (
    id uuid PRIMARY KEY REFERENCES member_users(id) DEFERRABLE INITIALLY DEFERRED,
    fediverse_handle text NOT NULL UNIQUE,
    fediverse_domain text NOT NULL,
    display_name text,
    avatar_url text,
    avatar_key text,
    emojis jsonb,
    verified_at timestamp,
    account_created_at timestamp,
    last_login_at timestamp,
    is_banned boolean,
    inserted_at timestamp NOT NULL,
    updated_at timestamp NOT NULL
);
-- A handle is not an immutable AP actor ID. Reserve it; never invent an actor
-- URL or silently attach the legacy membership to whoever holds the handle now.
CREATE TABLE member_legacy_claims (
    member_id uuid PRIMARY KEY REFERENCES member_users(id) DEFERRABLE INITIALLY DEFERRED,
    handle text NOT NULL UNIQUE
);
CREATE TABLE legacy_sites (
    id uuid PRIMARY KEY REFERENCES directory_sites(id) DEFERRABLE INITIALLY DEFERRED,
    domain text NOT NULL UNIQUE,
    name text, software text, software_version text, description text,
    language text, registration_open boolean, approval_required boolean,
    tags text[], thumbnail_url text, rules text,
    user_count integer, active_user_count integer, status_count integer,
    last_checked_at timestamp, is_alive boolean, response_time_ms integer,
    is_hidden boolean, invite_only boolean, favicon_key text,
    admin_verified_via text, admin_comment text, is_force_hidden boolean NOT NULL,
    is_closed boolean, avg_response_time_7d integer,
    admin_user_id uuid REFERENCES member_users(id) DEFERRABLE INITIALLY DEFERRED,
    inserted_at timestamp NOT NULL, updated_at timestamp NOT NULL
);
CREATE TABLE catalog_categories (
    id bigint PRIMARY KEY, name text NOT NULL UNIQUE, label text NOT NULL,
    emoji text, display_order integer NOT NULL,
    inserted_at timestamp NOT NULL, updated_at timestamp NOT NULL
);
CREATE TABLE catalog_software (
    id bigint PRIMARY KEY, name text NOT NULL UNIQUE, display_name text NOT NULL,
    family text, logo_key text, description text, tech_stack text,
    features text[], website_url text, category_tag text, categories text[],
    brand_color text, is_featured boolean NOT NULL, display_order integer NOT NULL,
    inserted_at timestamp NOT NULL, updated_at timestamp NOT NULL
);
CREATE TABLE community_comments (
    id uuid PRIMARY KEY,
    user_id uuid REFERENCES member_users(id) DEFERRABLE INITIALLY DEFERRED,
    server_id uuid NOT NULL REFERENCES directory_sites(id) DEFERRABLE INITIALLY DEFERRED,
    parent_id uuid REFERENCES community_comments(id) DEFERRABLE INITIALLY DEFERRED,
    body text NOT NULL, is_deleted boolean,
    inserted_at timestamp NOT NULL, updated_at timestamp NOT NULL
);
CREATE INDEX community_comments_server_time ON community_comments(server_id, inserted_at);
CREATE TABLE community_reports (
    id uuid PRIMARY KEY,
    reporter_id uuid REFERENCES member_users(id) DEFERRABLE INITIALLY DEFERRED,
    comment_id uuid REFERENCES community_comments(id) DEFERRABLE INITIALLY DEFERRED,
    resolved_by_id uuid REFERENCES member_users(id) DEFERRABLE INITIALLY DEFERRED,
    reason text NOT NULL, detail text, status text NOT NULL,
    admin_note text, resolved_at timestamp,
    inserted_at timestamp NOT NULL, updated_at timestamp NOT NULL,
    UNIQUE(reporter_id, comment_id)
);
CREATE TABLE legacy_health_checks (
    id uuid PRIMARY KEY,
    server_id uuid NOT NULL REFERENCES directory_sites(id) DEFERRABLE INITIALLY DEFERRED,
    is_alive boolean NOT NULL, response_time_ms integer, status_code integer,
    error text, checked_at timestamp NOT NULL
);
CREATE TABLE legacy_instance_keys (
    id uuid PRIMARY KEY, public_key_pem text NOT NULL, private_key_pem text NOT NULL,
    inserted_at timestamp NOT NULL, updated_at timestamp NOT NULL
);
CREATE TABLE legacy_import_state (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    source_fingerprint text NOT NULL CHECK (length(source_fingerprint) = 64),
    format_version integer NOT NULL CHECK (format_version = 1),
    state text NOT NULL CHECK (state = 'quarantined'),
    imported_at timestamptz NOT NULL DEFAULT now()
);
