# 최종 Linux/OCI 후보 보고 — 2026-09-15

## 후속 이관 검증으로 기존 후보 보류

실제 DB 복원·dry-run/apply/재실행 대조를 완료했지만, 그 과정에서 종단 LF 두 개가 있는
기존 서명키의 PEM 파싱 호환 결함을 발견·수정했다. 아래 Linux/OCI/ZIP은 수정 전 후보의
기록이다. 이번 수정은 Windows native release CLI와 회귀시험으로 검증했으며 WSL 정본에도
동일하게 반영했다. **새 Linux/OCI 후보 재빌드·검증 전 아래 묶음을 그대로 배포하지 않는다.**
현재 실자료 결과와 남은 파일·activation·로그인 경계는
[실자료 리허설](real-data-rehearsal.md)의 현재 결과를 따른다.

## 최종 판정 (아래 초기 실패 기록을 대체)

동일 앱 소스의 Linux 서버/웹 릴리즈 묶음과 OCI 이미지 생성에 성공했다.
기존 Linux runtime/OCI 검사 **219개, 80.11초, 합성 자료 전용**으로 통과했다.
실제 자료 이관·공개 HTTPS/AP·운영 전환 승인은 아직 별도다.

작업 기준은 WSL Ubuntu `<workspace>/fediversekr2`(ext4)다. 앱 입력은 `e0a543f`이며
이후 변경은 문서와 실행 검사뿐이다. Windows 보존 checkout도 clean이며
`src/`, `assets/`, Cargo/Dioxus 설정은 서로 같다. 승인된 랜딩 변경은 없다.

### 조립 경로

1. 기존 Linux Cargo 캐시와 Rust 1.94.1로 native release ELF를 빌드했다.
2. 동일 Windows snapshot에서 Dioxus 0.7.9 fullstack release를 새로 빌드했다.
   Windows 빌드가 원본 앱 입력을 수정하지 않았음을 확인했다.
3. 새 `output/release-candidate-20260915-a`에 ELF와 matching `public/`를 함께 복사하고
   공식 `dx tools assets`를 적용했다. 이 명령은 ELF의 자산 참조를 처리하므로 해시가 바뀐다.
4. 기존 uv로 Python 3.12를 준비하고 cargo-about 0.9.2로 Linux server+web 고지를 생성했다.
   앱 의존성/lock 변경 없이 `prepare-container-context.py`와 고정 base image로 OCI를 만들었다.
5. 같은 bundle/image를 기존 `check-linux-runtime.py --container-image`에 전달했다.
   새 워커 monitor와 DB 중단/복구 검사를 포함한다. Rust 단위 테스트 전수를 다시 실행한 수치가 아니다.

### 고정 산출물

| 항목 | SHA-256 |
| --- | --- |
| Cargo.lock | `768467584b92dcbbaceba2e1126c216034848cd084aa5e05fdc344513dc7a3d0` |
| native ELF (자산 처리 전) | `ce99fba70cb01d3f85652dbcb48ae3046a7887f52c7dc40ff14e7e68951905fb` |
| 배포 server (33,710,488 bytes) | `bc5ae3653c3225a41eb5840dcc513d252eb104522b2f359ba177a18372634e82` |
| public/index.html | `d93b910a4370bfbe1467eb8926c8a587adfe78f5c022de3c3975dd9b1869b010` |
| 고지 HTML | `15b985a57c182bd4dd40a96b4cb8b6c293669286b55b7a57d8991469da0c50fc` |
| 고지 inventory | `1dda2ac534fcc80e207b8e3151fc222f1f30a4bd5d9b6d25fe42f8388b80e37a` |
| local OCI image ID (registry digest와 구분) | `a092c1cdc6407d56118d9f48ce75f2d98c30d1224e1e4ce99e52078134d6e547` |
| 전달용 OCI archive | `25de1a8ccbdb304685ef3d996677616a5f7fd0832447a553a59e3794a30499fa` |

- OCI context: `output/container-context-20260915-b`, linux/amd64, 기본 UID/GID 65532.
- 이미지: `localhost/fediversekr2:reviewed-20260915-b`. 태그 대신 위 ID로 검증했다.
- archive: 로컬 전달 디렉터리의 `fediversekr2-linux-amd64.oci.tar`, 44,477,952 bytes.
- public 원본 57개 + 고지 2개. 앱 normal/build 의존성 그래프는 server 430개/web 241개다.
  선택된 라이선스에 GPL/LGPL/AGPL은 없으며 OS base image의 라이선스 경계는 별도 문서를 따른다.
- 원본 backup/회원 행/키/암호는 입력·이미지·archive에 넣지 않았다. registry push/배포하지 않았다.

### 검증된 범위와 남은 범위

ELF의 실제 SSR/정적 파일, AP actor 공개키 유지, 비공개 API, OpenDAL 바이트/정리,
워커 재시작·만료 작업 회수, SIGTERM/느린 요청 종료와 baked OCI의 파일/미디어 회귀를 확인했다.
추가 monitor 권한/비밀값 비노출 및 DB 중단→복구 후 새 작업 1회 완료는 **원시 ELF 경로**에서
확인했다. OCI에서 동일 fault injection을 반복했다고 주장하지 않는다.

