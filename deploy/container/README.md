# 단일 프로세스 OCI 배포

이미 빌드·검증한 Linux Dioxus 배포물을 이미지에 담는다. HTTP와 워커는 같은
실행 파일/컨테이너에서 동작한다. PostgreSQL·프록시·브로커를 추가하거나 실제 운영
서비스를 전환하는 설정이 아니다.

## 배포물 준비와 이미지 빌드

같은 source/lock/assets에서 만든 `server`와 `public/` 전체를 사용한다.
Linux 서버에도 Dioxus 자산 처리가 필요하다. 먼저
[배포물 검증](../../docs/porting/release-lifecycle.md)을 따른다.

프로젝트 루트에서 아래 절대 경로를 검증된 배포물과 **존재하지 않는** 새 출력 경로로
바꾼다. 다음 경로는 예시이며 운영 데이터 경로가 아니다.

```sh
python3 scripts/prepare-container-context.py \
  --bundle /absolute/verified-release \
  --notices /absolute/generated-linux-notices \
  --output /absolute/new-container-context
podman build --pull-never --network none \
  -t localhost/fediversekr2:reviewed-release /absolute/new-container-context
```

베이스 이미지가 로컬에 없으면 실패한다. 승인된 베이스를 별도로 준비해야 하며,
위 명령은 자동으로 내려받지 않는다. Podman 옵션은 로컬 3.4.2 기준이다.
준비 도구는 다음 세 항목만 복사한다. 프로젝트 루트를 빌드 context로 주지 않는다.

```text
server
public/
Containerfile
```

`--notices`에는 [고지 생성](../../docs/porting/third-party-notices.md)의
`linux-server`(x86_64) 또는 `linux-aarch64-server`(aarch64) 결과를 준다. profile은
release ELF와 정확히 같아야 한다. 서버와 WASM을 별도로 분석한 고지 HTML과 공개용
inventory JSON을 `public/`에 추가한다. 누락·변조·다른 target, 변경된 Cargo/자산 입력,
기존 공개 파일과의 충돌은 거절한다.
고지의 입력 해시는 현재 checkout을 확인할 뿐 이미 빌드한 바이너리가 그 checkout에서
나왔다는 증거를 만들지는 않는다. 일치하는 release/lock을 선택하고 아래 실행 검사를 반복한다.

원본/출력의 중첩, 기존 출력 덮어쓰기, 공개 파일의 링크·숨김 파일·특수 파일과
불완전한 묶음을 거절한다. source나 release 루트의 `.env`는 복사하지 않는다.
복사 도중 실패하면 새로 만든 부분 context를 보존한다. ELF 헤더/파일 구조 검사는
실제 실행이나 동일 소스에서 빌드했는지의 증명을 대신하지 않는다.

## 이미지·라이선스 경계

베이스는 다음 multi-architecture index digest로 고정했다. 현재 검증 경로는
`linux-server`의 `linux/amd64`와 `linux-aarch64-server`의 `linux/arm64`를 지원한다.
컨테이너 엔진의 build platform은 bundle ELF와 notice profile에 맞춰 선택해야 하며,
다른 architecture로 emulation·재태깅하지 않는다.

```text
gcr.io/distroless/cc-debian13:nonroot@sha256:54df941ed0d06a1bd95ef5e0ce391fd8d9f94b64782dc9a60062727849ee3f97
```

