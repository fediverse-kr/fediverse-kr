CREATE TABLE directory_headers (
    site_id uuid PRIMARY KEY REFERENCES directory_sites(id) ON DELETE CASCADE,
    mime text NOT NULL CHECK (mime IN ('image/png','image/jpeg','image/gif','image/webp','image/avif','image/bmp','image/tiff','image/x-icon')),
    bytes bytea NOT NULL CHECK (octet_length(bytes) BETWEEN 1 AND 524288),
    fetched_at timestamptz NOT NULL DEFAULT now()
);
