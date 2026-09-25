# 회원 core — 선택형 자체 자격증명과 세션

## 제품 흐름

첫 로그인은 **연합계정의 공개 인증글을 확인**하여 회원을 만들거나 기존 회원을 찾는다. 자체 아이디·비밀번호는 로그인한 뒤 필요할 때 추가한다. 일반적인 아이디·비밀번호만의 공개 회원가입 엔드포인트는 만들지 않는다.

회원의 기본키는 로컬 UUID다. 연합계정 하나가 회원 하나의 기본키가 아니며, 한 회원에게 여러 계정이 귀속될 수 있다. 서버 추천이나 연합계정 생성 6개월 조건은 이 core의 가입 조건에 넣지 않았다.

## 경계

- `backend::auth`: 선택형 자체 자격증명, 비밀번호 로그인/변경, DB 세션.
- 공개 인증글 검증 모듈: 외부 응답 검증과 검증된 신원 객체. `verified: true` 같은 클라이언트 주장은 인증 증거가 아니다.
- 인증글 소비 트랜잭션: 최초 회원 생성, 기존 외부 계정으로 로그인, 추가 계정 귀속 충돌 처리, 단회 challenge 소비. 회원 core에는 이 검증을 건너뛰는 생성 API가 없다.
- HTTP: `HttpOnly`, `Secure`, `SameSite` 쿠키, 동일 Origin/CSRF 검증, 본문 크기 제한, IP 및 전체 요청 제한. 세션 원문을 Dioxus props나 JSON 응답에 실으면 안 된다.
- 저장소: `backend/db`의 Diesel/diesel-async. 도메인 모델에는 ORM derive/SQL이 없다.
- 실행: 같은 바이너리 시작 시 내장 마이그레이션과 서명키를 준비한다. 같은 프로세스 워커가 만료 데이터를 정리한다. [실행 구조](diesel-and-runtime.md) 참조.

## 공개 함수

| 함수 | 동작 |
| --- | --- |
| `set_local_credentials(db, session_token, login_id, password)` | 로그인한 회원에게 최초 자체 자격증명 추가, 기존 세션 폐기 후 새 세션 발급 |
| `login(db, login_id, password)` | 선택형 자체 자격증명으로 로그인 |
| `get_session(db, token)` | 만료·차단을 반영한 회원 조회 |
| `get_session_details(db, token)` | 위 조회에 세션 UUID·만료 시각 추가; 인증 challenge 바인딩용 |
| `revoke_session(db, token)` | 현재 세션 삭제; 반복 호출 가능 |
| `change_password(db, token, current, new)` | 현재 비밀번호 검증, 변경, 모든 기존 세션 폐기, 새 세션 발급 |
| `cleanup_expired(db)` | 만료 세션·challenge·시도 제한 데이터를 각 최대 1,000행 정리 |

세션 생성과 저장은 `backend/db/members.rs`의 `persist_session` 내부 전용이다. 공개 인증글 검증 이후의 회원 생성/귀속/로그인 트랜잭션에서 호출하도록 열어 두었다. 외부에서 임의 회원 UUID만 넣어 세션을 만드는 API로 노출하면 안 된다.

`AuthenticatedMember.login_id`는 `Option<String>`이다. `SessionGrant`는 직렬화하지 않으며 Debug도 세션 원문을 가린다. DB 오류는 내부 내용 없는 `AuthError`로 변환한다.

선택형 자격증명의 최초 등록은 발급 15분 이내의 세션에서만 허용한다. 오래된 세션은 공개 인증글로 다시 로그인한 뒤 등록한다. 현재 비밀번호를 바꾸는 경우는 기존 비밀번호 검증을 유지한다.

### 공개 인증글 완료 흐름

`backend::flow`의 `begin_challenge` → `get_pending` → ActivityPub 공개 글 검증 → `complete_challenge`가 이어진다. `list_linked_accounts`는 로그인한 회원 자신의 목록만 반환하도록 HTTP에서 회원 UUID를 결정한다.

