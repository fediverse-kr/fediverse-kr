# 서버 운영자 관리 — 포팅 경계

2026-09-13. Phoenix MIT 참조 소스를 직접 확인한 구현. 운영 서비스에는 반영하지 않았다.

## 현재 연결

- `/account/sites`: 세션으로 소유 서버를 조회하고 24개씩 페이지 나눔. 숨김·강제 숨김 서버도 해당 소유자만 관리할 수 있다.
- DNS TXT: `_fediverse-kr.<domain>` / `fk-verify=<일회용 값>`. 기존 형식을 유지하되 옛 deterministic HMAC 대신 256bit 임의 값, 해시 보관, 15분 만료, 정확한 회원+세션 바인딩을 사용한다. 이미 등록된 서버만 최종 귀속 가능하다. DNS를 조회하기 전에는 등록 여부/숨김 상태를 답하지 않는다.
- DNS 제어권은 기존 API/DNS 소유권보다 우선한다. 귀속 직후 같은 도메인의 다른 대기 인증을 폐기한다. 과거 DNS 레코드나 로그아웃 전 인증 요청으로 재귀속할 수 없다.
- API 자동 인증: 연동한 AP 계정을 선택하고 서버 도메인을 입력한다. Mastodon 계열은 공개 연락 계정, Misskey 계열은 로컬 `isAdmin` 계정을 확인한다. 기존 API 소유권은 교체할 수 있지만 DNS 소유권은 교체하지 못한다. 회원 인증 방식이 REST 로그인으로 바뀐 것은 아니다.
- 수동 갱신: 실제 소유자만 서버마다 1시간에 한 번, 기존 PG 큐에 요청한다. 화면에서 대기/진행/완료/실패를 조회할 수 있다. 종료된 서버도 명시적 1회 갱신은 가능하지만 종료·숨김·소개문을 바꾸거나 정기 수집을 재개하지 않는다.
- 이름·소개·규칙·언어·태그·초대/승인·운영자 코멘트·목록 숨김을 관리자 승인 없이 수정. 이름/소개가 비면 수집값을 표시한다. `is_force_hidden`, 종료, 관측값, 소유자 UUID는 수정 DTO에 없다.
- 낙관적 revision 검사와 DB 잠금으로 오래된 편집 덮어쓰기를 거절. 변경 이전 내용을 비공개 이력으로 보관한다. 이력 공개·되돌리기 UI는 아직 없다.
- 관리 권한 내려놓기: 도메인을 다시 입력. 서버/안내는 남기고 소유권만 해제. 탈퇴 시에도 소유권을 떼며 이력은 남는다. 이력의 이전 스냅샷에는 당시 회원 UUID가 남을 수 있으므로 완전한 데이터 소거로 안내하지 않는다.
- 공개 상세에 규칙·언어·태그·운영자 코멘트와 초대 여부를 반영한다. 원격/입력 문자열을 HTML로 해석하지 않는다. 공개 조회는 소유권·인증값·편집 이력을 직렬화하지 않는다.

회원 최초 로그인은 계속 **공개 AP 인증글**이다. 이 DNS 인증은 사이트 관리 권한에만 사용하며 회원 인증을 대체하지 않는다. 소유 인증/편집/사임은 실제 인증 15분 이내 세션이 필요하고 DB 트랜잭션에서 다시 확인한다. 도메인 제어권 획득에 신규 회원 가입 제한을 추가하지 않았다.

## 데이터와 런타임

- 내장 migration `202609130006_site_ownership`. `directory_site_details`는 수집값 및 `legacy_sites` 원본 보관과 별도다. 크롤러는 이 테이블을 덮어쓰지 않는다.
- importer가 옛 owner/method/rules/language/tags/invite/approval/comment를 별도 projection으로 정확히 옮기고 양방향 대조한다. 원본 필드 보관·quarantine 차단은 유지. 기존에 적용한 rehearsal의 누락 projection을 자동 덮어쓰지 않는다.
- `directory_owner_challenges`는 만료 작업에서 정리, 세션 삭제 시 cascade. DNS 조회 중에는 DB 트랜잭션을 열어두지 않고 완료 시 다시 권한 검사한다.
- HTTP는 기존 회원 Origin/cookie/no-store 경계를 공유한다. `/account/*`도 private/no-store. 한글 규칙·코멘트를 담는 `/api/member/sites/edit`만 32 KiB body 제한, 다른 회원 API는 8 KiB 유지.
- DNS는 시스템 resolver 설정을 사용하는 Hickory 0.26.3. FQDN 조회, 3초 timeout/1 attempt 및 5초 전체 제한, 기존 인증 네트워크 동시성 4개 제한. TXT 문자열은 같은 RR 안에서만 연결한다. 별도 서비스·컨테이너·공용 DoH 의존성 없음. Windows/Linux의 실제 배포 DNS 설정은 별도 확인 필요.
- 새 lock 항목 10개 라이선스 확인: Hickory 3개, critical-section, ipconfig, prefix-trie, resolv-conf, tagptr, widestring은 MIT/Apache-2.0 선택형. moka는 `(MIT OR Apache-2.0) AND Apache-2.0`. GPL/AGPL 소스 사용 없음.

