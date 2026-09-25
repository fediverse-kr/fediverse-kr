# Phoenix 데이터 이관 준비 — 2026-09-14

**격리 DB에서 보존·변환을 검증하는 오프라인 이관 도구다. 이 명령만으로 운영 전환하지 않는다.**
기준은 MIT Phoenix checkout `3323c7b`의 Ecto migration/schema다. 받은 실제 ZIP은
암호화되어 있어 내용·행 수·체크섬을 확인하지 않았다. 암호 추측, 외부 업로드,
운영 DB 연결, 실제 회원 데이터 복원은 하지 않는다.

## 현재 보존 범위

| 원본 | 보존 테이블 | 새 구조로 연결 |
| --- | --- | --- |
| users | legacy_members | 동일 UUID의 member_users, 비공개 핸들 예약 member_legacy_claims |
| servers | legacy_sites | 동일 UUID의 directory_sites, 관측 시각·상태가 있는 행만 directory_observations |
| health_checks | legacy_health_checks | 기존 UUID를 유지한 directory_health_checks |
| comments | community_comments | 회원·서버·부모 댓글 FK 유지 |
| reports | community_reports | 신고자·대상 댓글·처리자 FK와 비공개 메모 유지 |
| softwares | catalog_software | bigint ID, 분류·계열·내용·로고 키 유지 |
| software_categories | catalog_categories | bigint ID, 이름·표시·정렬 유지 |
| instance_keys | legacy_instance_keys | RSA 키쌍 검증 후 federation_instance_keys에 원래 비밀키 유지 |

원본 필드는 명시적인 타입의 컬럼에 저장한다. 임의 JSON 문서 창고가 아니다.
DB 간 전달 중에만 PostgreSQL JSON 표현을 사용하며, 변환 뒤 **모든 행·컬럼을 원본과
재대조**한다. NULL, 숨김/강제 숨김/종료, 차단, 삭제, 소유자, 배열, 프로필·이모지,
인증/생성/로그인 시각, 신고 메모, 이미지 키를 묵살하지 않는다. FK 검사는 지연했다가
동일 트랜잭션 안에서 반드시 완료하므로 부모보다 먼저 읽힌 답글도 유지된다.

- 원본 Ecto의 UTC-naive timestamp는 그대로 보존하고, 새 timestamptz에는 UTC로 변환한다.
- 표시 이름이 비었거나 64자를 넘으면 새 UI용 이름만 대체/축약한다. 원문은 그대로
  보존하고 변환 건수를 보고한다.
- 기존 NodeInfo 개별 수집 시각은 알 수 없으므로 `now()`로 꾸미지 않는다. 확인되지
  않은 서버 상태는 새 관측값을 만들지 않고 원본에 보존한다.
- 키가 여러 개이거나 RSA 형식/공개키·비밀키 일치 검증에 실패하면 전체 롤백한다.
  원본 키가 없으면 자동 생성하지 않고 보고서에 0개로 나타낸다.
- 만료 인증코드, Phoenix 세션, Oban 작업/피어, Ecto migration 이력은 새 인증/큐로
  복제하지 않는다. 원본에는 그대로 두며, 어떠한 오래된 작업도 실행하지 않는다.

## 안전 경계

- 동일 `fediversekr2` 바이너리의 CLI 경로다. HTTP, 크롤러, AP 요청, 정리, 서명키
  자동 생성 경로를 호출하지 않는다. 추가 의존성·바이너리·컨테이너는 없다.
- source는 literal `127.0.0.1`의 `fedkr_snapshot_*`, target은 `fedkr_rehearsal_*`만
  허용한다. 빈 suffix, query/fragment, 연결 옵션 우회, 개발/테스트/운영 DB명은 거부한다.
  접속 후 실제 DB명과 서버 주소도 대조한다.
- source는 `REPEATABLE READ READ ONLY`. target은 빈 DB 또는 동일 snapshot의 검증된
  격리 이관 결과만 받는다. 기존 대상 데이터의 교체/병합/삭제 모드는 없다.
- 입력 테이블/컬럼/타입이 계약과 다르면 중단한다. 알려지지 않은 업무 테이블도
  무시하지 않는다. SQL은 코드에 고정되어 있고 덤프 SQL을 실행하는 기능은 없다.
- 256행 keyset 페이지, 행 1 MiB/페이지 16 MiB 한도. 출력은 고정 테이블명·건수·상태만.
  회원 정보, 원문 SQL 오류, 연결 URL, 키, snapshot 지문은 CLI에 출력하지 않는다.
- PostgreSQL 오류 로그에도 bound JSON이 남지 않도록 이관 연결의 statement/parameter
  로깅을 끈다. 이 설정 권한이 없으면 데이터를 읽기 전에 실패한다. 격리 클러스터의
  로컬 관리 역할을 사용한다. OS/스토리지·별도 감사 확장의 로깅은 별도로 통제해야 한다.
- 대상 advisory lock으로 준비/검사/이관을 직렬화한다. 데이터와 완료 표시는 한
  트랜잭션이다. 재실행은 원본 지문·전체 보존값·새 구조를 재검증하며 덮어쓰지 않는다.
