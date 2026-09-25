-- Do not silently lose a retained icon that the previous constraint cannot
-- represent. Operators must remove or convert it deliberately before downgrade.
DO $$ BEGIN
    IF EXISTS(
        SELECT 1 FROM directory_icons
        WHERE mime NOT IN ('image/png','image/jpeg','image/gif','image/webp','image/x-icon')
    ) THEN
        RAISE EXCEPTION 'Newer directory icon MIME must be preserved before downgrade';
    END IF;
END $$;
ALTER TABLE directory_icons DROP CONSTRAINT directory_icons_mime_check;
ALTER TABLE directory_icons ADD CONSTRAINT directory_icons_mime_check
    CHECK (mime IN ('image/png','image/jpeg','image/gif','image/webp','image/x-icon'));