- 시작 시 확인한 actor와 256비트 난수 코드를 브라우저 nonce/현재 세션에 바인딩한다.
- 코드 형식은 `fedkr-proof-` 뒤 64자리 hex다. 문구 앞뒤 설명은 가능하나 코드 자체는 바꾸지 않는다.
- 네트워크 조회 전에 HTTP가 `challenge_begin` 시도를 소비한다. flow가 다시 차감하지 않는다. 확인 단계에서는 `get_pending`이 `challenge_verify`를 소비한다.
- `VerifiedIdentity`의 actor/handle뿐 아니라 private code hash/발급 시각도 잠근 challenge 행과 정확히 비교한다. 다른 인증 요청의 성공 객체는 재사용할 수 없다.
- 동일 actor의 동시 최초 가입은 트랜잭션 advisory lock과 UNIQUE 제약으로 직렬화한다. 계정 소유권 충돌 시 자동 병합하지 않는다.
- 최초 회원 생성·외부 계정 귀속·challenge 소비·세션 발급을 한 트랜잭션에서 완료한다.
- 추가 연결은 현재 브라우저의 이전 세션만 교체한다. 연결 중인 challenge는 소비 뒤 해당 세션의 삭제와 함께 정리되며 재사용되지 않는다.
- DB 잠금 순서는 actor 직렬화 → 사용자 → 세션 → challenge다. 기존 자격증명 변경의 세션 정리와 교착하지 않도록 challenge를 먼저 잠그지 않는다.

## 저장과 동시성

- `member_users`는 Phoenix `users`와 별개다. `login_id`와 `password_hash`는 함께 없거나 함께 있어야 한다.
- 아이디: ASCII 영문·숫자·밑줄 3~32자. 소문자로 정규화하여 UNIQUE로 보장한다.
- 비밀번호: 15~128 Unicode scalar 값, 최대 512바이트. 공백 제거나 Unicode 정규화를 하지 않는다.
- **Argon2id만 허용**(bcrypt 금지): 19,456 KiB / 2 iterations / 1 lane / 임의 salt. 현재 [OWASP 최소 권장 설정](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html)을 적용했다.
- 해싱은 `spawn_blocking`에서 수행한다. 프로세스당 동시에 최대 2개만 허용하며 포화 시 무제한 대기열 대신 일시 처리 불가를 반환한다. 분산 인스턴스 전체의 제한은 별도로 필요하다.
- 없는 아이디/차단 회원도 비밀번호 검증 비용을 지불하고, 잘못된 자격증명으로 합친다. 입력 바이트 상한을 넘는 값은 비용을 들이기 전에 거절한다.
- 세션: OS 난수 32바이트, 브라우저에는 64자리 hex, DB에는 SHA-256만. 고정 30일 만료이며 [OWASP 세션 지침](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)의 예측 불가능한 서버 측 세션 방식을 사용한다.
- 비밀번호 검증은 DB 트랜잭션 밖에서 한다. 세션 발급 직전 사용자 행을 잠그고 비밀번호 해시와 차단 상태를 다시 검사한다. 동시 비밀번호 변경 후 이전 비밀번호로 새 세션이 살아남는 경쟁을 방지한다.
- 자격증명 추가/변경은 사용자 행 → 현재 세션 순으로 잠그고, 데이터 변경·기존 세션 삭제·새 세션 발급을 한 트랜잭션에서 처리한다.
- 비밀번호 로그인은 아이디별 15분 12회, 변경은 회원별 15분 8회로 제한한다. DB UPSERT가 원자적으로 계산하며 성공했다고 시도 횟수를 지우지 않는다.
- 시도 제한 키는 고정 32바이트 해시다. 요청마다 최대 128개의 만료 키를 지운다. 15분 이내 신규 키 생성량까지 이 제한으로 막는 것은 아니므로 IP/전체 요청 제한을 함께 사용해야 한다.

외부 계정은 기본 비공개이며 actor URL, handle, `(provider_origin, provider_subject_id)` 중복 귀속을 DB에서 금지한다. 계정 이사/alias 변경과 자동 병합은 이번 core 범위가 아니다.

`member_link_challenges`는 공개 인증글 전용이다. `login`은 아직 회원·세션이 없고, `link`는 현재 회원·세션 UUID 모두 필요하다. 복합 FK로 세션이 해당 회원의 것인지도 보장한다. 브라우저 바인딩 비밀과 인증코드는 SHA-256만 저장한다. 인증 완료 요청의 코드를 먼저 해시와 비교한 후 외부 글 검증에 사용한다. actor/handle/profile/outbox는 시작 시 확인한 값을 보관하며, 소비 시 재검증과 `consumed_at IS NULL` 조건을 만족한 행만 원자적으로 완료 처리해야 한다.

