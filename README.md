# fediverse.kr — Dioxus 리빌드

fediverse.kr는 현재 https://fediverse.kr 에 서비스를 제공하고 있습니다. Phoenix 교체 준비의 역사적 기록과 남은 검토 항목은 [현재 완료·미완료와 검토 순서](docs/cutover-readiness.md)에, 제품 방향은 [PRODUCT.md](PRODUCT.md)에 있습니다. 그 준비 기록은 현재 서비스가 비공개이거나 미전환이라는 뜻이 아닙니다.

GitHub Issues와 Pull Requests를 환영합니다. 기여 방법·검토 흐름·라이선스는 [CONTRIBUTING.md](CONTRIBUTING.md)와 [GitHub 기여 흐름](docs/github-contributions.md)을 참고하세요.

**빌드·배포 원칙:** 유지보수자가 로컬 환경에서 직접 검증·빌드하고 운영에 배포합니다. GitHub는 소스 공개와 Issues/PR 기여 창구이며, GitHub CI/Actions는 구성하거나 실행하지 않습니다.

## 개발과 검증

Dioxus crate는 `0.7.9`에 고정되어 있으며 UI 실행·번들 빌드에는 같은 버전의 `dx`를 사용합니다. Cargo 점검에는 `dx`가 필요하지 않습니다. [Linux 도구 준비와 격리 DB 검사](docs/dependency-policy.md)를 먼저 따라 주세요. ARM64 Linux에서는 `FEDKR_TEST_TARGET=aarch64-unknown-linux-gnu`를 설정합니다. 로컬 점검 진입점은 다음입니다.

```sh
python3 scripts/test-contribution-checks.py
python3 scripts/check-contribution.py quick
```

현재 시스템의 `dx`가 `0.7.9`가 아니라면 그 결과를 프로젝트 검증으로 사용하지 마세요. 예를 들어 다음으로 CLI를 맞출 수 있습니다.

```sh
cargo install dioxus-cli --version 0.7.9 --locked
```

`quick`은 WASM·네이티브 Linux 서버 빌드 점검, 서버 단위 테스트, 세 지원 그래프의 `cargo deny`를 실행합니다. 현재 보안 권고 2건 때문에 의존성 게이트는 실패하며, 이를 억제하지 않습니다. 범위와 해소 조건은 [의존성 정책](docs/dependency-policy.md)에 있습니다. 기여자와 유지보수자는 이 검사를 로컬에서 실행하고 실제 결과를 공유합니다.

### 실행

화면 검토만 할 때(DB 설정 없이, 가상 서버 예시):

```powershell
dx serve --web --fullstack true --bin fediversekr2 --addr 127.0.0.1 --port 12239 --open false --interactive false --locked
```

개발 DB 및 회원·워커 통합:

```powershell
./scripts/dev.ps1 -Mode Setup
./scripts/dev.ps1 -Mode Start
```

개발 DB는 .local/postgres, 127.0.0.1:16439의 fedkr_dev/fedkr_test입니다. trust 인증은 개발 전용입니다. 운영 스냅샷을 이 클러스터에 넣지 마세요. .env 파일은 자동 로드하지 않습니다.

## 화면

| 경로 | 내용 |
| --- | --- |
| / | 승인된 랜딩, 세 장면의 공유 SNS 데모, 느린 표어 순환, 서버·계정 통계 |
| /start | 단일 SNS의 바깥이 드러나는 스크롤 이야기 |
| /start/delivery, /start/visibility, /start/boundaries | 전달·공개범위·거리두기 |
| /start/find, /start/moving | 주소로 팔로우하기·이사하기 (검토 초안) |
| /platforms | 종류 → 소프트웨어 목록, 전체 DB 이름 검색·페이지 이동 ([조회 계약](docs/porting/catalog-browsing.md)) |
| /software/:name | 소프트웨어 설명 → 해당 서버 목록 |
| /software/:name/servers | 소프트웨어별 서버 검색 |
| /servers, /servers/:domain | 공개 서버 검색·페이지 나눔·수집 상태 상세 |
| /people | 비공개 연동 / 공개 프로필 / 목록 노출을 구분하는 검토안 |
| /community | 한국어 연합우주 커뮤니티와 독립 프로젝트 안내 |
| /operate, /guides/self-hosting | 운영 시작·관리형 서비스 확인 사항·공동 편집 방향 |
| /develop | 클라이언트·연합 소프트웨어·호환성 기록의 진입점 |
| /apps, /guides/migration, /about | 앱 이용·이사·사이트 소개 |
| /login, /account | 공개 AP 인증글 로그인·연동·선택형 Argon2id 자체 로그인 |
| /account/moderation, /account/moderation/:id | 관리자 전용 신고 검토·댓글/회원 조치·비공개 이력 |
| /account/moderation/sites, /account/moderation/sites/:id | 관리자 서버 조회·숨김/종료/초대제/태그·수동 수집·조치 이력 |

DB 미설정일 때만 명시된 예시를 사용합니다. **DB 연결 실패를 가상 데이터로 숨기지 않습니다.**
DB 연결 시 목록/통계는 공개용 필드만 조회합니다. 숨김/강제 숨김은 직접 URL과 통계에서도 제외합니다. 통계는 운영 종료를 제외하며 모르는 계정 수는 0이 아닙니다.

