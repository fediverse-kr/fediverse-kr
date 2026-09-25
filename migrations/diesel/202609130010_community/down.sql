DO $$ BEGIN
    IF EXISTS(SELECT 1 FROM community_report_evidence) THEN
        RAISE EXCEPTION 'Report evidence exists; explicit archival required before downgrade';
    END IF;
END $$;
DROP TABLE community_report_evidence;
DROP INDEX community_report_rate;
DROP INDEX community_comment_rate;
DROP INDEX community_thread_page;
