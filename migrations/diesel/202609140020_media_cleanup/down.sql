DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM media_deletions) THEN
  RAISE EXCEPTION 'Pending media deletion work must be resolved before rollback';
 END IF;
END $$;
DROP TABLE media_deletions;
