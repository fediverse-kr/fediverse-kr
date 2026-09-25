-- Session rotation is not a new authentication. Preserve the original time,
-- and never treat pre-migration sessions as proof of a fresh federated login.
ALTER TABLE member_sessions ADD COLUMN authenticated_at timestamptz;
UPDATE member_sessions SET authenticated_at = created_at;
ALTER TABLE member_sessions ALTER COLUMN authenticated_at SET NOT NULL;
ALTER TABLE member_sessions ALTER COLUMN authenticated_at SET DEFAULT now();
ALTER TABLE member_sessions ADD COLUMN federated_authenticated_at timestamptz;
ALTER TABLE member_sessions ADD CONSTRAINT member_sessions_authentication_time
    CHECK (authenticated_at <= created_at AND
           (federated_authenticated_at IS NULL OR federated_authenticated_at <= authenticated_at));
