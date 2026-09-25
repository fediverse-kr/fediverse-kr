-- Durable owner-requested work uses the same queue and worker, not a new service.
ALTER TABLE directory_jobs ADD COLUMN owner_requested boolean NOT NULL DEFAULT false;
ALTER TABLE directory_site_details
    ADD COLUMN refresh_requested_at timestamptz,
    ADD COLUMN refresh_job_id uuid REFERENCES directory_jobs(id) ON DELETE SET NULL;