별도 브라우저 검사도 최종 bundle의 **실제 Linux 서버**에서 완료했다. DB 없는 preview에서
PC 1440×1000, 모바일 390×844의 SSR+WASM hydration, `/`→`/start`→`/start/delivery`
클라이언트 이동, delivery 휠 스크롤의 장면 0→1 전환을 확인했다. pageerror 0, 실패 요청 0.
캡처는 보존 Windows checkout의 `output/playwright/final-bundle-hydration-{1440,390}.png`다.
앞서 정적 파일 서버만으로 열었을 때의 atob 오류는 SSR 데이터를 만들지 않는 잘못된 검사 조건이었다.
실제 앱 서버에서는 재현되지 않았으며 이를 앱 결함 수정이나 정상 서비스 장애로 기록하지 않는다.

실자료 restore/import, S3 원본 파일 이전, 실제 인증글 로그인, 공개 HTTPS/AP,
운영 UID 65532 볼륨 권한, 실제 대상 한 주기의 자원 사용·전환·복귀는 남아 있다.
진행은 [배포 인계서](../../deploy/RELEASE_HANDOFF.md)를 따른다.

## 초기 시도 기록 (해결됨)

이번 실행에서는 Linux/OCI 후보를 만들지 못했다. 앱 소스·`Cargo.lock`·의존성 선언은 변경하지
않았고, 기존 후보·캐시도 덮어쓰지 않았다. 개발 서버 세션 `24189`는 정상적으로 Ctrl+C 종료했다.

## 실행한 단일 빌드 시도

WSL 배포판 `Ubuntu`에서 기존 glibc loader wrapper를 사용했다.

```sh
cd <windows-workspace>/fediversekr2
export CARGO_HOME=<cache>/fediversekr2/cargo
export CARGO_TARGET_DIR=<cache>/fediversekr2/target-final-20260915
./.local/run-linux-dx.sh build --web --fullstack true --release --bin fediversekr2 \
  --locked --offline --debug-symbols false --force-sequential true
```

`dx build`가 컴파일 전에 `cargo metadata`에서 멈췄다.

```text
failed to get `dioxus-primitives`
Unable to update https://github.com/DioxusLabs/components#bf007c15
can't checkout from 'https://github.com/DioxusLabs/components':
you are in the offline mode (--offline)
```

지정한 새 target 경로 `<cache>/fediversekr2/target-final-20260915`에는
`server`, WASM, `public/index.html` 산출물이 생성되지 않았다. 기존 캐시의 git checkout에도
해당 components checkout이 확인되지 않았다.

## 따라서 실행하지 못한 단계

후속 후보 bundle 복사, `dx tools assets`, third-party notices 생성/검증, 새 OCI context 생성,
rootless Podman build, `check-linux-runtime.py --container-image`, ELF/public 해시 및 image ID
기록은 모두 실행하지 않았다. 후보가 없으므로 Linux runtime/OCI 통과를 주장할 수 없다.

막힌 원인은 잠긴 git 의존성의 오프라인 checkout 부재다. 의존성·lock을 바꾸거나 네트워크 빌드로
범위를 넓히지 않고 여기서 중단한다. 다음 실행에는 승인된 방식으로 동일 commit의 checkout을
먼저 기존 캐시에 제공한 뒤, 같은 명령을 새 target 경로에서 다시 실행해야 한다.

## 2026-09-15 캐시 보충 후 재시도

기존 `<cache>/fediversekr2/cargo`와 `<cache>/fediversekr2/target`를 보존한 채,
잠긴 revision만 받기 위해 다음을 실행했다.

```sh
cd <windows-workspace>/fediversekr2
export CARGO_HOME=<cache>/fediversekr2/cargo
cargo fetch --locked
```

Cargo는 registry와 `https://github.com/DioxusLabs/components`의 고정 revision을 포함해
748개 crate(100.1 MiB)를 캐시에 보충했다. `Cargo.lock` SHA-256은 실행 전후
`768467584b92dcbbaceba2e1126c216034848cd084aa5e05fdc344513dc7a3d0`로 동일했다.

같은 새 target 경로와 같은 DX 명령으로 재시도했지만, 컴파일 전에 bundled loader가
`rustc -vV`를 SIGSEGV로 종료했다.

```text
error: process didn't exit successfully: .../ld-linux-x86-64.so.2 .../rustc -vV
(signal: 11, SIGSEGV: invalid memory reference)
Cargo build failed - no output location
```

이 재시도에도 Linux ELF/WASM/public 산출물은 생성되지 않았다. native cargo와 DX를 임의로
분리하거나 loader를 바꾸는 추가 빌드는 실행하지 않았다. 현재 차단은 의존성 캐시가 아닌
WSL bundled-loader/toolchain 호환성 문제로, Linux 후보·자산 처리·고지·OCI 검증은 여전히
미실행이다.

## 초기 시도 당시의 검증 경계 (현재 판정은 맨 위 참조)

실제 운영 DB/백업/HTTPS/AP, DB outage 복구, 처리량·RSS·CPU·PG 연결량, 운영 볼륨 UID 65532,
이미지 서명, registry push/deployment는 이번 후보 작업의 검증 대상이 아니며 실행하지 않았다.
