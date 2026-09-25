-- Original comment/report columns remain unchanged for exact offline import.
CREATE INDEX community_thread_page ON community_comments(server_id,parent_id,inserted_at DESC,id DESC);
CREATE INDEX community_comment_rate ON community_comments(user_id,server_id,inserted_at DESC);
CREATE INDEX community_report_rate ON community_reports(reporter_id,inserted_at DESC);
CREATE TABLE community_report_evidence (
    report_id uuid PRIMARY KEY REFERENCES community_reports(id) ON DELETE CASCADE,
    body text NOT NULL CHECK(octet_length(body)<=1048576),
    author_name text NOT NULL,
    comment_updated_at timestamp NOT NULL
);
