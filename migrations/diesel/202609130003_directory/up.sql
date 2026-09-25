-- Editorial records and observed remote values are deliberately separate.
-- No membership editing/approval policy is inferred by the collector.
CREATE TABLE directory_sites (
    id uuid PRIMARY KEY,
    domain text NOT NULL UNIQUE CHECK (length(domain) BETWEEN 3 AND 253),
    name text,
    description text,
    is_hidden boolean NOT NULL DEFAULT false,
    is_force_hidden boolean NOT NULL DEFAULT false,
    is_closed boolean NOT NULL DEFAULT false,
    created_at timestamptz NOT NULL DEFAULT now(),
    updated_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE directory_observations (
    site_id uuid PRIMARY KEY REFERENCES directory_sites(id) ON DELETE CASCADE,
    is_alive boolean NOT NULL,
    response_time_ms integer CHECK (response_time_ms >= 0),
    status_code integer CHECK (status_code BETWEEN 100 AND 599),
    health_error text,
    checked_at timestamptz NOT NULL,
    software text,
    software_version text,
    observed_name text,
    observed_description text,
    registration_open boolean,
    user_count bigint CHECK (user_count >= 0),
    active_user_count bigint CHECK (active_user_count >= 0),
    status_count bigint CHECK (status_count >= 0),
    nodeinfo_checked_at timestamptz,
    nodeinfo_error text,
    avg_response_time_7d integer CHECK (avg_response_time_7d >= 0)
);
CREATE TABLE directory_jobs (
    id uuid PRIMARY KEY,
    site_id uuid NOT NULL REFERENCES directory_sites(id) ON DELETE CASCADE,
    scheduled_at timestamptz NOT NULL,
    state text NOT NULL DEFAULT 'pending' CHECK (state IN ('pending','running','complete','dead')),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts BETWEEN 0 AND 3),
    lease_token uuid,
    lease_until timestamptz,
    last_error text,
    completed_at timestamptz,
    UNIQUE (site_id, scheduled_at),
    CHECK ((state = 'running') = (lease_token IS NOT NULL AND lease_until IS NOT NULL))
);
CREATE INDEX directory_jobs_ready_idx ON directory_jobs(state, scheduled_at, lease_until);
CREATE UNIQUE INDEX directory_jobs_one_running_site ON directory_jobs(site_id) WHERE state = 'running';
CREATE TABLE directory_health_checks (
    job_id uuid PRIMARY KEY,
    site_id uuid NOT NULL REFERENCES directory_sites(id) ON DELETE CASCADE,
    is_alive boolean NOT NULL,
    response_time_ms integer CHECK (response_time_ms >= 0),
    status_code integer CHECK (status_code BETWEEN 100 AND 599),
    error text,
    checked_at timestamptz NOT NULL
);
CREATE INDEX directory_health_site_time_idx ON directory_health_checks(site_id, checked_at DESC);
CREATE INDEX directory_health_expiry_idx ON directory_health_checks(checked_at);
CREATE TABLE directory_icons (
    site_id uuid PRIMARY KEY REFERENCES directory_sites(id) ON DELETE CASCADE,
    mime text NOT NULL CHECK (mime IN ('image/png','image/jpeg','image/gif','image/webp','image/x-icon')),
    bytes bytea NOT NULL CHECK (octet_length(bytes) BETWEEN 1 AND 524288),
    fetched_at timestamptz NOT NULL DEFAULT now()
);
CREATE TABLE maintenance_schedule (
    name text PRIMARY KEY CHECK (name IN ('cleanup','averages')),
    last_slot bigint NOT NULL
);
