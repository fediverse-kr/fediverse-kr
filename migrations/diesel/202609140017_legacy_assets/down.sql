DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM legacy_asset_manifest) OR EXISTS(SELECT 1 FROM legacy_asset_files) THEN
        RAISE EXCEPTION 'Refusing to discard the retained asset inventory';
    END IF;
END $$;
DROP TABLE legacy_asset_files;
DROP TABLE legacy_asset_manifest;