## 구조

- src/landing.rs: 승인된 랜딩 재생 상태
- src/social_demo.rs: 랜딩·설명의 공통 가상 앱 컴포넌트
- src/explainer/: 스크롤 설명과 장면별 모델
- src/directory/: 공개 DTO, SSR 화면, GET API, 명시적 미리보기
- src/information.rs: 운영·개발·사람 찾기 등 콘텐츠 검토안
- src/membership/: 세션/API/계정 화면
- src/moderation/: 관리자 전용 DTO/API/신고·서버·소프트웨어/분류 관리 ([권한 부여](docs/porting/moderation.md), [카탈로그 관리](docs/porting/catalog-moderation.md))
- src/backend/db/: Diesel/diesel-async 저장소 경계
- src/backend/federation/: WebFinger·AP 읽기·서명·안전한 전송 계층
- src/backend/runtime.rs: HTTP와 같은 프로세스의 PG 영속 작업 큐 실행
- src/backend/storage.rs: OpenDAL 파일 조회·정리 ([공유 파일과 탈퇴 수명주기](docs/porting/media-cleanup.md))
- migrations/diesel/: 바이너리에 포함되는 마이그레이션

### 직접 검증

아래 `--offline` 명령은 최초 `cargo fetch --locked`로 의존성을 받은 뒤 사용합니다. WASM 타깃과 네이티브 도구 설치는 위의 Linux 준비 문서를 따릅니다.

```powershell
./scripts/dev.ps1 -Mode Test
cargo check --locked --offline --release --no-default-features --features server --bin fediversekr2
cargo check --locked --offline --target wasm32-unknown-unknown --no-default-features --features web --bin fediversekr2
```

/healthz는 프로세스 응답, /readyz는 설정된 DB 조회 가능 여부입니다. **readiness 200은 기능 동등성이나 이관 완료의 증거가 아닙니다.** 미설정 UI 미리보기는 /readyz 503을 반환합니다.

배포물은 일반 Cargo 바이너리만이 아니라 **같은 Dioxus 빌드의 서버와 `public/` 묶음**입니다.
Windows 개발 환경에서는 다음으로 빌드하고 합성 자료만의 실제 프로세스 검사를 재현합니다.

```powershell
dx build --web --fullstack true --release --bin fediversekr2 --locked --offline --debug-symbols false --windows-subsystem CONSOLE
./scripts/check-activation-lifecycle.ps1
```

실행 조건과 검증하지 않는 항목은 [배포 수명주기 검증](docs/porting/release-lifecycle.md)을 따릅니다.

### 라이선스

이 저장소의 프로젝트 소스는 [MIT](LICENSE-MIT) 또는 [Apache-2.0](LICENSE-APACHE) 중 선택하여 사용할 수 있습니다. 별도 표시가 없는 기여는 같은 `MIT OR Apache-2.0` 조건으로 받습니다. 의존성·폰트·아이콘 등 제3자 자료는 각자의 저작권과 라이선스가 적용되며, 해당 고지를 유지합니다.

## 운영 데이터

[오프라인 이관 도구](docs/porting/legacy-import.md)는 별도 격리 PostgreSQL에서만 동작합니다. [실자료 리허설 기록](docs/porting/real-data-rehearsal.md)과 과거 전환 준비 문서는 당시의 검증 범위와 한계를 설명하는 역사적 기록입니다. 그 문서의 미완료 항목을 현재 서비스의 미전환 상태로 해석하지 않습니다. 실제 스냅샷이나 자격증명을 저장소·브라우저·외부 서비스에 올리지 않습니다.

[기존 파일 보존 묶음](docs/porting/legacy-assets.md)은 같은 바이너리의 `--legacy-assets` 명령입니다. 별도 저장소 export의 파일 바이트를 대조·보존하며, DB의 파일 키만 옮겨 놓고 이미지 이전이 끝났다고 간주하지 않습니다. [런타임 이미지 조회](docs/porting/media-serving.md)는 같은 OpenDAL Fs reader로 로고·서버 아이콘·본인 아바타/이모지를 제공합니다. [라이브러리 선택과 도메인 정책 경계](docs/porting/library-boundaries.md)를 기록했습니다.

[검토된 이관 사본 활성화](docs/porting/activation.md)는 `--legacy-activate`로 검사하고 `--apply`로 명시적으로 승인합니다. DB 이름/origin과 파일 대조를 확인하며, 원본·트래픽·실행 중인 서비스를 자동 변경하지 않습니다. 일반 기여 검증은 이 명령이나 운영 자료를 필요로 하지 않습니다.

과거 Phoenix 레퍼런스는 별도 읽기 전용 자료이며 일반 빌드·기여의 필수 입력이 아닙니다. 기존 [MIT 고지](docs/licenses/phoenix-reference.MIT.txt)를 보존합니다. GPL/AGPL 소스, SQLx, bcrypt 및 별도 worker/migration 컨테이너는 추가하지 않습니다.


