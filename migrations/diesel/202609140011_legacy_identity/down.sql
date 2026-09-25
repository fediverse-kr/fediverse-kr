DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM member_legacy_claims WHERE actor_url IS NOT NULL) THEN
        RAISE EXCEPTION 'Cannot remove verified legacy identity bindings';
    END IF;
END $$;
ALTER TABLE member_legacy_claims
    DROP CONSTRAINT legacy_claim_binding_complete,
    DROP COLUMN actor_url,
    DROP COLUMN claimed_at;
