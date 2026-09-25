# 기여하기

GitHub Issues와 Pull Requests를 환영합니다. 버그 보고, 제안, 코드·문서 개선을 이 저장소에서 받습니다.

## 시작하기

1. 기존 이슈를 찾아 중복을 피하거나, 재현 가능한 문제를 새 이슈로 등록합니다.
2. 작은 범위의 브랜치에서 수정하고, 관련 검사를 실행합니다.
3. PR에는 무엇을 왜 바꿨는지, 테스트 결과와 UI 변경의 스크린샷(해당하는 경우)을 적습니다.
4. 비밀값, `.env` 파일, 운영 데이터, 개인 데이터, 운영 스냅샷은 이슈·PR·스크린샷에 첨부하지 않습니다.

새 기여자는 [GitHub 기여 흐름](docs/github-contributions.md)과 [README의 개발·검증 안내](README.md#개발과-검증)를 먼저 읽어 주세요.

## 개발과 검증

Dioxus crate 버전은 `0.7.9`입니다. UI 실행·번들 빌드에는 같은 버전의 `dx`가 필요하지만, Cargo 점검에는 필요하지 않습니다. 먼저 [Linux 준비와 DB 검사](docs/dependency-policy.md)의 공개 도구를 설치합니다. ARM64 Linux에서는 `FEDKR_TEST_TARGET=aarch64-unknown-linux-gnu`를 설정합니다.

```sh
python3 scripts/test-contribution-checks.py
python3 scripts/check-contribution.py quick
```

`quick`은 WASM과 네이티브 Linux 서버를 검사하고 서버 단위 테스트 및 세 지원 그래프의 `cargo deny`를 실행합니다. 현재 남은 보안 권고와 기존 포맷 차이는 [의존성 정책](docs/dependency-policy.md)에 공개되어 있습니다. 실패를 숨기거나 테스트를 생략하지 말고 PR에 실제 결과를 적어 주세요. DB 통합 검사는 같은 스크립트의 `database` 모드이며 운영 데이터는 사용하지 않습니다. 기존 Windows 개발 경로는 PostgreSQL 17 도구와 함께 다음을 사용합니다.

```powershell
./scripts/dev.ps1 -Mode Setup
./scripts/dev.ps1 -Mode Test
```

DB가 필요 없는 화면 검토는 로컬에서 다음처럼 실행합니다.

```sh
cargo install dioxus-cli --version 0.7.9 --locked
dx serve --web --fullstack true --bin fediversekr2 --addr 127.0.0.1 --port 12239 --open false --interactive false --locked
```

## 검토와 반영

유지보수자는 PR의 범위, 로컬 테스트 증거와 사용자 영향을 검토합니다. 승인된 PR은 기여자 저작자 정보를 보존하여 정본인 Forgejo에 반영합니다. 유지보수자가 로컬 환경에서 직접 검증·빌드·배포한 뒤 해당 소스를 GitHub로 동기화합니다. GitHub는 소스 공개와 Issues/PR 기여 창구이며, GitHub CI/Actions는 구성하거나 실행하지 않습니다.

## 라이선스

기여를 제출하면, 별도 표시가 없는 한 기여분을 프로젝트와 같은 `MIT OR Apache-2.0` 조건으로 제공하는 데 동의한 것으로 봅니다. 이는 별도 CLA나 추가 법적 정책을 요구하지 않습니다. 제3자 코드·자산을 포함할 경우 원래의 저작권·NOTICE·라이선스를 보존하고 출처를 설명해 주세요.