[공식 Distroless](https://github.com/GoogleContainerTools/distroless)의 OS 런타임
이미지를 사용한다. 셸/패키지 관리자를 전제로 하는 entrypoint나 healthcheck를 넣지 않는다.
베이스 변경 시 digest·아키텍처·사용자·라이선스 및 보안 업데이트를 검토하고 새 이미지로
아래 검사를 반복한다. **이미지 서명 검증은 아직 수행하지 않았다.**

앱 Cargo 의존성의 GPL 계열 회피와 OS 바이너리 이미지의 라이선스는 별개다.
이 이미지를 GPL-free라고 부르지 않는다. 공개 배포 전 베이스 및 앱 의존성의
필수 고지/배포 의무도 확인해야 한다. Cargo·아이콘·폰트 고지는 위 생성 경로로 포함하되,
이것만으로 OS 이미지나 출처 미확정 자산까지 법적 검토가 완료되었다고 간주하지 않는다.
현재는 로컬 이미지이며 레지스트리에 게시하지 않았다.

## 실행 환경 계약

- entrypoint는 `/app/server` 하나, 기본 UID/GID는 `65532:65532`다.
- 기본 listen 주소는 컨테이너 내부 `0.0.0.0:8080`. 호스트 포트 공개 여부는 배포자가 정한다.
- `FEDKR_DATABASE_URL`, `FEDKR_PUBLIC_ORIGIN`은 배포 환경에서 주입한다.
  주소는 정규화된 HTTPS origin이어야 한다. literal loopback HTTP는 개발용 예외다.
- 이미지의 위 두 변수 기본값은 빈 문자열이다. 설정 누락 시 기존 앱 검증이 실패 종료한다.
  원시 바이너리의 의도적인 DB 없는 UI 미리보기와 운영 컨테이너를 구분한다.
- `.env.example`의 개발 trust DB나 개인 스냅샷 경로를 운영 이미지에 복사하지 않는다.
  비밀값을 빌드 인수, 레이어, 명령행 값 또는 저장소에 넣지 않는다.
- 앱은 내장 migration, 이관 DB 활성화 경계, 필요한 저장소와 서명 키를 확인한다.
  검토되지 않은 snapshot/quarantine DB를 런타임에 연결해 우회하지 않는다.

DB가 사설 CA를 사용하면 인증서 묶음을 별도 읽기 전용 파일로 마운트하고
`FEDKR_DATABASE_CA_FILE`에 컨테이너 내부 절대 경로를 지정한다. 이미지에 개인 인증서나
키를 굽지 않는다. [CA 파일 규칙과 TLS 검증](../../docs/porting/postgres-tls.md)을 따른다.
이는 PostgreSQL 신뢰만 추가하며 공개 AP/크롤러 HTTP의 인증서 정책을 바꾸지 않는다.

`FEDKR_MEDIA_DIR`을 쓰면 기존 비공개 디렉터리를 `/media` 같은 절대 경로에 RW로
마운트하고 변수를 설정한다. 앱 UID/GID가 읽기·쓰기·잠금·삭제할 수 있어야 한다.
원본 export/백업이 아닌 **검토된 별도 live 사본**을 사용하고 `.runtime.lock`을 바꾸지 않는다.
DB와 이 저장소는 컨테이너 교체 후에도 유지해야 한다. 이미지 레이어에 두지 않는다.
현재 저장소 구성은 [OpenDAL 경계](../../docs/porting/library-boundaries.md)를 따른다.
S3/PVC 도입·이관이 완료됐다는 뜻은 아니다.

운영 배치에는 read-only root filesystem, 불필요한 capability 제거와
no-new-privileges를 적용할 수 있다. 구체적인 호스트·볼륨 권한·TLS/프록시·비밀 주입
방법은 실제 환경에서 확인한다. 앱 사용자를 root로 바꿔 권한 오류를 우회하지 않는다.

## 상태 확인·종료·전환

외부 HTTP probe를 사용한다. 이미지 안에서 셸/curl 명령을 돌리는 검사는 넣지 않는다.

| 경로/동작 | 의미 |
| --- | --- |
| `GET /healthz` | HTTP 프로세스 응답 확인. DB 장애로 즉시 재시작시키는 용도가 아님 |
| `GET /readyz` | 기동 초기화 완료 후 DB ping 성공 시 200, DB 미설정/조회 실패 시 503. HTTP 검사 제한 3초보다 probe timeout을 여유 있게 설정 |
| SIGTERM | 신규 HTTP 수락 중단, 기존 HTTP 최대 30초/워커 최대 25초 동시 정리 |
| 강제 종료 유예 | 최소 위 정리 시간보다 길게 설정. 로컬 컨테이너 검사는 40초 사용 |

설정된 서버는 HTTP 제공 전에 origin·저장소·내장 migration·활성화 무결성·서명 키를
초기화한다. 이 단계 실패는 503 응답을 계속 제공하는 것이 아니라 프로세스 종료다.
위 3초는 HTTP readiness 검사 제한이며 시작 migration 전체의 제한이 아니다.
readiness 성공은 실자료 이관 승인이나 외부 AP/TLS 정상의 증명이 아니다. 시작 migration에
필요한 시간은 실제 사본에서 측정해 startup probe/배포 제한 시간을 정한다.
Phoenix 중단과 최종 백업·쓰기 차이 보존·복귀는
[컷오버 순서](../../docs/cutover-readiness.md#운영-전환-전-순서)를 따른다.
이미지 태그만 되돌려서는 두 버전의 DB 쓰기 일관성이 복구되지 않는다.

## 로컬 OCI 검증

rootless Podman과 Linux PG14 도구가 있는 일반 사용자 환경에서 실행한다.
`--container-image`는 로컬의 immutable 64자리 이미지 ID만 받으며 pull/push하지 않는다.

```sh
python3 scripts/check-container-context.py
python3 scripts/check-linux-runtime.py \
  --binary /absolute/verified-release/server \
  --public-dir /absolute/verified-release/public \
  --container-image <local-image-id>
```

검사는 기존 listener가 없는 loopback:16439에 자신만의 일회용 **합성** PG를 만들고,
원시 ELF 검사에 이어 같은 PG/미디어 사본으로 OCI를 검사한다. 운영 DB/실제 ZIP을 입력받지 않는다.
server/public을 마운트로 덮지 않으며 이미지 실행 파일/index 해시와 공개 파일 HTTP 바이트,
실제 SSR을 대조한다. 관리자 합성 세션으로 로고 업로드→OpenDAL 저장→HTTP 조회→컨테이너
재생성→삭제와 같은 프로세스의 파일 정리를 확인한다. 공개 인증글 로그인 검사는 아니다.

구 Podman은 최신 keep-id UID 재매핑을 지원하지 않아, RW bind 검사는 호스트 일반 사용자
UID로 실행한다. 이미지 기본 65532는 별도 설정 누락 실행에서 사용한다. 따라서 **운영
볼륨의 UID 65532 권한까지 검증했다고 해석하지 않는다.** 합성 DB 접근만을 위한
host network/loopback 사용은 운영 배치 추천이 아니다. 생성한 정확한 이름+소유 라벨의
컨테이너와 합성 PG/파일만 정리하고 빌드 이미지는 보존한다.

최종 통과/실패와 검증 수는 [통합 검증 기록](../../docs/porting/integration-checks.md)에 남긴다.
실제 백업, 운영 HTTPS/AP, 브라우저 hydration, 이미지 서명, 운영 볼륨 및 전환 리허설은 별도다.
