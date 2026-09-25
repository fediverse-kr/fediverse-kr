-- One explicitly requested administrator batch, processed by the same binary.
CREATE TABLE profile_refresh_batches (
    id uuid PRIMARY KEY,
    actor_id uuid REFERENCES member_users(id) ON DELETE SET NULL,
    state text NOT NULL CHECK (state IN ('running','complete','cancelled')),
    created_at timestamptz NOT NULL DEFAULT now(),
    finished_at timestamptz
);
CREATE UNIQUE INDEX profile_refresh_one_batch ON profile_refresh_batches((true)) WHERE state='running';
CREATE TABLE profile_refresh_jobs (
    batch_id uuid NOT NULL REFERENCES profile_refresh_batches(id) ON DELETE CASCADE,
    -- Keep an opaque target/count after withdrawal; never retain remote identity here.
    member_id uuid NOT NULL,
    state text NOT NULL DEFAULT 'pending' CHECK (state IN ('pending','running','succeeded','partial','failed','skipped','cancelled')),
    attempts integer NOT NULL DEFAULT 0 CHECK (attempts BETWEEN 0 AND 3),
    lease_token uuid,
    lease_until timestamptz,
    PRIMARY KEY(batch_id,member_id),
    CHECK ((state='running') = (lease_token IS NOT NULL AND lease_until IS NOT NULL))
);
CREATE INDEX profile_refresh_jobs_ready ON profile_refresh_jobs(batch_id,state,member_id);
