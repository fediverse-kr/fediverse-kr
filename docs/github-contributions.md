# GitHub 기여 흐름

## 현재 위치

`fediverse.kr`는 현재 https://fediverse.kr 에 서비스를 제공하고 있습니다. 과거 Phoenix 교체 준비와 남은 검토 항목은 [cutover readiness](cutover-readiness.md)에 기록되어 있으며, 그 기록은 현재 사이트가 비공개이거나 미전환이라는 뜻이 아닙니다.

구현된 탐색 경로에는 `/community`의 한국어 연합우주 커뮤니티·프로젝트 안내와 `/people`의 공개 방식 프로토타입이 있습니다. `/people`의 공개 프로필 및 사람 목록은 아직 시안으로 표시되므로, 완성된 사람 찾기 기능처럼 보고하지 마세요.

## 기여 경로

GitHub Issues와 Pull Requests가 기여의 입구입니다. 저장소를 fork한 뒤 작은 작업 브랜치에서 변경하고 PR을 열어 주세요.

- 버그: 재현 단계, 기대 결과, 실제 결과, 환경을 작성합니다.
- 제안: 해결하려는 사용자 문제와 최소한의 제안 범위를 설명합니다.
- PR: 관련 이슈(있다면), 변경 이유, 테스트 명령과 결과를 적습니다. UI 변경에는 가능하면 스크린샷을 포함합니다.
- 금지: 비밀값, 인증 정보, `.env`, 운영 DB/스냅샷, 개인 데이터의 첨부나 게시.

작은 독립 변경을 우선하고, 포맷 변경이나 무관한 리팩터링을 PR에 섞지 않는 것이 검토에 도움이 됩니다.

## 로컬 실행

`Cargo.toml`은 Dioxus crate `0.7.9`를 고정합니다. CLI도 정확히 `0.7.9`를 사용합니다. 설치된 `dx --version`이 다르면 그 결과물은 이 프로젝트의 검증 증거로 사용하지 않습니다.

Linux 사전 점검은 [도구 설치·지원 대상·격리 DB 안내](dependency-policy.md)를 따릅니다. ARM64에서는 `FEDKR_TEST_TARGET=aarch64-unknown-linux-gnu`를 설정합니다.

```sh
python3 scripts/test-contribution-checks.py
python3 scripts/check-contribution.py quick
```

직접 Rust 점검은 최초 `cargo fetch --locked` 및 WASM 타깃 설치 후 다음과 같습니다.

```sh
cargo check --locked --offline --release --no-default-features --features server --bin fediversekr2
cargo check --locked --offline --target wasm32-unknown-unknown --no-default-features --features web --bin fediversekr2
```

화면만 검토할 때는 DB 설정 없이 다음을 실행합니다.

```sh
dx serve --web --fullstack true --bin fediversekr2 --addr 127.0.0.1 --port 12239 --open false --interactive false --locked
```

기존 Windows 개발 경로는 PostgreSQL 17 명령행 도구를 사용합니다.

```powershell
./scripts/dev.ps1 -Mode Setup
./scripts/dev.ps1 -Mode Start
./scripts/dev.ps1 -Mode Test
```

`Setup`과 `Test`는 격리된 로컬 개발 DB를 사용합니다. 운영 DB나 운영 자료를 사용하거나 첨부하지 마세요.

## 반영 흐름

유지보수자가 GitHub 이슈와 PR을 검토하고 로컬 검증 결과를 확인합니다. 수락한 PR은 저작자 정보를 유지한 패치로 정본 Forgejo에 가져옵니다. **유지보수자가 로컬 환경에서 직접 검증·빌드·배포**하며, GitHub CI/Actions는 구성하거나 실행하지 않습니다. 운영 배포가 끝나면 **실제로 배포한 커밋의 소스 트리**를 GitHub `main`에 새 스냅샷 커밋으로 추가합니다. PR에는 반영된 공개 커밋과 검증 결과를 연결하고 처리합니다.

운영 코드·의존성·자산에 영향이 없는 문서나 저장소 설정 정정은 명시적인 승인 아래 재배포 없이 동기화할 수 있습니다. 이 경우 공개 `Source-commit`은 정정 커밋을 가리키며, 실제 운영 이미지의 소스 커밋과 구분해 기록합니다.

공개 Git 이력은 첫 공개 소스부터 시작합니다. 내부의 과거 커밋·메시지·개인 이메일·태그는 옮기지 않습니다. 공개 커밋은 게시 작업을 나타내는 `fediverse.kr source sync` 명의이며, `Source-commit`으로 정본 커밋을 식별합니다. Git 트리 ID가 같으므로 파일 내용과 실행 비트는 정본과 동일합니다. 원래 기여자의 저작자 정보는 정본 이력과 공개 PR에 유지하며, 저작권·제3자 고지도 그대로 보존합니다.

유지보수자의 배포 후 동기화 명령은 다음입니다. `SOURCE_SHA`는 로컬 최신 HEAD가 아니라 배포 검증에서 확인한 전체 커밋 SHA입니다.

```sh
python3 scripts/test-source-sync.py
python3 scripts/sync-public-source.py \
  --revision "$SOURCE_SHA" \
  --remote https://github.com/fediverse-kr/fediverse-kr.git
```

스크립트는 같은 커밋 재실행 시 추가 커밋을 만들지 않으며, 읽기 재확인까지 성공해야 완료로 처리합니다. 미추적 파일·비밀 파일·다른 브랜치·태그를 게시하지 않고 강제 push도 하지 않습니다. 게시 권한은 유지보수자 환경에서만 공급하며 GitHub Actions에는 넣지 않습니다.

GitHub `main`에 별도 변경이 있거나 이전 공개 소스와 정본 계보가 어긋나면 덮어쓰지 않고 중단합니다. 따라서 GitHub에서 PR을 먼저 `main`으로 병합하지 않습니다. 예상하지 않은 직접 변경이 생기면 정본 반영·공개 이력 연결을 명시적으로 정리한 뒤 재개해야 합니다. 소스 push 자체를 배포 성공으로 간주하거나, 검증 전 커밋을 자동 게시하는 Git hook은 설치하지 않습니다.

## 라이선스와 저작권

프로젝트 코드는 [MIT](../LICENSE-MIT) 또는 [Apache-2.0](../LICENSE-APACHE) 중 선택하여 사용할 수 있습니다. PR을 제출하면, 별도 표시가 없는 한 기여분을 같은 `MIT OR Apache-2.0` 조건으로 제공하는 데 동의한 것으로 봅니다. 제3자 의존성 및 자산에는 자체 라이선스·저작권·NOTICE가 적용되며, 이 프로젝트의 소유물로 재표기하지 않습니다.
