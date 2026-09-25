# 회원 관리 후속 통합

2026-09-14. 운영 데이터가 아닌 격리 PG/모의 AP/로컬 브라우저를 대상으로 구현했다.

[회원 사진·이모지 갱신](profile-media-refresh.md): AP 로그인 후 자동 수집, 계정별 출처 선택과
본인 수동 가져오기, 실패 시 보존, 연결 해제/탈퇴 정리를 후속 연결했다.

## 실제 연결된 기능

- `/account/comments`: 기존 Phoenix ‘내 댓글’ 목록. 본인 작성 댓글·답글, 페이지 나눔,
  공개 서버 상세 이동과 삭제 제외를 연결했다. [개인 기록 경계](community.md).

- `/account`: 표시 이름 저장, 연동별 프로필 표시 허용/비공개 저장, 연결 해제.
- 마지막 로그인 방법은 제거할 수 없다. 다른 연합 계정 또는 자체 ID/Argon2id가 남아 있어야 한다. 연결 해제 시 모든 세션과 해당 계정의 미완료 인증을 폐기한다.
- 현재 암호로 변경하는 기존 경로에 더해, **이미 연결된 계정의 새 공개 인증글**로 다시 인증한 뒤 15분 안에 암호를 재설정할 수 있다. 임의 이메일 복구나 비인증 ID 가입은 없다.
- 새 계정 추가, 자체 암호 최초 등록, 연결 해제, 탈퇴는 15분 이내의 실제 인증이 필요하다. 기존 계정 재인증은 오래된 세션에서도 가능하다.
- DB에 `authenticated_at`, `federated_authenticated_at`을 추가했다. 단순 세션 회전/새 계정 연결은 인증 시각을 새로 만들지 않는다. 구 세션의 연합 재인증 근거는 NULL이다.
- 민감한 쓰기는 회원→세션 순서로 잠근 뒤 현재 권한을 다시 확인한다. 연결 해제와 동시에 진행 중인 로그인은 오래된 소유 관계를 다시 붙일 수 없다.
- 회원 UUID는 변경하지 않으므로 공통 회원 guard는 `FOR NO KEY UPDATE`를 사용한다.
  인증 확인 중 다른 회원 변경/삭제는 계속 직렬화하되, 관리자 변경의 지연된 owner 외래키
  확인(`KEY SHARE`)까지 막아서 회원→서버와 서버→회원 참조가 교착되지 않게 한다.
  [PostgreSQL 행 잠금의 충돌 관계](https://www.postgresql.org/docs/14/explicit-locking.html#LOCKING-ROWS).
- 본인 요청 탈퇴: 댓글 삭제 표시와 작성자 연결 제거, 신고자/처리자 연결 제거, 서버 운영자/인증 방법 해제, 회원/연동/세션/인증 요청 삭제. ‘탈퇴’ 입력 확인이 필요하다.

서버 함수는 객체 소유자를 세션에서 가져온다. 클라이언트 회원 UUID를 신뢰하지 않는다. Origin/본문 크기 제한/no-store를 공유한다. 로그아웃·연결 해제·탈퇴 응답은 세션과 브라우저 바인딩 두 쿠키를 각각 만료시킨다. Dioxus의 header insert가 두 번째 Set-Cookie로 첫 번째를 덮는 문제를 HTTP guard의 append로 피한다.

## 원본 동작과 남은 경계

탈퇴 근거는 MIT reference `lib/fediverse_kr/accounts/accounts.ex`의 `withdraw_user/1` 및 comments/reports/servers 외래키 migration이다. 댓글 본문과 신고 기록을 모두 지우는 정책으로 바꾸지 않았다. 타인의 답글을 지우지 않으며 스레드 관계는 보존한다. UI도 완전한 데이터 소거라고 표현하지 않는다.

- [탈퇴 후 파일 정리](media-cleanup.md)를 연결했다. 참조 제거와 PG 삭제 예약은 탈퇴와 같은 트랜잭션이고, 같은 프로세스의 OpenDAL 워커가 공유하지 않는 바이트만 지운다. 재수집한 새 프로필 이미지도 포함한다. 실제 원본 export·백업 삭제 주기는 별도다.
- 2026-09-14 [구 회원 첫 로그인](legacy-member-login.md)을 연결했다. 새 공개 인증글 검증 뒤 옛 UUID를 재사용하고 최초 actor를 고정한다. 해제한 옛 계정의 익명 복구는 거절한다. [검토 사본 활성화](activation.md) 도구는 추가했지만 실제 운영 데이터에는 실행하지 않았으며 importer quarantine은 보존한다.
- 연동의 표시 설정은 저장되지만 공개 프로필과 사람 목록은 아직 시안이다. 연결 하나를 공개로 설정했다고 회원 전체가 자동 공개되거나 목록에 올라가지 않는다.
- 모든 로그인 수단을 잃은 경우 자동 복구는 제공하지 않는다. 다른 회원과의 계정 병합도 미구현.
- 원격 서버에서 내 인증글을 자동 삭제하지 않는다. fediverse.kr 탈퇴와 원래 SNS 탈퇴는 별개다.
- 향후 오너/댓글 쓰기도 같은 회원 lock 순서를 사용해야 탈퇴와 경합할 때 일관성을 보장할 수 있다.

## 재현 검증

서버 전체 테스트: `FEDKR_TEST_DATABASE_URL`을 격리 `fedkr_test`로 설정한 뒤 `cargo test --locked --offline --no-default-features --features server --bin fediversekr2 -- --include-ignored --test-threads=1`.

추가 PG 테스트는 실제 인증 verifier에 가상 AP 문서를 주입한다. 공개/비공개 보존, 타인 연동 변경 거절, 마지막 로그인 보호, 동시 해제, 오래된 세션으로 새 계정 추가 거절, 새 연결과 재인증 구분, 암호 복구/세션 폐기, 탈퇴 후 댓글·신고·운영자 관계를 확인한다.

실제 화면/HTTP 검증은 `scripts/member-browser-fixture.ps1` + `scripts/check-membership-browser.js`. 전자는 고정 loopback/개발 DB/클러스터 경로를 검사한 뒤 무작위 가상 회원만 만들고 지운다. 실제 AP 상호운용 증거는 아니며 운영 압축파일도 읽지 않는다. 실행 결과는 `docs/cutover-readiness.md`에 기록한다.
