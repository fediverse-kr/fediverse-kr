# 최종 Linux/OCI 후보 전 점검 — 2026-09-14

## 확인한 기존 산출물과 재현 경로

- WSL `<cache>/fediversekr2/`에 `bundle-v13`, `bundle-v14`,
  `container-context-v13`~`v15`가 남아 있다. `container-context-v15`의
  server SHA-256은 `94ff…f95e4`, index SHA-256은 `a2b7…37f8`이며, 이들은
  v14 server/public 기반이다. 로컬 linux/amd64 OCI image
  `c251c8…7d52d`도 존재한다. 이는 현재 소스 후보가 아니다.
- `worker_monitor.rs`와 그 테스트의 수정 시각은 위 server/context보다
  늦다. 따라서 기존 OCI 결과에는 최신 worker monitor가 들어 있지 않다.
- WSL Ubuntu 22.04에서는 `.local/run-linux-dx.sh`로 캐시한 Dioxus 0.7.9
  runtime loader를 써야 `dx tools assets`를 실행할 수 있다. 확인된 후보
  검사 명령은 다음이다. 새 server/public 묶음과 새 context 경로를 사용한다.

```sh
<windows-workspace>/fediversekr2/.local/run-linux-dx.sh tools assets \
  /absolute/new-bundle/server /absolute/new-bundle/public/assets
python3 scripts/check-linux-runtime.py \
  --binary /absolute/new-bundle/server \
  --public-dir /absolute/new-bundle/public \
  --container-image <새 immutable local image ID>
```

컨텍스트는 `prepare-container-context.py`로 새 빈 경로에 만들고, 그 경로를
`podman build --pull-never --network none`에 준다. 과거 Linux *build* 명령은
기록되어 있지 않아 추정해 재실행하면 안 된다. 새 후보를 만드는 실행자가
동일 source/lock에서 Linux ELF와 web `public/`을 함께 만드는 정확한 명령을
먼저 고정해야 한다.

## 이번 후보의 최소 검증

1. 새 ELF/public에 `check-linux-runtime.py --container-image`를 한 번 실행한다.
   이 기존 검사는 독립 합성 PG14, 실제 SIGTERM, 만료 directory lease와 profile
   batch의 재시작 회수, 같은 프로세스의 worker/미디어 정리를 포함한다.
2. 최신 `check-linux-runtime.py`에 관리자 합성 session의 monitor snapshot,
   익명 접근 거절, 원시 오류/토큰 비공개 검사를 추가했다. 최종 후보 실행 결과는
   [최종 후보 기록](final-release-candidate.md)을 따른다.
3. 같은 검사에 소유한 PG 중단→liveness 유지/readiness 실패→PG 재시작→
   새 작업 1회 완료 검사를 추가했다. 이 검사는 원시 ELF 경로에서 수행하며,
   OCI에서 같은 fault injection을 반복했다고 해석하지 않는다.

## 자원·전환의 남은 게이트

- 현재 검사는 synthetic correctness/lifecycle이지 실제 처리량·RSS·CPU·PG connection
  사용량 측정이 아니다. 이 조건은 무기한 부하 시험으로 넓히지 말고, 전환 직전 snapshot의
  `자동 대상 수 A`, `수동 대기 M`, `profile 대기 P`를 먼저 비밀 없이 **건수만** 확정한 뒤
  같은 수의 staging 자료로 한 주기만 실행한다. 제공된 manifest상 전체 서버는 97개여서
  자동 수집 대상 A의 상한은 알 수 있으나, A/M/P의 실제 값은 기존 덤프 복원 뒤 확인한다.
  새 덤프를 다시 요청할 필요는 없다. 5 directory worker, DB pool 최대 8, 배포 CPU/RAM 제한에서 wall
  time, 최대 RSS/CPU, 최대 PG connections, 완료/재시도/dead 수를 기록하고 예산 초과면
  배포를 멈춘다.
- importer/activation은 의도적으로 literal loopback의
  `fedkr_snapshot_*` → `fedkr_rehearsal_*` → `fedkr_live_*` DB 이름만 허용한다.
  이는 운영 원격 DB에 직접 import하는 명령이 아니다. 유지보수자는 격리 loopback 사본에서
  검토·활성화한 뒤, 승인 DB/origin/media를 실제 runtime 환경에 정확히 제공하는 전달
  방법과 DB 이동/복구 책임을 별도로 확정해야 한다.
- 현재 복귀는 새 쓰기를 먼저 멈추고 보존·수동 복구 범위를 운영자가 결정하는 절차다.
  자동 역이관이나 Phoenix와의 양방향 쓰기는 없다. 실제 backup의 로컬 restore/import/재대조는
  승인 방식 변경 후 통과했다. 발견한 PEM 호환 수정 때문에 기존 Linux/OCI는 재생성해야 한다.
  [실자료 리허설](real-data-rehearsal.md)을 따른다. live HTTPS/AP, volume UID 65532 권한,
  TLS/secret injection, 운영 아키텍처와
  A/M/P 입력은 코드 결함이 아니라 아직 제공되지 않은 운영 입력이다.
