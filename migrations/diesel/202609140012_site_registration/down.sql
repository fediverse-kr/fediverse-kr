DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM directory_site_registrations) THEN
        RAISE EXCEPTION 'Refusing to discard registration attribution';
    END IF;
END $$;
DROP TABLE directory_site_registrations;
