-- New local membership domain. Deliberately does not alter Phoenix users or data.
-- Apply explicitly with the migration command, never automatically at startup.
CREATE TABLE member_users (
    id UUID PRIMARY KEY,
    login_id TEXT UNIQUE
        CHECK (login_id ~ '^[a-z0-9_]{3,32}$'),
    display_name TEXT NOT NULL
        CHECK (char_length(display_name) BETWEEN 1 AND 64),
    password_hash TEXT CHECK (password_hash LIKE '$argon2id$%'),
    CHECK ((login_id IS NULL) = (password_hash IS NULL)),
    is_banned BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE member_sessions (
    id UUID PRIMARY KEY,
    member_id UUID NOT NULL REFERENCES member_users(id) ON DELETE CASCADE,
    token_hash BYTEA NOT NULL UNIQUE CHECK (octet_length(token_hash) = 32),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    UNIQUE (id, member_id),
    CHECK (expires_at > created_at)
);
CREATE INDEX member_sessions_member_idx ON member_sessions(member_id);
CREATE INDEX member_sessions_expiry_idx ON member_sessions(expires_at);

-- Federation identities are attached to a member, never the membership primary key.
-- No access/refresh token or third-party password is stored here.
CREATE TABLE member_linked_accounts (
    id UUID PRIMARY KEY,
    member_id UUID NOT NULL REFERENCES member_users(id) ON DELETE CASCADE,
    actor_url TEXT NOT NULL UNIQUE,
    handle TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    profile_url TEXT NOT NULL,
    provider TEXT NOT NULL CHECK (provider = 'activitypub_post'),
    provider_origin TEXT NOT NULL,
    provider_subject_id TEXT NOT NULL,
    is_public BOOLEAN NOT NULL DEFAULT FALSE,
    verified_at TIMESTAMPTZ NOT NULL,
    account_created_at TIMESTAMPTZ,
    account_created_at_verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (provider_origin, provider_subject_id)
);
CREATE INDEX member_linked_accounts_member_idx ON member_linked_accounts(member_id);

-- A future verified callback must consume this row and attach the identity in one
-- transaction. Merely inserting a challenge or knowing a handle grants no rights.
CREATE TABLE member_link_challenges (
    id UUID PRIMARY KEY,
    purpose TEXT NOT NULL CHECK (purpose IN ('login', 'link')),
    member_id UUID REFERENCES member_users(id) ON DELETE CASCADE,
    session_id UUID,
    browser_binding_hash BYTEA NOT NULL CHECK (octet_length(browser_binding_hash) = 32),
    code_hash BYTEA NOT NULL UNIQUE CHECK (octet_length(code_hash) = 32),
    actor_url TEXT NOT NULL,
    handle TEXT NOT NULL,
    display_name TEXT NOT NULL,
    profile_url TEXT NOT NULL,
    outbox_url TEXT NOT NULL,
    actor_published_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    verified_post_url TEXT,
    CHECK (expires_at > created_at),
    CHECK (
        (purpose = 'login' AND member_id IS NULL AND session_id IS NULL)
        OR (purpose = 'link' AND member_id IS NOT NULL AND session_id IS NOT NULL)
    ),
    FOREIGN KEY (session_id, member_id)
        REFERENCES member_sessions(id, member_id) ON DELETE CASCADE
);
CREATE INDEX member_link_challenges_expiry_idx ON member_link_challenges(expires_at);

-- Fixed-size hashed keys avoid storing attacker-controlled login strings. Rows
-- expire in 15 minutes and bounded cleanup runs on attempts and maintenance.
CREATE TABLE member_auth_rate_limits (
    scope TEXT NOT NULL CHECK (scope IN ('login', 'password_change', 'challenge_begin', 'challenge_verify')),
    key_hash BYTEA NOT NULL CHECK (octet_length(key_hash) = 32),
    attempts INTEGER NOT NULL CHECK (attempts BETWEEN 1 AND 1000000),
    resets_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (scope, key_hash)
);
CREATE INDEX member_auth_rate_limits_expiry_idx ON member_auth_rate_limits(resets_at);
