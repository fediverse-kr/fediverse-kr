-- Existing crawler-owned icons begin at policy 0 and are retried once with
-- full candidate discovery. Legacy/imported assets have no directory_icons row
-- and therefore remain outside this automatic refresh policy.
ALTER TABLE directory_icons
    ADD COLUMN collection_version smallint NOT NULL DEFAULT 0
    CHECK (collection_version >= 0);
