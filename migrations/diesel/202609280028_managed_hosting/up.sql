CREATE TABLE hosting_services (
    slug text PRIMARY KEY CHECK (length(slug) BETWEEN 1 AND 64),
    name text NOT NULL CHECK (length(name) BETWEEN 1 AND 160),
    website_url text NOT NULL CHECK (length(website_url) BETWEEN 1 AND 2048),
    scope text NOT NULL DEFAULT '' CHECK (length(scope) <= 240),
    software text NOT NULL DEFAULT '' CHECK (length(software) <= 240),
    provider_responsibilities text NOT NULL DEFAULT '' CHECK (length(provider_responsibilities) <= 1000),
    customer_responsibilities text NOT NULL DEFAULT '' CHECK (length(customer_responsibilities) <= 1000),
    source_url text NOT NULL DEFAULT '' CHECK (length(source_url) <= 2048),
    checked_on date,
    revision bigint NOT NULL DEFAULT 1 CHECK (revision >= 1),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE hosting_service_edits (
    slug text NOT NULL REFERENCES hosting_services(slug) ON DELETE CASCADE,
    revision bigint NOT NULL CHECK (revision >= 1),
    actor_id uuid REFERENCES member_users(id) ON DELETE SET NULL,
    action text NOT NULL CHECK (action IN ('create','edit','restore')),
    summary text NOT NULL CHECK (length(summary) BETWEEN 1 AND 200),
    snapshot jsonb NOT NULL CHECK (octet_length(snapshot::text) <= 65536),
    created_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (slug, revision)
);
CREATE UNIQUE INDEX hosting_services_website_unique ON hosting_services(lower(website_url));
CREATE INDEX hosting_service_edits_actor_time ON hosting_service_edits(actor_id, created_at);
