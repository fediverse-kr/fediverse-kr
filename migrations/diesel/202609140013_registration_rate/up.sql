ALTER TABLE member_auth_rate_limits DROP CONSTRAINT member_auth_rate_limits_scope_check;
ALTER TABLE member_auth_rate_limits ADD CONSTRAINT member_auth_rate_limits_scope_check
    CHECK(scope IN ('login','password_change','challenge_begin','challenge_verify','site_registration'));
