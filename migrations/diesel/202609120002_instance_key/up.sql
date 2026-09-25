-- Persistent outbound GET signing identity; not a general federation inbox.
CREATE TABLE federation_instance_keys (
    singleton boolean PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    private_key_pem text NOT NULL CHECK (octet_length(private_key_pem) BETWEEN 1000 AND 16384),
    created_at timestamptz NOT NULL DEFAULT now()
);
