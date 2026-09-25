# 배포 묶음과 실제 프로세스 검증

서버 실행 파일만 빌드하는 것과 웹 배포물을 만드는 것은 다르다.
이 프로젝트는 Dioxus가 처리한 서버와 **같은 릴리스 입력으로 만든 `public/` 전체**를 함께 배포한다.
Rust 바이너리만 복사하거나 개발 서버의 웹 파일을 섞지 않는다.
HTTP와 워커는 이 서버 프로세스 하나에서 동작한다.

## Windows 개발 환경에서 재현

프로젝트 루트에서 Dioxus CLI **0.7.9**와 잠긴 Cargo 의존성을 사용한다.
`dioxus-cli-config` 라이브러리 버전 0.7.10과 실행 도구의 버전은 별개다.

```powershell
dx build --web --fullstack true --release --bin fediversekr2 --locked --offline --debug-symbols false --windows-subsystem CONSOLE
./scripts/check-activation-lifecycle.ps1
```

`--offline`은 이미 내려받은 의존성이 있는 환경용이다. 라이선스 검토 없이 lock을
갱신하거나 다른 버전으로 대체하는 해결책은 사용하지 않는다.
검사 기본 실행 파일은 `target/dx/fediversekr2/release/web/server.exe`이며,
명시적인 `-Binary`를 주어도 그 옆의 `public/index.html`과 WASM 파일이 필요하다.

이 검사는 PowerShell 7, 로컬 PostgreSQL 17 클라이언트, 기존 **합성 자료 전용**
workspace PG 클러스터(127.0.0.1:16439 / fedkr_test)를 요구한다.
실제 자료나 임의 DB URL을 입력받는 이관 도구가 아니다.

## 확인하는 연결

1. 독립적인 합성 Phoenix 스냅샷과 네 파일 참조를 생성한다. 실제 운영 백업은 읽지 않는다.
2. 실제 실행 파일의 import → OpenDAL 파일 보존 → 명시적 승인 명령을 차례로 실행한다.
3. 승인 전, origin 불일치, 저장소 누락·변조 상태에서 서버 시작이 거절되는지 확인한다.
4. 승인 후 HTTP/SSR, 번들의 정적 파일 바이트, 서명 키 보존, 숨김 서버, 회원 UUID와
   본인 전용 이미지, 세션 폐기 후 거절을 확인한다.
5. 같은 프로세스의 예약·수집·정리 작업과 강제 종료 후 만료된 PG 임대 복구를 확인한다.
   실제 HTTP 탈퇴 후 사용하지 않는 아바타 파일 삭제, 다른 용도로 공유하는 이미지 보존,
   삭제된 파일 없이 재시작, 원본 export/이관 inventory 보존도 확인한다.
6. 실제 워커의 기록이 생긴 뒤 승인 명령을 반복해도 snapshot으로 덮어쓰지 않는지 확인한다.

회원 HTTP 검사에는 합성 UUID에 대한 **테스트용 DB 세션**을 사용한다. 공개 인증글 검증을
통과했다는 증거는 아니다. 워커에는 DNS/HTTP 전에 거절되는 주소만 넣고 기존 합성 서버는
종료 상태로 둔다. 외부 사이트를 수집하거나 실제 가입·팔로우·게시를 하지 않는다.

실패해도 이 실행에서 만든 정확한 DB 이름/OID와 임시 경로만 확인해 정리하고,
직접 띄운 테스트 프로세스만 종료한다. 원문 SQL/개인 키/세션 토큰은 출력하지 않는다.
SQL은 UTF-8 표준입력으로 전달해 Windows 명령행의 한글 손실과 비밀 인수 노출을 피한다.

## Linux 서버와 자산 처리

WASM/JS/CSS는 OS와 무관하지만, **Linux 서버도 `dx tools assets` 처리를 받아야 한다.**
`cargo build` 결과에 `public/`만 복사하면 HTTP 200이어도 SSR의 자산 주소가 placeholder로 남는다.
`check-linux-runtime.py`는 실제 Linux SSR에서 참조한 자산과 파일 응답을 검사해 이를 잡는다.
주소를 직접 패치하는 구현을 만들지 않고 Dioxus의 공식 자산 처리 명령을 사용한다.

일반적인 Linux Dioxus fullstack build를 쓸 수 있다면 그 묶음을 검증하면 된다. 서버/웹을
별도 빌드 환경에서 만들 때는 동일한 source/lock/assets로 릴리스를 만들고, 새 출력 디렉터리에
Linux ELF 서버와 웹 release의 `public/`를 배치한 뒤 다음 처리를 실행한다.

