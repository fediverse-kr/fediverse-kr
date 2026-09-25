DO $$ BEGIN
 IF EXISTS(SELECT 1 FROM legacy_runtime_activation) THEN
  RAISE EXCEPTION 'Refusing to discard an activation approval';
 END IF;
END $$;
DROP TABLE legacy_runtime_activation;
