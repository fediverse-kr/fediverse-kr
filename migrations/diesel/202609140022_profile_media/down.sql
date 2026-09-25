DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM member_profile_media) THEN
        RAISE EXCEPTION 'Profile media exists; preserve it before rollback';
    END IF;
END $$;
DROP TABLE member_profile_media;
