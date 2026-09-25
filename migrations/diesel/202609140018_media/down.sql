DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM stored_files) THEN
  RAISE EXCEPTION 'Refusing to discard runtime file references';
 END IF;
END $$;
DROP TABLE stored_files;
