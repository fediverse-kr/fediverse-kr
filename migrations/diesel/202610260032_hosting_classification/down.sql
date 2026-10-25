-- Never silently discard a curated classification on downgrade.
DO $$ BEGIN
    IF EXISTS (SELECT 1 FROM hosting_services WHERE operator_region <> 'unknown'
        OR korean_support IS NOT NULL OR classification_source_url <> '') THEN
        RAISE EXCEPTION 'Hosting classifications must be preserved before downgrade';
    END IF;
END $$;
ALTER TABLE hosting_services DROP CONSTRAINT hosting_classification_source_required,
    DROP COLUMN operator_region, DROP COLUMN korean_support, DROP COLUMN classification_source_url;
