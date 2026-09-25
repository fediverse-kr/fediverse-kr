ALTER TABLE directory_site_details DROP COLUMN refresh_job_id, DROP COLUMN refresh_requested_at;
ALTER TABLE directory_jobs DROP COLUMN owner_requested;
