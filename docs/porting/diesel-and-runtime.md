# Diesel 전환과 단일 프로세스 워커 — 2026-09-13 (현재 상태 2026-09-14)

이 문서는 해당 전환 시점의 설계 기록이다. 당시의 미완료 목록은 아래에 historical 기록으로
보존한다. 회원 관리·디렉터리·아이콘 조회·소프트웨어 편집·이관·배포 수명주기는 후속 문서에서
구현과 합성 검증을 연결했다. 최신 범위는 [교체 준비 현황](../cutover-readiness.md), 실제 실행
파일과 웹 파일의 구성은 [배포 수명주기 검증](release-lifecycle.md)을 따른다.

## 확정된 선택

- SQLx와 관련 crate를 전부 제거했다. Diesel 2.3.13 / diesel-async 0.9.2 / diesel_migrations 2.3.2를 사용한다.
- `backend/auth.rs`, `flow.rs`는 회원·자격증명·세션·공개 인증글 모델을 다룬다. Diesel schema/Queryable/SQL/연결 풀은 `backend/db/` 안에만 둔다. HTTP에는 `Database`의 도메인 API만 전달된다.
- 일반 CRUD에 Diesel의 타입 검사되는 DSL을 사용한다. PG 고유 작업 선점·advisory lock·대량 정리·윈도 집계·migration 이력 인계와 JSONB projection/조건부 소유권 갱신은 저장소 안의 고정 SQL도 사용한다. 사용자 입력은 bind하며 SQL 문자열에 연결하지 않는다. ORM/SQL은 도메인·HTTP 모듈 밖의 DB adapter에만 둔다.
- 비밀번호는 **Argon2id만** 사용한다. bcrypt 의존성·인증·fallback·해시 변환은 없다. 원래 Argon2id PHC 해시와 UUID/세션/연결 계정/서명키는 보존한다.
- HTTP와 수집·정리 서비스는 **`fediversekr2` 한 바이너리, 한 프로세스**다. 추가 컨테이너, worker executable, init-container, NATS, RabbitMQ가 없다. PG는 기존 애플리케이션 DB를 사용한다.

## 마이그레이션

`migrations/diesel/`의 SQL은 `embed_migrations!`로 바이너리에 포함된다. 배포물에 SQL 파일이나 Diesel CLI를 따로 설치할 필요가 없다. `build.rs`가 migration 변경 시 재컴파일을 보장한다.

DB가 설정된 서버는 시작할 때 내장 migration → 지속 서명키 확보 → HTTP/백그라운드 서비스 순서로 초기화한다. 실패하면 서비스를 시작하지 않는다. 여러 프로세스가 동시에 시작하더라도 PG transaction advisory lock으로 직렬화하고, 인계와 pending migration은 한 트랜잭션으로 처리한다.

이미 SQLx로 준비한 개발 DB에서는 알려진 2개 migration의 성공 여부와 SHA-384 체크섬을 확인한 뒤 Diesel 이력으로 인계한다. Git 줄바꿈 차이(LF/CRLF)는 허용하되 다른 SQL 내용/알 수 없는 버전은 거절한다. 기존 `_sqlx_migrations`는 이력 보존용일 뿐 SQLx 런타임 의존성이 아니다. 기존 flat SQL 두 파일도 이 체크섬 검증용 원본이다. 테이블을 재생성하거나 기존 데이터를 삭제하지 않는다.

선택형 개발 명령도 같은 바이너리다:

```powershell
cargo run --no-default-features --features server --bin fediversekr2 -- --migrate
```

이 명시적 개발 명령은 기존 loopback:16439의 `fedkr_dev`/`fedkr_test` allowlist를 유지한다. **보통 실행에는 명령을 따로 실행할 필요가 없다.** DB 환경변수가 없는 UI preview는 DB나 워커를 만들지 않는다. 운영용 자동 마이그레이션 코드를 작성했지만 실제 운영 DB에는 연결/실행하지 않았다.

연결 풀은 bb8 + AsyncPgConnection, 최대 8개다. 로컬 literal loopback 이외 DB는 Rustls 인증서 검증과 TLS를 강제한다.
선택형 `FEDKR_DATABASE_CA_FILE`로 DB 전용 사설 CA 묶음을 추가할 수 있다. 이 값이 설정되면
loopback도 TLS 필수이며 인증서/호스트 검증을 끌 수 없다. 파일 규칙과 실제 검증은
[PostgreSQL TLS](postgres-tls.md)를 따른다. 클라이언트 인증서는 아직 지원하지 않는다.
앱 실행에 libpq 설치는 필요하지 않다.