## API 인증의 바인딩과 호환 경계

- 선택 계정은 로그인한 회원의 `activitypub_post` 연동 행에서만 읽는다. 도메인은 그 계정의 WebFinger 주소 도메인 또는 AP actor 호스트여야 한다. 다른 사이트의 동명 username으로는 인증할 수 없다.
- 저장된 핸들을 기존 AP 클라이언트로 다시 조회해 actor ID가 같은지 확인한 다음, 해당 서버의 고정 API 경로만 조회한다. `username`, 로컬 `acct`/`host`, 제공되는 `uri`도 비교한다. 오래된 Mastodon API의 `uri` 누락은 허용하되 현재 AP 해석과 로컬 계정 비교는 생략하지 않는다.
- v2가 404/405/501일 때만 v1으로 내려간다. Phoenix 원본의 모든 오류 fallback보다 엄격하다. v2에 다른 연락 계정이 있거나 접근 거절·장애·리다이렉트가 있으면 구 v1의 오래된 계정으로 권한을 얻지 못한다. 이런 경우 재시도 또는 DNS 인증이 필요하다.
- 소프트웨어 계열은 기존 MIT reference의 고정 호환 목록으로 판단한다. 회원 편집 카탈로그로 인증 방법을 임의 지정하지 않는다. 해당 API를 실제로 제공하지 않는 포크/버전은 실패로 안내한다.
- 외부 조회 중 DB 잠금을 유지하지 않는다. 완료 시 회원/세션/연동 ID·actor·핸들·검증시각/서버·revision·계열을 다시 검사한다. 연동 해제·재인증·ban·로그아웃·DNS 귀속·편집과 경합하면 이전 결과를 재사용하지 못한다.
- 기존 SSRF 방어를 공유한다: 공개 HTTPS 443, 검증한 IP에 연결 고정, mixed-private DNS 거절, 환경 proxy/redirect 금지. API 요청 10초, 전체 확인 40초, JSON 256 KiB. 개인/전체 시도 제한과 4개 네트워크 동시성 제한. 공급자 암호·토큰을 받지 않는다. Misskey의 POST는 `users/show` 조회뿐이다.
- 내장 migration 007은 `api_claim` 이력 동작만 추가한다. 이력이 생긴 뒤 downgrade가 필요하면 실패시키며 기록을 삭제하지 않는다.

## 수동 갱신의 정확한 의미

- 내장 migration 008의 `owner_requested` 작업도 같은 `directory_jobs`에서 임대/재선점/최대 3회/토큰 fencing을 따른다. 파비콘도 재수집하며 성공한 새 이미지가 있을 때만 교체한다.
- `refresh_requested_at`과 작업 연결은 안내 revision과 별도다. 30분 자동 수집의 `checked_at`을 1시간 제한에 쓰면 수동 갱신이 계속 막히므로, **수동 요청 사이의 1시간**으로 제한한다. 중복 요청은 DB 잠금/트랜잭션으로 한 번만 큐에 넣는다.
- POST 성공은 접수일 뿐 성공 수집이 아니다. `진행 상태 확인`은 해당 소유자에게만 실제 작업 상태를 읽어준다. 원격 실패 시 이전 NodeInfo/아이콘은 유지하고 실패 상태를 보여준다. 상태 조회는 편집 중인 입력을 초기화하지 않는다.
- 종료 서버의 명시적 갱신 외에는 기존 자동 수집의 종료 검사와 편집 보호를 유지한다. 새 컨테이너/브로커/의존성은 없다. importer는 새 런타임 요청 필드가 NULL이고 작업이 비어 있음을 대조한다.

## 아직 동등하지 않은 것

- 신규 서버 등록 및 6개월 계정의 상세 판정. 가입 자체를 6개월로 제한하지 않는다.
- 소프트웨어/공동 콘텐츠의 회원 편집·이력 UI·분쟁 개입.
- 원격 DNS 성공/실제 서비스 API 호환/운영 데이터 import는 자동 가상 테스트 결과로 대체하지 않는다.

## 근거

MIT Phoenix: `lib/fediverse_kr/servers/admin_verification.ex`, `servers.ex`, `server.ex`, `lib/fediverse_kr_web/live/my_live.ex`, `server_list_live.ex`.

DNS API: https://docs.rs/hickory-resolver/0.26.3/hickory_resolver/struct.Resolver.html — 다운로드한 MIT/Apache 라이브러리 타입/기능 및 라이선스도 확인했다.

Mastodon 응답 계약: https://docs.joinmastodon.org/entities/Instance/ 및 https://docs.joinmastodon.org/entities/Account/ (contact.account, local acct, uri). Misskey 계약은 위 MIT Phoenix 구현을 기준으로 하며 실제 제품별 호환 시험은 남아 있다. GPL/AGPL 제품 소스는 사용하지 않았다.
