-- Phoenix authenticated a normalized handle, then reused users.id. Preserve
-- that first-login contract; pin the verified actor after the first new proof.
ALTER TABLE member_legacy_claims
    ADD COLUMN actor_url text UNIQUE,
    ADD COLUMN claimed_at timestamptz,
    ADD CONSTRAINT legacy_claim_binding_complete CHECK (
        (actor_url IS NULL AND claimed_at IS NULL) OR
        (actor_url IS NOT NULL AND claimed_at IS NOT NULL AND
         char_length(actor_url) BETWEEN 1 AND 2048)
    );
