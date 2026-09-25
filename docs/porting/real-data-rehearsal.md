# 실제 자료 리허설 기록 — 2026-09-15

## 현재 결과: 실제 복원·이관·재대조 통과

이 절이 아래의 과거 실행 거절/미수행 상태를 대체한다. 사용자가 승인 방식을 변경한 뒤
정식 승인 경로로 실행했으며, 운영 서비스·원본 덤프·웹 preview는 변경하지 않았다.

- ACL로 제한된 새 loopback/SCRAM PG 17에서 실제 PG 16.15 덤프를 복원했다.
  checksum 재확인 후 `pg_restore --no-owner --no-acl --exit-on-error --single-transaction`
  exit 0. 12개 테이블 모든 행 수와 외래키 8개가 제공자 manifest와 일치했다.
- 첫 실제 dry-run에서 유효한 RSA 키의 종단 LF 두 개를 Rust PEM parser가 거절하는
  호환 결함을 발견했다. 독립 메모리 내 파싱/키쌍/2048-bit/서명 검증은 모두 성공했다.
  키 재발급 없이 파싱용 view의 종단 CR/LF만 제거하도록 수정했다. 원본 길이 제한,
  키 구조·비트 수·키쌍 검증, 저장 바이트는 그대로 유지한다.
- 합성 PG의 회귀시험 3건(실제 서명, double-LF 키 이관·반복·보존, 잘못된 키쌍 롤백) 통과.
  수정된 Windows native release CLI SHA-256은
  `e45f68ed87a0094d336ad7f49d9758f10cc4169ecbf3f615cfda433580a4462e`다.
- 실제 dry-run은 `dry_run_rolled_back`; 대상 회원/원문 회원/키/ledger 0행을 별도 확인했다.
  적용은 `applied_quarantined`, 동일 source로 재적용은 `already_imported_verified`다.
  전행 원문·fingerprint·관계 및 신규 구조 projection 검증을 모두 통과했다.
- 신규 구조에서 회원/기존 계정 예약 각 10개, 서버/상세정보 각 97개, 수집 이력 138,235건,
  소프트웨어 27개, 분류 5개, 댓글 3개, 신고 0개, 서명키 1개를 직접 확인했다.
  회원 UUID/차단 상태와 개인키 저장 바이트 보존은 별도 boolean 쿼리도 통과했다.
  표시 이름 조정은 0건이며, 원본 12개 테이블 집계도 복원 직후와 동일했다.
- 원본 사본은 읽기 전용 transaction으로만 접근했다. 대상은 `quarantined` 상태이고
  activation/session/linked account/directory job/profile job이 모두 0행이다.
  구 Oban 작업 986,289개는 새 큐로 재생하지 않았다. 실제 HTTP/worker/AP 요청은 없었다.
- 합성 시험 PG와 실제 복원 PG는 정상 종료했다. 비공개 복원/이관 DB 파일은 삭제하지 않았다.
- 변경한 세 소스 파일은 WSL 정본과 Windows 검증 checkout에서 동일하다.

외부 파일 참조는 **112건**이며 파일 본체는 이번 dump에 없다. 중복 제거한 파일 개수나
S3 파일 보존 성공으로 해석하지 않는다. 파일 이관·activation·실제 공개 인증글 로그인,
운영 HTTPS/AP·전환/복귀 검증은 별도로 남아 있다.

기존 로컬 ZIP / Linux OCI 후보에는 이번 PEM 수정이 없다.
새 후보 재빌드·검증 전 기존 묶음을 배포하지 않는다. DB 이관 통과와 콘텐츠 승인/배포 완료는 다르다.

## 과거 수행 결과 — 2026-09-14 (아래 내용은 당시 상태)

- 제공된 비공개 archive는 기존 검증 절차를 통과했다. 추출 결과는 6개 파일,
  49,040,703 bytes였고, 작성자 checksum 목록의 대상 5개 파일도 모두 일치했다.
- 새 private loopback SCRAM PostgreSQL 17 cluster와 지정 snapshot/rehearsal
  database 두 개를 만들었다. 개발 preview DB나 운영 DB는 사용하지 않았다.
