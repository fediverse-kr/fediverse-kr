DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM member_auth_rate_limits WHERE scope='account_age_retry') THEN
        RAISE EXCEPTION 'Account age retry state must expire and be cleaned before downgrade';
    END IF;
END $$;
ALTER TABLE member_auth_rate_limits DROP CONSTRAINT member_auth_rate_limits_scope_check;
ALTER TABLE member_auth_rate_limits ADD CONSTRAINT member_auth_rate_limits_scope_check
    CHECK(scope IN ('login','password_change','challenge_begin','challenge_verify','site_registration'));
