-- Synthetic source fixture, independently specified from the MIT Phoenix Ecto
-- migration contract. No production data or copied upstream implementation.
CREATE TABLE users (
 id uuid PRIMARY KEY, fediverse_handle varchar(255) NOT NULL UNIQUE,
 fediverse_domain varchar(255) NOT NULL, display_name varchar(255), avatar_url varchar(255),
 avatar_key varchar(255), emojis jsonb DEFAULT '{}', verified_at timestamp,
 account_created_at timestamp, last_login_at timestamp, is_banned boolean DEFAULT false,
 inserted_at timestamp NOT NULL, updated_at timestamp NOT NULL
);
CREATE TABLE servers (
 id uuid PRIMARY KEY, domain varchar(255) NOT NULL UNIQUE, name varchar(255),
 software varchar(255), software_version varchar(255), description text,
 language varchar(255), registration_open boolean, approval_required boolean,
 tags varchar(255)[], thumbnail_url varchar(255), rules text, user_count integer,
 active_user_count integer, status_count integer, last_checked_at timestamp,
 is_alive boolean, response_time_ms integer, is_hidden boolean, invite_only boolean,
 favicon_key varchar(255), admin_verified_via varchar(255), admin_comment text,
 is_force_hidden boolean NOT NULL DEFAULT false, is_closed boolean,
 avg_response_time_7d integer, admin_user_id uuid,
 inserted_at timestamp NOT NULL, updated_at timestamp NOT NULL
);
CREATE TABLE software_categories (
 id bigint PRIMARY KEY, name varchar(255) NOT NULL UNIQUE, label varchar(255) NOT NULL,
 emoji varchar(255), display_order integer NOT NULL,
 inserted_at timestamp NOT NULL, updated_at timestamp NOT NULL
);
CREATE TABLE softwares (
 id bigint PRIMARY KEY, name varchar(255) NOT NULL UNIQUE, display_name varchar(255) NOT NULL,
 family varchar(255), logo_key varchar(255), description text, tech_stack varchar(255),
 features varchar(255)[], website_url varchar(255), category_tag varchar(255), categories varchar(255)[],
 brand_color varchar(255), is_featured boolean NOT NULL, display_order integer NOT NULL,
 inserted_at timestamp NOT NULL, updated_at timestamp NOT NULL
);
CREATE TABLE comments (
 id uuid PRIMARY KEY, user_id uuid, server_id uuid NOT NULL, parent_id uuid,
 body text NOT NULL, is_deleted boolean, inserted_at timestamp NOT NULL, updated_at timestamp NOT NULL
);
CREATE TABLE reports (
 id uuid PRIMARY KEY, reporter_id uuid, comment_id uuid, resolved_by_id uuid,
 reason varchar(255) NOT NULL, detail text, status varchar(255) NOT NULL,
 admin_note text, resolved_at timestamp, inserted_at timestamp NOT NULL, updated_at timestamp NOT NULL
);
CREATE TABLE health_checks (
 id uuid PRIMARY KEY, server_id uuid NOT NULL, is_alive boolean NOT NULL,
 response_time_ms integer, status_code integer, error varchar(255), checked_at timestamp NOT NULL
);
CREATE TABLE instance_keys (
 id uuid PRIMARY KEY, public_key_pem text NOT NULL, private_key_pem text NOT NULL,
 inserted_at timestamp NOT NULL, updated_at timestamp NOT NULL
);
CREATE TABLE verifications (id uuid PRIMARY KEY, code text);
CREATE TABLE oban_jobs (id bigint PRIMARY KEY, args jsonb);
CREATE TABLE schema_migrations (version bigint PRIMARY KEY);
INSERT INTO users VALUES
 ('00000000-0000-0000-0000-000000000001','alice@social.example.com','social.example.com',repeat('가',70),NULL,'avatars/fixture.png','{"smile":"emojis/social.example.com/smile.png"}','2025-02-03 04:05:06','2020-01-01','2026-08-01',false,'2025-02-03 04:05:06','2026-08-01'),
 ('00000000-0000-0000-0000-000000000002','banned@social.example.com','social.example.com',NULL,NULL,NULL,NULL,NULL,NULL,NULL,true,'2025-02-03 04:05:06','2026-08-01');
INSERT INTO servers(id,domain,name,software,description,language,registration_open,approval_required,tags,rules,user_count,active_user_count,status_count,last_checked_at,is_alive,response_time_ms,is_hidden,invite_only,favicon_key,admin_verified_via,admin_comment,is_force_hidden,is_closed,avg_response_time_7d,admin_user_id,inserted_at,updated_at) VALUES
 ('10000000-0000-0000-0000-000000000001','social.example.com','테스트 사이트','fixture','<script>untrusted</script>','ko',true,true,ARRAY['그림','개인'],'테스트 규칙',123,12,456,'2026-09-12 12:34:56',true,42,true,true,'favicons/fixture.png','dns','원문 코멘트',false,false,40,'00000000-0000-0000-0000-000000000001','2025-01-01','2026-09-12'),
 ('10000000-0000-0000-0000-000000000002','closed.example',NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,'2026-08-01',false,NULL,false,false,NULL,NULL,NULL,true,true,NULL,NULL,'2025-01-01','2026-08-01'),
 ('10000000-0000-0000-0000-000000000003','unknown.example',NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,NULL,false,NULL,NULL,NULL,'2025-01-01','2026-08-01');
INSERT INTO software_categories VALUES(17,'drawing','그림','🎨',2,'2025-01-01','2026-08-01');
INSERT INTO softwares VALUES(43,'fixture','테스트 소프트웨어','fixture','software-logos/fixture.png','설명',NULL,ARRAY['기능'],NULL,NULL,ARRAY['drawing'],'#123456',true,5,'2025-01-01','2026-08-01');
-- The child sorts before its parent; import must defer FK checks, not drop them.
INSERT INTO comments VALUES
 ('20000000-0000-0000-0000-000000000001',NULL,'10000000-0000-0000-0000-000000000001','20000000-0000-0000-0000-000000000002','탈퇴한 회원의 답글',true,'2025-02-03','2025-02-04'),
 ('20000000-0000-0000-0000-000000000002','00000000-0000-0000-0000-000000000001','10000000-0000-0000-0000-000000000001',NULL,'부모 댓글',false,'2025-02-02','2025-02-02');
INSERT INTO reports VALUES('30000000-0000-0000-0000-000000000001',NULL,'20000000-0000-0000-0000-000000000002','00000000-0000-0000-0000-000000000002','other','비공개 신고 내용','resolved','비공개 관리자 메모','2025-02-05','2025-02-04','2025-02-05');
INSERT INTO health_checks SELECT ('40000000-0000-0000-0000-'||lpad(n::text,12,'0'))::uuid,'10000000-0000-0000-0000-000000000001',n%3<>0,40+n,CASE WHEN n%3<>0 THEN 200 ELSE 503 END,CASE WHEN n%3=0 THEN 'fixture-error' END,'2026-09-12'::timestamp+interval '1 minute'*n FROM generate_series(1,280) n;
INSERT INTO verifications VALUES('50000000-0000-0000-0000-000000000001','DO-NOT-IMPORT-TEST-CODE');
INSERT INTO oban_jobs VALUES(1,'{"must_not_run":true}');
