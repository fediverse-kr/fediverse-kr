ALTER TABLE hosting_services
    ADD COLUMN operator_region text NOT NULL DEFAULT 'unknown'
        CHECK (operator_region IN ('korea', 'overseas', 'unknown')),
    ADD COLUMN korean_support boolean,
    ADD COLUMN classification_source_url text NOT NULL DEFAULT ''
        CHECK (char_length(classification_source_url) <= 2048),
    ADD CONSTRAINT hosting_classification_source_required CHECK (
        (operator_region = 'unknown' AND korean_support IS NULL)
        OR classification_source_url <> ''
    );