```sh
dx tools assets /absolute/release/server /absolute/release/public/assets
python3 scripts/check-linux-runtime.py \
  --binary /absolute/release/server \
  --public-dir /absolute/release/public \
  --unit-tests
```

검사는 Linux의 일반 사용자와 설치된 PG14 도구를 사용해 사용자 캐시에 매번 새로운 0700
합성 클러스터를 만든다. 기존 16439 listener가 있으면 거절한다. 별도 `fedkr_dev`/`fedkr_test`를
생성해 프로세스 검사와 Rust/PG 전수를 분리하고, 종료 후 자신이 만든 클러스터·파일만 정리한다.
현재 호스트의 Windows PG17에 접근하거나 포트를 외부에 노출하지 않는다.
`--rust-filter`는 실패 원인 좁히기용이며 이 옵션을 쓴 결과를 전체 테스트 통과로 세지 않는다.

현재 WSL(Ubuntu 22.04, glibc 2.35)에서는 공식 CLI 0.7.9 Linux 바이너리가 요구하는
`GLIBC_2.39`가 없어 바로 실행되지 않았다. 빌드 도구 캐시에만 Ubuntu 서명 저장소의
`libc6 2.39-0ubuntu8` **바이너리**를 풀고 그 loader로 CLI를 실행해 자산을 처리했다.
OS 패키지를 설치/업그레이드하지 않았고, 이 도구용 파일은 앱 배포물·Rust 의존성·저장소에
포함하지 않는다. 일반 Linux 배포 환경에서도 이 개인 캐시 경로가 필요하다는 뜻은 아니다.

- CLI release archive SHA-256: `3b132551b480bc96f938f9f0d37936ee1190f994977539dcc347eaf38540d005`.
- 도구용 libc6 deb SHA-256: `af36c7ac770770fe3d3c10e85d6bc538e76e57570ba7db7d397fb9f654783ef3`.
- 출처: [Dioxus 0.7.9](https://github.com/DioxusLabs/dioxus/releases/tag/v0.7.9),
  Ubuntu archive의 서명된 noble/main 패키지 색인. libc **소스**를 받아 빌드하거나 앱에 vendoring하지 않았다.

## 시작·종료 계약

- release는 `public/index.html`, Dioxus router 구성, 포트 bind를 확인한 뒤 DB 초기화·워커를 시작한다.
  불완전한 배포/점유된 포트 때문에 실패한 프로세스가 먼저 migration/수집을 실행하지 않는다.
- SIGTERM/종료 신호에서 HTTP 신규 연결 수락을 즉시 중단하고 기존 요청과 워커를 **동시에** 정리한다.
  워커의 최대 25초 정리가 끝날 때까지 새 HTTP 요청을 받는 순서가 아니다.
- 기존 HTTP 요청은 최대 30초, 워커는 기존 25초 안에서 정리한다. HTTP 종료 기한 초과는
  조용한 성공으로 숨기지 않고 고정 오류 메시지와 실패 종료로 남긴다. 남은 PG 임대는 다음 시작에서 회수한다.
  배포 관리자의 강제 종료 기한에는 이 정리 시간을 포함할 여유가 필요하다(예: 35초 이상).
- Linux 검사는 실제 SIGTERM, 본문을 덜 보낸 요청의 408 정리, 극도로 느린 읽기 연결의
  종료 기한도 다룬다. Windows Ctrl+C와 운영 프록시 동작까지 검증했다는 뜻은 아니다.

## 증거의 한계

동일한 Linux 묶음을 배포 컨테이너에 담는 설정과 `--container-image` 검사는
[단일 프로세스 OCI 배포](../../deploy/container/README.md)를 따른다. 해당 옵션은 실제 이미지의
server/index 해시, 웹 자산, 비root/read-only 실행과 외부 미디어의 저장·재시작·정리를 추가 검사한다.
Linux 원시 바이너리 검증을 이미지 실행 성공으로 대신하지 않는다.

HTTP 정적 파일 응답은 실제 브라우저 hydration·시각 검토와 다르다.
Windows 검사만으로 Linux 실행이나 SIGTERM을 증명하지 않으며, 위 Linux 검사 결과를 따로 기록한다.
두 검사만으로 실제 원격 AP 호환, TLS/프록시, 운영 규모의 자료,
운영 스냅샷 최신성·쓰기 정지·롤백을 검증했다고 주장하지 않는다.

시점별 실행 결과는 [통합 검증 기록](integration-checks.md), 실제 이관 승인 절차는
[activation.md](activation.md)를 따른다. 프로덕션 전환은 별도 작업이다.
