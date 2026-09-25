-- Private attribution. Registering a directory entry does not confer ownership.
CREATE TABLE directory_site_registrations (
    site_id uuid PRIMARY KEY REFERENCES directory_sites(id) ON DELETE CASCADE,
    member_id uuid REFERENCES member_users(id) ON DELETE SET NULL,
    created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX directory_registration_member_time ON directory_site_registrations(member_id,created_at);