- 완료 표시가 있는 DB는 이름을 바꿔도 웹/워커 초기화가 거부된다. snapshot 이름이나
  기존 Phoenix users/servers 테이블이 있는 DB도 자동 migration 전에 차단한다.
  이관 명령 자체에는 격리 해제 옵션이 없다. 후속 [검토 사본 활성화](activation.md)는
  원본·파일 재대조와 별도의 명시적 승인으로만 수행한다. 실제 자료를 활성화하지는 않았다.

## 실행

원본 덤프를 별도 비공개 PostgreSQL 클러스터에 복원한 뒤에만 사용한다. 웹 개발용
trust-auth 클러스터에 실제 회원 데이터를 넣지 않는다. 현재 구현은 ZIP 암호를
받거나 pg_restore를 대신 실행하지 않는다. 접근 제한·덤프 출처/무결성·외부 저장소
백업 확인은 실제 복원 전 체크리스트다. 원본 ZIP은 저장소 밖에서 보존한다.

Windows의 [비공개 빈 PG 준비 도구](private-import-cluster.md)는 개발 DB와 별개의
loopback/SCRAM 클러스터·접근 제한·암호 비노출·준비 상태 검사를 제공한다.
이는 격리 환경 준비까지이며, ZIP 해제나 덤프 복원은 수행하지 않는다.
별도의 [암호화 ZIP 준비 도구](private-archive.md)는 접근이 제한된 새 폴더에 해제하고
무결성을 검사한다. 이 도구 역시 SQL/PG 복원이나 실제 이관을 대신하지 않는다.

다음은 **환경변수 이름과 명령 형식**이다. 실제 접속정보는 문서/명령행에 쓰지 않는다.

```text
FEDKR_IMPORT_SOURCE_URL  -> isolated fedkr_snapshot_* database
FEDKR_IMPORT_TARGET_URL  -> empty isolated fedkr_rehearsal_* database

fediversekr2 --legacy-import
fediversekr2 --legacy-import --apply
```

첫 명령은 기본 dry-run: target에 빈 내장 스키마를 준비한 뒤 실제 변환·FK·값 대조까지
수행하고 **전달한 데이터와 완료 표시를 롤백**한다. 빈 스키마 및 Diesel 이력은 남는다.
물리 디스크/WAL에 기록될 수 있으므로 dry-run도 실제 데이터와 같은 보안 수준이 필요하다.
두 번째 명령은 검증에 성공했을 때만 데이터를 `quarantined` 상태로 보관한다.
연결이 COMMIT 응답 중 끊기면 성공 여부를 단정하지 않는다. 같은 snapshot으로
재실행하면 저장된 결과와 전체 값을 대조하고, 이미 완료됐으면 덮어쓰지 않고 확인한다.

## 여전히 필요한 것

1. 실제 암호화 덤프의 스키마/데이터 검증. 현재 증거는 합성 fixture 리허설이다.
2. 실제 사본의 활성화 리허설과 실회원 인증 호환 검증. [활성화 경로](activation.md)와
   **기존 회원 로그인은 연결했다.**
   Phoenix와 같은 정규화 주소에 대해 새 공개 인증글을 검증한 뒤 옛 UUID를 재사용하고,
   최초 검증 actor ID에 고정한다. 과거 actor ID가 없다는 신뢰 한계, 연결 해제/탈퇴와
   재이관 거절은 [기존 회원 로그인 계약](legacy-member-login.md)을 따른다. 격리는 아직 해제하지 않는다.
3. 사용자용 서버 관리·댓글/신고·소프트웨어 직접 편집/조회 UI는 연결했다.
   신규 서버 등록과 관리자 신고 처리·댓글/회원 조치, 사이트 숨김/종료/초대제/태그/수집·이력도 연결했다.
   소프트웨어 잠금/관리자 편집·분류 관리와 [관리자 사이트 삭제](site-moderation.md)도 연결했다.
   새 공개 편집 이력의 제한 열람 정책은 별도 남아 있다.
   새 관리자 사이트/카탈로그 이력이나 분류 편집 상태가 있으면 재이관을 거절한다.
   데이터를 보존했다고 모든 기능 parity까지 달성한 것은 아니다.
4. 이미지 **내용**은 DB가 아니라 외부 스토리지에 있다. 키/URL 보존은 파일 이전이 아니다.
   후속 [파일 보존 묶음](legacy-assets.md)을 추가했다. 이모지 참조까지 포함하며
   별도 export에서 파일 바이트를 보존한다. [런타임 파일 제공](media-serving.md)도 연결했지만,
   실제 파일 백업의 완전성과 운영 저장소 검증은 남았다.
5. 원래 AP actor URL/key ID와 새 공개 endpoint의 동일성 확인, 실서버 호환,
   최종 snapshot 시점·쓰기 중지·전환·복구 계획. 운영 반영은 별도 승인 대상이다.

추천 기능과 6개월 회원 정책은 추가하지 않았다. 랜딩/소개 기획도 변경하지 않았다.