## 워커 동작

기존 MIT Phoenix 소스의 실제 worker/NodeInfo 구현을 기준으로 포팅했다.

- 30분: `is_closed=false`인 등록 사이트를 현재 주기에 한 번 예약. `is_hidden`은 목록 숨김이므로 기존처럼 수집한다. 종료 중 빠진 과거 주기를 무한히 재생하지 않는다.
- 운영자의 명시적 수동 요청은 서버당 1시간 간격으로 같은 큐에 예약한다. 이 요청만 종료 서버의 단발 조회와 기존 파비콘 갱신을 허용하며, 종료 상태나 자동 수집 대상을 바꾸지 않는다. [권한/상태 의미](site-ownership.md).
- 최대 5개 동시 수집. 건강 상태 GET, NodeInfo 2.0/2.1, 파비콘 발견·다운로드를 수행한다.
- PG에 영속 작업 저장, `SKIP LOCKED` 선점, 120초 임대, 토큰 fencing, 같은 사이트 동시 실행 금지. 프로세스 실패로 만료된 작업은 최대 3회 시도 후 dead로 남긴다.
- 원격 HTTP 실패는 정상 수집 결과로 기록하고 다음 30분 주기에 다시 관측한다. 워커 장애와 원격 사이트 장애를 같은 재시도로 취급하지 않는다.
- HTTP 요청당 10초, 전체 수집 60초, 본문 256 KiB/아이콘 512 KiB. 공개 HTTPS와 확인한 DNS 주소를 연결에 고정하며, redirect는 최대 3번이고 매번 같은 안전 검사를 한다. AP 인증 GET의 redirect 금지 정책은 완화하지 않았다.
- 결과·최신 관측값·이력·작업 완료를 한 트랜잭션에 저장한다. 중복 완료/만료된 임대의 결과/자동 수집 도중 닫힌 사이트의 결과를 거부한다. 수동 요청은 DB에 저장된 플래그로만 종료 검사를 예외 처리한다.
- 6시간(00/06/12/18시 15분 UTC 기준 slot): 최근 7일 alive 응답 최소 20개, 양끝 10%를 반올림해 제거한 평균. 유효 표본이 사라지면 이전 평균도 지운다.
- 1시간: 만료 인증 데이터 각 1,000행, 30일 지난 이력·작업 각 10,000행 한도 정리. 평균·정리 실행권도 PG에서 직렬화한다. 이 규모를 넘는 지속적 backlog는 한도/주기 조정 대상이다.

`directory_sites`의 수동 이름·설명·숨김과 `directory_observations`의 원격 이름·설명·통계는 분리한다. NodeInfo 실패 시 이전 관측과 그 시각을 유지하고 실패를 별도 기록한다. 성공한 응답에서 누락된 수치는 **미상(NULL)**이지 0명/가입 불가가 아니다.

파비콘/서버 아이콘은 수집 결과를 PG의 작은 bytea 캐시에 저장한다. 공개 조회는 현재
`directory_icons` 캐시를 우선하고, 이관된 파비콘은 OpenDAL의 `stored_files`/`favicons/` 매핑으로
fallback한다. PNG/JPEG/GIF/WebP/ICO뿐 아니라 AVIF/BMP/TIFF도 시그니처와 저장 MIME을 대조해
제공한다. 이는 전체 디코딩 검증이 아니다. SVG도 기존 `quick-xml` 기반의 크기·깊이 제한과
단일 SVG 루트 검사로 식별하며, DTD/처리 명령/깨진 XML은 거부한다. 스크립트를 제거하는
sanitizer가 아니므로 별도 이미지 응답의 공통 CSP/sandbox가 실행·외부 리소스를 차단한다.
인라인 스타일과 data 이미지는 허용한다. HTML을 SVG로 제공하지 않는다.

내장 migration 025는 `directory_icons`의 저장 MIME 목록도 이 식별 정책에 맞춘다.
기존 바이트를 변경하지 않으며 이전 제약으로 표현할 수 없는 이미지가 있으면 downgrade를
거부한다. 이관된 `favicons/` 키에 실제 파일 매핑이 있으면 자동 수집은 이를 보존하고,
운영자의 명시적 갱신은 다시 수집한다. 아이콘 공개 서빙·목록 UI는 [서버 찾기](server-discovery.md),
실제 실행 결과와 원격 검증의 한계는 [통합 검증](integration-checks.md)을 따른다. S3 운영 설정은 별도다.

