-- Operator-curated with upstream evidence in the description and edit history.
-- NULL means unspecified, not an assertion of active maintenance.
ALTER TABLE catalog_software_state ADD COLUMN development_status text
    CHECK (development_status IN ('discontinued', 'renamed', 'uncertain'));