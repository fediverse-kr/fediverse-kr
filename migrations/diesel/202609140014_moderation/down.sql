DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM member_admin_roles) OR EXISTS(SELECT 1 FROM moderation_events) THEN
        RAISE EXCEPTION 'Refusing to discard administrator roles or moderation history';
    END IF;
END $$;
DROP INDEX community_reports_status_page;
DROP TABLE moderation_events;
DROP TABLE member_admin_roles;