## Kameo 검토

[Kameo supervision](https://docs.page/tqwewe/kameo/core-concepts/supervision)은 프로세스 내 자식 생명주기, 실패 재시작 정책과 mailbox가 필요한 서비스에는 유익하다. 다만 현재 작업은 PG에서 가져온 독립 HTTP 수집과 주기별 정리이며, 액터가 소유해야 할 장기 도메인 상태나 메시지 협업이 없다.

따라서 이번에는 추가하지 않았다. Tokio child task를 감독해 실패 시 1–60초 지수 backoff로 재시작하고, 종료 시 신규 선점을 멈추고 최대 25초 배출한다. 강제 종료 시에도 PG 임대가 복구 기준이다. DB 작업의 재시도/중복 방지와 프로세스 감독은 별개다. 향후 AP inbox/outbox, 호스트별 상태 머신과 정책 명령이 생기면 그 실행 계층을 Kameo로 바꿀 수 있게 crawler/domain/db/runtime 모듈을 나눴다.

개발 빌드는 Dioxus의 기존 hot reload를 유지하며, router가 다시 만들어져도 워커는 프로세스당 한 번만 시작한다. Release 빌드는 Axum의 graceful shutdown으로 HTTP와 백그라운드를 함께 종료한다.

## 현재 구현과 운영 경계 — 2026-09-14

다음은 후속 구현으로 연결된 현재 상태다. 합성 PG/로컬 HTTP/배포물 검증은 실제 백업·실서버·
운영 전환의 증거가 아니다.

- 회원 생명주기와 프로필 사진·이모지 갱신은 세션·탈퇴·연결 해제·출처 선택·같은 프로세스
  갱신 워커까지 연결했다. 실제 회원 백업과 외부 AP 성공 상호운용은 [회원 생명주기](member-lifecycle.md),
  [프로필 미디어 갱신](profile-media-refresh.md)의 미완료 경계에 남아 있다.
- 서버 조회·응답 이력·아이콘 fallback과 소프트웨어 독립 상세 조회는 연결했다. 필터/페이지/숨김과
  운영 수집 대상의 실데이터 검증은 [서버 찾기](server-discovery.md)에서 별도로 남긴다.
- 소프트웨어 회원 편집·관리자 편집·종류 관리는 연결했다. 공개 이력 제한 열람과 삭제/병합 등
  정책, 운영 자료 편집은 [소프트웨어 기여](software-contributions.md), [카탈로그 관리](catalog-moderation.md)의
  현재 미완료 범위다.
- 오프라인 import/assets/activation과 Windows·Linux release lifecycle 검사는 연결했다. 실제
  snapshot/파일 백업 대조, 운영 호스트·TLS/프록시·실 AP·트래픽 전환은 완료하지 않았다.

## 당시 미완료 목록 — 2026-09-13 historical 기록

아래 항목은 이 문서가 작성된 시점의 표현을 보존한 것이다. 위의 현재 상태와 섞어 읽지 않는다.

후속: [legacy-import.md](legacy-import.md)의 소스 계약 기반 오프라인 이관/보존 도구를 추가했다. 아래 항목 중 snapshot **도구 부재**는 해소했지만, 실제 데이터 검증·회원 귀속·기능 parity는 여전히 미완료다.

- Phoenix 실제 snapshot 검증/귀속, 프로필 이미지 내용 이전, 계정 복구·탈퇴·연결 해제·공개 설정. 추가 회원 필드/emojis는 격리 이관에서 보존한다.
- 기존 서버 전체 필드의 실데이터 이관 검증/export 및 더미 목록 UI의 실제 조회 연결. `add_site`는 내부 경계일 뿐 공개 등록 기능이 아니며, 오프라인 이관은 별도 CLI를 쓴다.
- 소개 페이지 재기획, 소프트웨어 종류·유저 직접 편집·디자인, 운영/호스팅 사업자 정보의 위키식 편집 정책. 이 작업에서 임의로 정하지 않았다.
- 실 SNS/실운영 DB 검증, 사설 CA/TLS DB 실연결, 배포·컷오버. 테스트는 격리 개발 PG와 모의 원격 응답을 사용한다.

전체 사이트 기능 parity 또는 컷오버 완료를 뜻하지 않는다.