## 검증

단위 테스트는 아이디 정규화, 비밀번호 길이·공백 보존, 표시 이름 입력, 세션 난수 형식, salt와 실제 Argon2 검증, 비정상 PHC 파라미터 거절, Debug 비밀값 비노출을 검사한다.

```powershell
cargo test --bin fediversekr2 --no-default-features --features server backend::auth::tests
```

PostgreSQL 통합 테스트는 기본으로 무시한다. 명시적으로 준비한 폐기 가능한 DB의 `FEDKR_TEST_DATABASE_URL`이 있어야 실행하며, 해당 테스트가 fixture 마이그레이션을 적용한다. 인증글 검증을 우회하는 auth core의 테스트 fixture는 해당 테스트 내부에만 있고 앱 함수로 내보내지 않는다. flow 테스트는 FakeTransport의 공개 AP 문서를 실제 검증기로 통과시켜 `VerifiedIdentity`를 얻는다.

```powershell
cargo test --bin fediversekr2 --no-default-features --features server postgres_member_session_lifecycle -- --ignored
```

이 테스트는 선택형 자격증명 등록, 중복 아이디/재설정 거부, 비밀번호 로그인·변경, 전체 세션 폐기, 회원 차단 반영, 로그아웃, 원자적 시도 제한을 검사한다. 테스트가 만든 UUID 행만 정리하며 기존 테이블을 TRUNCATE하거나 기존 사용자 데이터를 삭제하지 않는다. 검증되지 않은 DB에 실행하지 않는다.

초기 core 작성 시 DB가 없었으나, 통합 단계에서 별도 loopback PostgreSQL과 폐기용 테스트 DB가 준비되었다. DB 테스트 결과는 아래 통합 검증 기록과 구분한다.

모듈 통합 전 검증으로, 현재 Cargo 의존성의 feature에 맞춘 `rustc --test` 독립 테스트 바이너리를 빌드·실행했다. **6 passed / 0 failed / 1 ignored**, 실제 Argon2 해싱·검증을 포함해 4.63초였다. 무시된 1건은 위 PostgreSQL 통합 테스트이며, 정상 앱 모듈 전체의 빌드·동작 검증은 별도다.

### 2026-09-12 통합 검증

별도 loopback PostgreSQL의 폐기용 `fedkr_test` DB에서 다음을 실행했다.

```powershell
cargo test --bin fediversekr2 --no-default-features --features server backend:: -- --include-ignored
```

**29 passed / 0 failed / 0 ignored**, 8.46초. 실제 마이그레이션, auth 세션 lifecycle, 공개 인증글 기반 신규 회원 생성/복수 계정 연결/기존 회원 재로그인, 교차 브라우저·코드·세션, 중복 귀속 거절, 만료, 동시 소비 단 한 번 성공을 포함한다.

이어 다른 challenge의 성공 proof 재사용 및 원격 검증 이후 만료된 challenge의 완료 거절을 DB 테스트에 추가했다. `backend::flow::tests -- --include-ignored` 재실행은 **3 passed / 0 failed / 0 ignored**, 0.25초였다.

이 검증은 외부 서버 대신 FakeTransport를 사용했다. 실제 다른 서버에 인증글을 게시하거나 실운영 DB를 변경하지 않았다. 최종 브라우저/HTTP/서버 재시작 검증은 루트 통합 작업의 범위다.

## 라이선스

GPL/AGPL 구현 코드는 사용하지 않았다. 비밀번호·세션 코드는 아래 permissive 라이브러리 API를 사용한 새 구현이다.

- [argon2 0.5.3](https://docs.rs/crate/argon2/0.5.3/source/Cargo.toml): MIT OR Apache-2.0.
- Diesel 2.3.13 / diesel-async 0.9.2 / diesel_migrations 2.3.2: MIT OR Apache-2.0. SQLx는 제거했다.
- 로컬 Cargo manifest 확인: `rand 0.8`, `sha2 0.10`, `uuid 1`, `chrono 0.4`는 MIT OR Apache-2.0, `tokio 1`은 MIT.

이는 이 core의 직접 의존성 확인이다. 전체 프로젝트의 모든 전이 의존성을 감사했다는 뜻은 아니다.