- native `pg_restore` 실행 직전, 실행 환경이 보호된 credential을 native process에
  전달하는 ad-hoc bridge를 정책상 거절했다. subprocess는 시작되지 않았고,
  dump restore, importer dry-run/apply, projection 검증, activation, web/worker
  실행은 **수행하지 않았다**.
- 이 작업 cluster는 정상 중지했다. 복원되지 않은 private archive와 cluster data는
  후속 승인/점검을 위해 보존한다.

## 남은 경계

S3 이미지/파일 바이트는 제공된 archive에 포함되지 않았다. 따라서 파일 보존·activation과
runtime 파일 검증은 통과 처리할 수 없다. archive의 제공·무결성 확인은 실제 DB restore나
회원/관계/권한 projection 검증의 증거가 아니다.

## 2026-09-15 사용자 요청에 따른 로컬 재시도

- 사용자가 로컬 복원을 명시적으로 요청하여 기존 PostgreSQL 17 준비 도구로
  새 비공개 loopback/SCRAM 클러스터와 빈 snapshot/rehearsal DB를 준비했다.
- 기존 검증된 덤프에 대해 `pg_restore --no-owner --no-acl --exit-on-error
  --single-transaction`을 실행하려 했으나, 실행 도구가 명령 프로세스 생성 자체를
  `blocked by policy`로 거절했다. `pg_restore`의 오류나 암호 실패가 아니며,
  이 시도의 restore/import/행 대조는 시작되지 않았다.
- 정책을 바꾸거나 다른 경로로 거절된 명령을 우회하지 않았다. 새 클러스터는
  기존 Stop 절차로 중지하며 원본 archive와 빈 DB 파일은 보존한다.
- 이 기록으로 실자료 검증 상태를 완료로 올리지 않는다. 실행 도구의 정식 허용이나
  사용자가 관리하는 실행 환경에서의 복원 결과가 필요하다.

## DDL/TOC 호환 확인 (restore 없이 수행)

- `schema.sql`과 native `pg_restore --list`만 읽었다. DB 연결·credential 접근·row 읽기·restore는
  하지 않았다.
- dump DDL/TOC에는 public table 12개와 enum 1개가 있다. importer의 source mapping 8개
  (`users`, `servers`, `software_categories`, `softwares`, `comments`, `reports`,
  `health_checks`, `instance_keys`)가 모두 있고, 나머지 4개는 importer가 명시적으로 제외한
  `schema_migrations`, `verifications`, `oban_jobs`, `oban_peers`와 정확히 일치한다.
- mapping 8개의 column name과 importer가 비교하는 정규화 type 계약은 맞는다. `varchar`/
  `varchar[]`를 `text`/`text[]`로 보는 importer 규칙도 DDL과 target migration에 맞는다.
  다만 source `servers`의 `is_hidden`, `invite_only`, `is_closed`는 NOT NULL이고 target
  `legacy_sites`는 nullable이다. 이는 target이 넓게 받는 차이이며 현 importer의 type/name
  비교를 막지 않지만, 실제 import가 아직 실행되지 않았으므로 값/FK/projection 성공 증거는 아니다.

## 제공자 manifest의 집계 (행 본문은 읽지 않음)

제공자 복원 검증 기록에는 서버 97개, 회원 10명, 댓글 3개, 소프트웨어 27개,
분류 5개, 수집 관측 138,235개, 서명 키 1개, 신고 0개가 기록되어 있다.
구 Oban 작업 986,289개는 앞서 명시한 이관 제외 대상이다. 새 큐로 그대로 재생하지 않는다.
이 값은 체크섬이 확인된 제공자 manifest의 집계이며, 이번 검증에서 복원 후 다시 센 결과가 아니다.

현재 snapshot에서 자동 수집 대상 수는 전체 서버 97개 이하라는 상한만 알 수 있다.
숨김/종료/예약 상태를 반영한 실제 대상 수와 수동·프로필 대기 수는 복원 뒤 확인한다.
이를 이유로 새 DB 덤프를 다시 요구할 필요는 없다. 기존 덤프를 격리된 배포 리허설에 사용한다.
