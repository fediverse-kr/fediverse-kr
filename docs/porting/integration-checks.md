# 회원·연합 인증 로컬 통합 검증

## 2026-09-14 비공개 ZIP 준비와 AES 변조 회귀

- [비공개 ZIP 도구](private-archive.md)는 SQL/PG 복원이 아닌 안전한 해제·inventory
  검사까지 맡는다. 기존 PG 도구의 경로/ACL 함수를 부작용 없는 공통 파일로 분리했다.
  `check-private-import.ps1` 재실행 **14개** 통과. 새 빈 PG는 정상 Stop했고 자료는 보존했다.
- `check-private-archive.ps1` 최종 **64개** 통과. pyzipper 0.4.0 + pycryptodomex 3.23.0으로
  독립 생성한 15종 합성 ZIP을 실제 SharpZipLib 1.4.2 reader와 새 PowerShell 프로세스에서
  검사했다. 정상 6개 파일·빈 파일·명시 빈 디렉터리·한글 경로의 보존, SHA-256과 ACL,
  별도 Check의 비변경성을 확인했다.
- 잘못된 암호, AES authentication tag 한 바이트만 변조, 경로 탈출/절대경로/백슬래시,
  대소문자 및 파일/디렉터리 충돌, symlink, Windows 예약 이름, 과도한 압축 비율,
  257개 항목, BZIP2와 비암호화 파일을 거절했다. 출력 스트림 상한·만료 예산,
  manifest schema/state/batch/hash/path/size/count 변조와 출력 파일 변조/추가/누락도 확인했다.
- 실제 검증에서 SharpCompress 0.50.4가 WinZip AES auth tag 변조를 수용했다.
  고정 원본 코드에서도 auth bytes를 읽기만 하고 비교하지 않는 점을 확인하여 사용을 중단했다.
  MIT SharpZipLib 1.4.2로 교체했으며 암호화/ZIP/CRC 알고리즘을 직접 만들지 않았다.
  schema 2부터 새 검증 계약을 사용하고 기존 schema 1 complete 결과는 Check가 거절한다.
- 최종 합성 fixture는 `.local/archive-check-e3bbdd8937114d17b9b945b1580e5aa7`.
  source ZIP 15개 해시는 전부 원래와 같았다. 새 비공개 출력 배치 8개는 재검토를 위해
  보존했다. 고의 변조/누락 검사 뒤의 출력은 당연히 검증 불가 상태이며 실자료가 아니다.
- SharpZipLib 공식 NuGet의 저장소 서명 검사 통과, DLL SHA-256 고정, MIT 원문 보존.
  fixture 도구의 MIT/PSF/BSD·public-domain 고지도 보존했다. 앱에 도구 DLL/Python을
  포함하지 않았고 Cargo.toml/Cargo.lock은 직전 고지 작업 이후 변하지 않았다.
- 이번 증거는 **합성 ZIP + 빈 독립 PG**다. 실제 백업 복호화/SQL 실행/실회원·이미지
  이관이나 실제 AP 인증을 하지 않았다. 웹·Rust·OCI 재빌드/회귀 결과를 추가한 것도 아니다.
  비-AES CRC 코드는 라이브러리를 사용하지만 ZipCrypto 전용 변조 fixture는 이번 범위에 없다.

## 2026-09-14 제3자 고지 생성과 실제 OCI 포함

- cargo-about **0.9.2** 공식 Windows 릴리스 archive의 SHA-256을 GitHub 릴리스 메타데이터와
  대조했다. 작업 폴더 도구로만 준비했고 앱 의존성·전역 도구·Cargo.lock은 바꾸지 않았다.
  Cargo.toml에는 crates.io 게시 대상이 아니라는 `publish=false`만 추가했다. 프로젝트 자체의
  소스 라이선스를 지정하지 않았으며 이전 Linux v14 서버 바이너리를 그대로 재사용했다.
- [고지 생성 계약](third-party-notices.md): 라이선스 선택/AND·OR 처리는 cargo-about,
  원문·NOTICE 보강과 안전한 배포 형식은 얇은 Python 도구로 연결했다. raw Cargo JSON의
  머신 경로는 공개하지 않는다. source/lock/about/Dioxus/보강/실제 자산 해시를 보존하고,
  생성 중 변경·잘못된 플랫폼·미완성 결과·기존 출력 덮어쓰기를 거절한다.
- 검토 담당자가 MIT 원문과 자산 고지를 수집하고 구현 담당자가 생성기 및 회귀 검사를 구현했다.
  통합 담당자는 Apache/ISC·ASF/Moka NOTICE, 실제 crate archive 대조, context 연결과 실행을 담당했다.
  보강 74개 패키지/버전/라이선스 항목 중 **7개는 별도 LICENSE 없는 선언 자료**다.
  그 7개 Cargo.toml은 공개 원본 archive와 바이트가 같고 archive SHA-256은 lock과 같다.
  HTML/inventory에 선언-only 경고를 유지한다. 검토되지 않은 문구를 창작하지 않았다.
- 원격 원문 URL **24개**를 다시 대조했다. **22개 byte-exact**, components/sdk **2개는 EOF
  LF 한 바이트 차이만** 있다. 문구는 같으며 두 해시를 위 계약 문서에 구분했다.
- 최초 실제 생성은 선언 파일명 제한 때문에 실패했고 출력은 없었다. 해당 이름 규칙을
  고친 뒤에는 `mac`의 옛 `MIT/Apache-2.0` 표기를 도구가 `MIT OR Apache-2.0`로 표시해
  대조가 실패했다. 정확한 패키지·버전·archive hash에만 옛 표기 대조를 허용했다.
  일반 SPDX 파서를 추가하거나 다른 식을 허용하지 않았다. 수정 중의 합성 검사 1회는
  Windows fixture 줄바꿈/해시 불일치로 실패했고 최종 검사에 포함해 재통과했다.
- 최종 생성기 합성 검사 **10개**, context 합성 검사 **Windows 27개 / Linux 27개** 통과.
  실제 offline/locked 생성 결과는 Linux/server **430**, Windows/server **433**, web **241**개다.
  모든 selected license에 GPL/LGPL/AGPL은 없다. root/app 및 OS runtime 라이선스 판단과 구별한다.
- 별도 Cargo tree normal/build 대조는 Linux/server 430개와 Windows/server 433개가 정확히
  일치했다. wasm tree는 host `dioxus-router-macro → sha2 → cpufeatures`까지 세어 **242개**다.
  그 cpufeatures는 server 고지에 이미 포함된다. server+web 합집합 **Linux 438 / Windows 441**개가
  모두 해당 묶음의 고지에 있다. web 241개를 host 도구 전체까지 센 수치라고 주장하지 않는다.
- `.local/notices-linux-v1` 및 `notices-windows-v1`에 각각 HTML/inventory를 생성했다.
  HTML hash·선언-only 경고·ASF/Moka 원문·머신 절대 경로 비노출을 확인했다. 생성 결과를
  기존 release에 덮어쓰지 않고 `container-context-v15`의 public/에 두 파일을 추가했다.
- 새 로컬 OCI **ELF+이미지 208개 / 공개 파일 54개 / 70.19초 통과**.
  고지 두 파일의 실제 HTTP 바이트를 원본과 대조했고, 비root/read-only·SSR·자산·같은
  프로세스 워커·OpenDAL 업로드/재시작/삭제·SIGTERM도 재검증했다. 서버 바이너리/index는
  v14와 같고 새 Rust 빌드나 Windows 실행/브라우저 전수 검사를 했다는 뜻은 아니다.
- 실제 ZIP/회원 DB/서명 키/운영 환경은 읽거나 변경하지 않았다. 이미지는 로컬에만 있으며
  push/트래픽 전환을 하지 않았다. 선언-only 7개와 출처 미확정 자산, 베이스 이미지 고지·서명,
  실제 자료 이관·HTTPS/AP 호환 및 컷오버 리허설은 여전히 별도 검토 대상이다.

```text
app image: af73fb6f98c66013a98a5f970bff531a20f7bf2c9a320f1fc286d967094ac09f
server SHA-256: 94ff1c14a0d4e014a7740fbc555451395ba38e2534ec3134e728e4e0543f95e4
Linux notices HTML SHA-256: b3e57a6fc333fc906f45dccf798c34d19963dd1d049bfbf798573b4788ca3b4e
Windows notices HTML SHA-256: 995617aeaba66dfa7fdf090f074e98418f7cc06c4c72bbb73030b755fe6aeadb
```

## 2026-09-14 사설 CA PostgreSQL TLS와 배포 회귀

- 기존 DB 연결은 공인 CA만 지원했다. 선택형 `FEDKR_DATABASE_CA_FILE`을 DB transport
  모듈에 추가했다. 기존 Rustls/PemObject/RootCertStore와 tokio-postgres-rustls만 사용한다.
  Cargo.toml/lock 해시는 직전과 같고 새 의존성·ORM·migration·프로세스는 없다.
  CA를 지정하면 loopback도 TLS 필수다. AP/크롤러/시스템 trust는 바꾸지 않았다.
- [TLS 계약과 재현](postgres-tls.md): 최대 256 KiB의 절대 일반 PEM certificate 파일,
  CA 검증과 hostname 확인, 설정 실패 시 기동 거절과 비밀 비노출을 문서화했다.
  클라이언트 인증서와 실제 운영 인증서는 이 범위가 아니다.
- 기존 `bundle-v13`을 새 TLS fixture에 실행하면 readiness 전에 종료됐다.
  fixture의 libpq verify-full 제어 연결은 먼저 성공했으며, 기존 코드는 새 CA 환경변수를
  지원하지 않았다. 테스트 PG/인증서는 이 실패 후에도 정리했다.
- 구현 담당자가 transport/단위 검사를 구현했고 통합 담당자가 실제 TLS PG fixture를 작성했다.
  검토 담당자가 hostname 검사에서 localhost의 IPv6 접속 실패가 잘못된 통과 원인이 될 수 있음을
  지적했다. hostaddr=127.0.0.1로 목적지를 고정하고 같은 연결의 verify-ca 제어 요청을
  추가했다. 서버의 hostname 검증을 완화하지 않았다.
- 최종 Linux `bundle-v14`의 `check-postgres-tls.py`: **47개/4.23초 통과**.
  생성한 CA/서버 인증서로 내장 migration 25개와 실제 풀의 `pg_stat_ssl=true`를 확인했다.
  잘못된 CA/hostname, 키가 섞인 PEM, symlink·없음·빈 파일·과대·상대 경로 등을 거절했고,
  평문 제어 요청이 성공하는 TLS-off PG에서도 CA 설정 앱은 연결을 거절했다.
  SIGTERM과 실패 기동 후 기존 서명 키 보존, 오류 출력 비밀 비노출도 확인했다.
- Windows 전체 Rust/격리 PG **250개 통과, 실패·무시 0개 (144.32초)**.
  최종 소스의 새 transport 검사 7개가 포함된다. Linux 새 transport 집중 **7개**도 통과.
  전체 Linux Rust 전수를 다시 실행했다는 뜻은 아니다.
- Linux 최종 release build **59.46초**, ELF/OCI/집중 검사 묶음 **205개/공개 파일 52개,
  134.97초** 통과. TLS fixture와 별도로 기존 loopback DB 경로, 이미지 내 바이너리/index/
  공개 파일 대조, read-only·비root, 실제 OpenDAL 업로드/재생성/정리와 종료를 재검증했다.
  TLS 47개는 Linux ELF 검사이며 컨테이너의 사설 CA 마운트 검증으로 확대하지 않는다.
- Windows 최종 fullstack release **106.08초**, 합성 이관·활성화/HTTP·워커 **127개/공개 파일
  52개** 통과. Windows 새 public 전체와 Linux v14 public의 파일/바이트가 같음을 대조했다.
  검토용 서버를 새 Windows 배포물로 재시작했고 `/readyz` 200을 확인했다.
  UI/웹 코드 변경은 없으며 이번에 브라우저 전수를 다시 검증한 것은 아니다.
- 일회용 TLS 인증서/PG와 Linux 합성 클러스터, 소유 라벨 컨테이너가 남지 않음을 확인했다.
  실제 ZIP/회원 자료/운영 DB/시스템 인증서 저장소는 읽거나 변경하지 않았다.
  운영 PG 접속 권한·실제 CA·볼륨 UID·HTTPS/AP·이미지 서명·컷오버 리허설은 별도 남아 있다.

```text
app image: c251c8ca53db1be6d4aab5b233525bb26a434541e26b796a251a25368627d52d
server SHA-256: 94ff1c14a0d4e014a7740fbc555451395ba38e2534ec3134e728e4e0543f95e4
index SHA-256: a2b755212022a7371db1c3aa4a775c9fa42110772742f67db7860f52167c37f8
```

## 2026-09-14 단일 프로세스 OCI 배포물 실행

- [OCI 실행 설정](../../deploy/container/README.md)과 제한된 context 준비 도구를 추가했다.
  검증된 Linux `bundle-v13`의 server/public만 복사한다. 기존 디렉터리 덮어쓰기·경로 중첩·
  링크·숨김 파일·불완전한 묶음 거절 등 합성 준비 검사 **17개** 통과. 이 결과는 ELF 실행 증거가 아니다.
- 로컬 WSL rootless Podman 3.4.2로 고정 digest의 distroless runtime 위에 이미지를 빌드했다.
  빌드는 네트워크 없이 로컬 베이스만 사용했다. 베이스 OS 바이너리 이미지는 별도 공개 pull로
  준비했으며, 앱 이미지를 게시/push하거나 프로덕션/클러스터를 변경하지 않았다.
  HTTP·PG 기반 워커·OpenDAL 정리는 기존 `/app/server` 한 프로세스다. PG 컨테이너는 만들지 않았다.
- 첫 OCI 검사는 **설정 누락 오류 검사에서 실패**했다. 같은 이미지의 독립 실행은 exit 1과
  `Invalid public origin`을 반환했으나, 검사기가 Podman logs의 stdout만 읽어 stderr를 놓쳤다.
  구현 담당자가 양쪽 출력의 비노출 검사를 수정했다. 앱의 설정 검증을 완화하지 않았다.
- 리뷰에서 이미지의 서버 실행 파일이 검사 대상과 같은지 빠져 있음을 확인했다. 정확한 소유
  라벨 컨테이너에서 `/app/server`, `/app/public/index.html`을 private temp로 복사해 SHA-256을
  비교하고 SSR 자산 참조 최소 개수도 검사한다. server/public을 호스트 파일로 overmount하지 않는다.
- 수정 후 `check-linux-runtime.py --container-image <immutable local ID>`:
  실제 ELF와 OCI를 합쳐 **204개/공개 파일 52개, 69.97초** 통과. 소스 구문 검사만의 결과가 아니다.
  이미지 기본 비root/단일 entrypoint, 설정 누락 시 거절, read-only rootfs, DB 연결/SSR/정적 파일
  바이트, 실제 API 로고 업로드→외부 OpenDAL 저장→컨테이너 재생성 후 키·파일 보존→삭제와
  같은 프로세스의 파일 정리, SIGTERM 정상 종료를 확인했다. index는 별도 바이트 대조했다.
- RW bind 검사는 구 Podman keep-id 제한 때문에 호스트 일반 사용자 UID로 실행했다.
  **운영 볼륨에서 이미지 기본 UID/GID 65532 권한을 확인한 것은 아니다.** 외부 HTTP probe,
  40초 종료 유예와 실제 배포 환경의 권한/비밀 주입 점검을 위 문서에 적었다.
- 최종 검사 후 소유 라벨 컨테이너와 `linux-e2e-*` 합성 클러스터 경로가 없음을 확인했다.
  로컬 이미지/준비 context는 재현용으로 보존했고 Windows 검토 서버 `/readyz`는 200이다.
  구문 검사 도중 생성된 `scripts/__pycache__` 두 파일은 삭제 정책 거부로 그대로 두고
  Git 제외만 추가했다. 우회 삭제하지 않았다.
- 서버 소스/Cargo 의존성/내장 migration/웹 파일은 직전 검증 배포물 그대로다.
  실제 백업·회원 자료, 운영 HTTPS/signed AP, 브라우저 hydration과 운영 트래픽 전환은 검증하지 않았다.
  베이스 이미지 서명과 공개 배포 고지 검토도 남는다. OS 이미지 라이선스와 앱 의존성 검토를 구별한다.

로컬 검증물 식별(개인 자료/자격 증명 아님):

```text
app image: 3e5e12ab626cc5ceaf666dbae9ac379873192eb2fcb02e5e0e632b8d4e92580b
server SHA-256: 4aa792d2e746031ae792dca74f30964afd20227c43d0fbc7a0cc6ceab6070a2c
index SHA-256: a2b755212022a7371db1c3aa4a775c9fa42110772742f67db7860f52167c37f8
```

## 2026-09-14 수집 SVG·이관 favicon 보존과 실제 공개 정보 조회

- MIT Phoenix favicon worker는 SVG를 저장하고 기존 favicon 키가 있으면 자동 재수집하지 않는다.
  새 수집기/공개 캐시를 기존 `quick-xml` SVG 식별·공통 이미지 CSP에 연결했고,
  유효한 이관 `favicons/` 파일 매핑이 있으면 자동 수집에서 보존한다. 운영자 수동 갱신은 예외다.
- 처음 Windows `directory` 집중 검사는 **15 통과/4 실패**였다. 저장 MIME CHECK가 SVG를
  거부했고, 새 테스트가 완료 작업의 임대를 비우지 않아 후속 큐 검사에 잔여 자료가 섞였다.
  내장 migration **025**로 명시적 MIME 목록을 맞추고 테스트 임대 값을 수정했다.
  실패 때 생성한 사이트 8개·legacy 행 4개·파일 매핑 2개만 UUID/도메인으로 대조해 정리했다.
  기존 migration/실자료/실제 작업 제약을 덮어쓰거나 완화하지 않았다.
- 구현 담당자가 수집기/DB/025를 구현하고 통합 담당자가 통합했다. 검토 담당자가 브라우저/실행 검사의 범위를
  검토하여, DB에 직접 넣은 SVG 조회를 실제 원격 수집 성공으로 부르지 않도록 이름을 고쳤다.
- Windows 전체 Rust/격리 PG **243개 통과, 실패·무시 0개 (185.38초)**.
  모의 원격 SVG → 실제 수집기 → 실제 PG 작업 완료/캐시 조회, 잘못된 SVG의 ICO fallback,
  자동/수동 수집 구별, migration 반복·기존 PNG 바이트 보존·downgrade 거절을 포함한다.
  허용 외 MIME 검사가 FK 오류로도 통과하지 않도록 유효한 사이트를 쓰게 보강한 뒤,
  해당 migration 검사 **1개를 별도로 재실행**해 통과했다. 실제 외부 favicon 다운로드 검사는 아니다.
- 최종 Windows fullstack release **121.24초**, Linux release **1분 11초** 빌드 성공.
  Windows 합성 이관·활성화/실제 HTTP·워커 lifecycle **127개/공개 파일 52개** 통과.
  Linux `bundle-v13` 실제 ELF/HTTP/자산/워커 lifecycle **125개/공개 파일 52개**,
  별도 PG14 `directory` 집중 Rust 검사 **21개/실패·무시 0개 (2.75초)** 통과.
  Linux 실행 묶음은 집중 검사 빌드 포함 127.62초이며 전체 Rust 전수 재실행은 아니다.
- Playwright 스킬로 실제 Windows release 브라우저: SVG **21개**, 기존 PNG **18개**, 각각
  런타임 예외 0개. 목록/상세 실제 디코딩, GET/HEAD/405, 바이트 길이, 오류 이름표, 숨김 후
  404/검색 제외/복구를 확인했다. SVG 직접 문서의 스크립트·같은 origin fetch·외부 image는
  차단되고 인라인 스타일은 유지됐다. SVG 캡처를 직접 확인했다. 전체 반응형 UI 전수 검사는 아니다.
- `check-live-site-preview.ps1 -AllowPublicNetworkRead`를 최종 release에서 실행했다.
  `mastodon.social` **200/0.49초**, `misskey.io` **200/0.38초**: 실제 공유 `PublicHttp`의
  DNS·TLS·루트/NodeInfo 조회로 제품 식별·회원 수/가입 상태 필드 존재를 확인했다.
  로컬 합성 회원으로 미리보기만 호출했으며 사이트를 등록하거나 원격 글을 게시하지 않았다.
  이는 unsigned 공개 정보 조회의 증거다. signed AP·실회원 로그인·외부 favicon 수집·운영
  HTTPS 주소의 인증 상호운용을 확인했다는 뜻은 아니다. 원격 상세 응답/회원 정보/토큰은 출력하지 않았다.
- 생성한 브라우저 회원/사이트/아이콘과 Linux PG·실행 프로세스, Windows lifecycle DB/파일을
  정리했다. 개발/검사 DB의 회원·사이트·작업·파일은 각각 0개, 새 migration 25개 적용과
  미리보기 readiness 200을 확인했다. 테스트 fixture는 재생성 가능하다.
  실제 암호화 ZIP·회원·Phoenix 운영 서버는 변경하지 않았다.
- Cargo.toml/lock 해시는 직전과 동일하며 새 라이브러리/GPL 계열/SQLx/bcrypt/컨테이너는 없다.
  [수집 계약](diesel-and-runtime.md), [공개 아이콘](server-discovery.md).

## 2026-09-14 이관 이모지 이름·기존 래스터 형식 호환

- MIT Phoenix `AvatarFetcher`는 shortcode 원문을 JSON key에 보관하고 파일명만 정리한다.
  기존 새 런타임은 이름을 ASCII 영숫자/밑줄로 제한해 일부 원본 이미지가 누락된 것처럼
  표시될 수 있었다. 원문 map lookup과 canonical query URL로 바꾸고 옛 path alias도 유지했다.
  dot 경로 정규화·한글·slash·plus·percent를 구별하며 malformed escape/UTF-8·중복 이름은 400이다.
- 래스터 MIME의 자작 시그니처 분기를 이미 쓰는 `image 0.25.10`으로 교체했다.
  기존 5종 외 AVIF/BMP/TIFF를 명시 허용하고 bytes는 보존한다. 추가 decoder/dependency는 없다.
  원본 바이트 조회를 새 업로드 디코딩 정책이나 모든 브라우저의 형식 지원과 혼동하지 않는다.
- 구현 담당자 구현·통합 담당자의 통합/검증, 검토 담당자 lifecycle 검사 작성·구현 담당자 읽기 검토로 진행했다.
  Windows 집중 Rust/격리 PG: `media` **19개**, `crawler` **4개**, `emoji` **6개**,
  `membership::profile` **2개**, `federation::profile` **1개** 통과. 중복 제외 **27개**,
  실패/무시 0개. 전체 239개 전수 재실행 결과는 아니다.
- 최종 Windows fullstack release(109.32초), Linux release(1분 6초) 성공.
  `check-activation-lifecycle.ps1` **127개/공개 파일 52개** 통과: 원본 6개 shortcode,
  4개 논리 파일/3개 고유 바이트, BMP MIME/바이트, 승인·세션·탈퇴·재시작/변조 검사를 포함한다.
  처음 정적 확인에서 .NET Uri가 `%ZZ`를 `%25ZZ`로 고치는 것을 발견해 검사 요청만
  원문 target을 보존하게 보정했다. 서버 입력 검증을 완화하지 않았다.
- 최종 Linux v12 배포물의 HTTP·자산·워커 수명주기 **121개/공개 파일 52개**, 54.01초 통과.
  query 이름/별칭/잘못된 query/익명·탈퇴 후 차단을 실제 Linux HTTP로 검사했다.
  Windows importer 검사를 Linux importer 검증으로 확대해서 해석하지 않는다.
- Playwright 스킬로 최종 release 브라우저 **89개**, 런타임 예외/예상 밖 요청 **0개**.
  1440/390/320px에서 특수 이름 3개가 공통 UI의 정확한 query로 요청되고 실제 BMP로 디코딩됐다.
  SVG sandbox/CSP, 공개 로고·사이트 아이콘, 본인만 조회, HEAD·로그아웃도 확인했다.
  데스크톱/320px 캡처를 직접 확인했고 UI 디자인은 변경하지 않았다.
- 재생성 가능한 테스트 행/파일과 소유한 브라우저·Linux PG는 정리했다. Windows 개발/검사 DB의
  회원·서버·댓글·stored_files는 각각 0개, 최종 미리보기 `/readyz`는 200이다.
  암호화 ZIP/실회원/운영 서버는 건드리지 않았다. 실제 백업 대조·원격 AP 성공은 여전히 미검증이다.
- Cargo.toml/lock 해시는 앞선 라이브러리 감사와 동일. 내장 migration 24개 유지,
  SQLx/bcrypt/새 컨테이너/GPL 계열 의존성 추가 없음. [접근 계약](media-serving.md).

## 2026-09-14 실자료 전용 빈 PG 준비 도구

- 검토 담당자 초안을 구현 담당자가 검토·보완하고 통합 담당자가 실제 Windows PG17으로 검증했다.
  [보안 경계와 사용법](private-import-cluster.md). 데이터 디렉터리는 저장소/개발 PG와
  분리한 LocalAppData, 접속은 loopback의 새 포트와 SCRAM, 자격 정보는 Windows DPAPI다.
- 초기 SID 문자열의 ACL 해석 실패와 DACL 외 권한 요청은 DB 생성 전에 중단되었다.
  중간 실행의 분리 프로세스 출력 EOF 대기는 정확히 그 실행의 준비 helper만 종료하고,
  선택 PID가 없는 준비 상태 manifest 처리와 출력 drain 상한을 보강한 뒤 해당 PG를
  `pg_ctl`로 정상 중지했다. PG 전체를 강제 종료하거나 기존 개발 DB를 지우지 않았다.
- 최종 `check-private-import.ps1` **14개 통과**. 실제 빈 DB 생성/설정/권한/틀린 암호,
  잘못된 batch·manifest·credential 경로·DB 이름·프로세스 ID 거절, 기존 루트 보존,
  정상 종료/재종료를 확인했다. 두 준비 DB의 업무 테이블은 0개이며 실백업은 입력하지 않았다.
- 성공 배치 `44a948c7cadc4b90ab75263a0fe8ae05`와 중단 배치
  `61146321103742eba1bf73327a719ad4`의 PG는 중지하고 디렉터리를 보존했다.
  중단 시 남은 비공개 initdb 암호 파일 1개의 exact-path 삭제는 도구 정책으로 거부되어
  우회하지 않았다. 실제 ZIP 암호·회원 정보와 관계없는, 생성한 빈 테스트 DB 자격 정보다.

## 2026-09-14 Phoenix actor/inbox 계약 복원

- MIT 원본과 현재 공개 `/actor` GET 응답을 대조해 inbox URL과 host 기반
  `preferredUsername`을 복원했다. 기존 actor/key ID와 이관된 키는 바꾸지 않는다.
  inbox는 빈 202 응답만 보존하고 활동을 저장·처리하지 않는다.
  [계약과 요청 상한](actor-compatibility.md). 회원 API의 Origin/본문 검사는 그대로다.
- Windows `actor_` 집중 검사는 합성 PG를 포함해 **14개 통과, 실패·무시 0개 (5.77초)**,
  회원 HTTP 경계 **4개 통과**. 앞선 PG 미지정 실행은 11개 통과/3개 무시였으며,
  그 뒤 같은 테스트 바이너리로 격리 PG를 지정해 14개를 확인했다. 전체 Rust 전수는 아니다.
- 최종 Windows fullstack release **122.87초**, Linux release **1분 43초** 빌드 성공.
  배포물의 실제 Windows 이관·활성화·HTTP/워커 재시작 **111개**, Linux `bundle-v11`
  ELF/자산/워커 종료·재시작 **103개 (58.53초)** 통과. 양쪽 공개 파일 각 50개를 포함한다.
  기존 키 유지, inbox 광고, Origin/쿠키 없는 activity+json POST의 빈 202/no-store,
  GET 405와 본문 초과 413을 실제 HTTP로 확인했다.
- 리허설은 자신이 생성한 합성 DB·파일·프로세스만 정리했다. 미리보기는 새 배포물로
  재시작해 readiness 200을 확인했다. 실제 백업 복원·원격 signed AP·TLS·브라우저 시각
  검토를 했다는 뜻은 아니다. 라이브러리 목록과 lock 해시는 이전과 동일하다.

## 2026-09-14 관리자 사이트 삭제 통합

- 구현 담당자가 DB/024 migration/테스트를, 검토 담당자가 관리 화면·브라우저 검사 초안을 맡았다.
  구현 담당자의 화면 상태 검토와 통합 담당자의 통합 뒤 같은 바이너리/API에 연결했다.
  [삭제 범위와 보존 계약](site-moderation.md). 새 의존성·프로세스·컨테이너는 없다.
- 첫 Windows 전체 Rust/PG 검사는 **227 통과/2 실패 (193.93초)**였다.
  신규 테스트가 오래된 세션의 인증 시각만 수정해 기존 시각 제약을 어겼고,
  그 panic이 남긴 합성 사이트 때문에 후속 큐 검사의 빈 큐 조건도 실패했다.
  테스트 시각을 일관되게 수정했다. 실제 권한 검사나 DB 제약은 완화하지 않았다.
- 첫 실제 release 브라우저는 **24개 통과**, 이후 화면 캡처에서 발견한 안내문의
  도메인 치환 오류를 고쳤다. 최종 Windows fullstack **121.24초**, Linux release
  **1분 44초** 빌드 후 브라우저 **25개 통과/pageerror 0개**를 확인했다.
  확인 누락·취소·익명/Origin/본문 상한 거절, 실제 삭제와 신고 UUID/원문 보존,
  1440/390/320px 폼·20px 확인란·실제 도메인 문구를 포함한다.
- 최종 배포물에서 두 차례 생성한 브라우저 합성 회원·사이트·댓글·신고는 정확한
  fixture UUID로 정리했다. 실제 회원 백업·외부 사이트·운영 DB에는 쓰지 않았다.
- Linux 최종 `bundle-v10`의 실제 ELF/웹 자산/같은 프로세스 워커/종료·재시작
  검사는 **98개 통과**(공개 파일 48개, 123.54초), 별도 PG14의 신규 삭제·downgrade
  집중 검사 **3개 통과**였다. Linux 전체 Rust 전수 재실행은 아니며, 브라우저 검증은
  위 Windows 실행 파일에 대한 것이다. 원격 AP 성공·운영 호스트/TLS·실백업 증거는 아니다.
- 수정 후 Windows 전체 Rust/PG17 검사는 **229개 통과, 실패·무시 0개 (174.28초)**.
  새 삭제 권한·신선도·revision, 이관된 관계의 삭제/신고·공유 파일 보존, 오래된 워커 차단,
  24개 내장 migration과 downgrade 거절, 기존 회원·이관·수집 회귀를 포함한다.
  두 실패 실행이 남긴 합성 사이트 2개와 회원 4개는 exact UUID/domain/fixture 표시를
  확인한 뒤 정리했고, 실제 데이터나 다른 기존 리허설 DB를 정리 대상으로 삼지 않았다.

## 2026-09-14 관리자 전체 프로필 갱신 복원

- Phoenix `AdminLive.refetch_all_profiles`를 관리자 메뉴, 개인 갱신과 같은 AP/OpenDAL,
  같은 서버 프로세스의 PG 큐로 연결했다. [기능/권한/보존 계약](profile-media-refresh.md).
  내장 migration **23개**. Cargo.toml/lock 해시는 사진 갱신 시점과 동일하며 새 의존성은 없다.
- 새 PG/OpenDAL/이관 집중 검사 **8개**. 첫 Windows 실행은 테스트 정리 helper가
  transaction 없이 `LOCK TABLE`을 호출하고, 과거 세션의 인증 시각만 되돌리면서 기존
  시간 제약을 어긴 문제로 실패했다. 테스트 helper를 수정하고 정확히 그 실행에서 남은
  합성 회원 2개와 무참조 파일 매핑 8개를 정리했다. 런타임 권한/저장 로직을 완화하지 않았다.
- 첫 전체 검사는 **225 통과/1 실패**: migration 개수 assertion이 옛 22개에 고정되어 있었다.
  23개 및 신규 두 테이블 존재를 확인하도록 갱신했다. 이후 전체 Windows Rust/PG17
  **226개 통과, 실패·무시 0개 (219.51초)**; Linux release Rust/PG14
  **227개 통과, 실패·무시 0개 (155.32초)**. 기존 개인 사진·회원·댓글·건강 이력·이관 포함.
- 실제 release 재시작 검사에는 만료된 프로필 임대를 넣고 같은 실행 파일에서
  재획득/실패 기록/완료 처리를 확인하는 경로를 추가했다. 처음의 합성 연동 fixture에
  필수 표시 이름이 빠진 SQL 오류를 고친 뒤 Windows **96개**, Linux **89개** 통과
  (각 공개 파일 39개, Linux 전체 Rust 검사 포함). 원격 주소는 네트워크 전 거절되며
  성공 AP 통신의 증거는 아니다.
- 첫 release 브라우저는 실제 PG의 합성 회원 31명으로 시작·진행·실패 1/건너뜀 30·중단·
  재로드·개인 이미지/이름/비공개 설정 보존 **22개**, pageerror 0개 통과.
  시각 검사에서 확인란이 큰 입력창 스타일을 받는 문제를 발견해 기존 `owner-checkbox`를
  재사용하고 하단 간격을 조정했다. 경로는 `/account/moderation/profiles`로 통일했다.
  수정이 진행 중인 빌드와 겹친 중간 배포물에서는 CSS 404가 있어 최종 성공으로 세지 않는다.
  변경을 고정한 뒤 재빌드·최종 브라우저/배포물 검사를 수행했다.
- 최종 Windows fullstack release **115.40초**, Linux release **1분 41초** 빌드.
  최종 브라우저 **25개 통과**, 1440/390/320px의 공통 20px 확인란, 하단 여백과 새 경로,
  pageerror 0개, 로드한 stylesheet 8개 전부 HTTP 200 확인. 390px 캡처를 직접 확인했다.
  `output/playwright/profile-batch-{1440,390,320}.png`.
- 최종 배포물 수명주기 Windows **101개** / Linux **93개**, 각 공개 파일 44개 통과.
  Linux 검토용 묶음은 `bundle-v9`이며 서버 응답 이력·내 댓글·전체 프로필 갱신까지 포함한다.
  마지막 재빌드 이후에는 UI 경로/스타일 변경에 해당하는 release 검사를 재실행했고,
  전체 Rust/PG 결과는 위 시점의 동일 백엔드 로직에 대한 결과로 구분한다.
- 모든 브라우저/수명주기 자료는 합성이다. 실회원 백업·성공 원격 AP·운영 TLS·트래픽 전환은
  검증하지 않았다. 개발/test 회원·프로필 작업·파일 매핑은 정리했고 실제 백업은 읽지 않았다.


## 2026-09-14 내 댓글 동선 복원

- [개인 댓글 목록](community.md)을 계정 메뉴/API/SSR에 연결했다. 새 테이블·migration·
  라이브러리는 없으며 Cargo.toml/lock 해시는 사진 갱신 완료 시점과 동일하다.
- 최초 컴파일의 RSX 문자열 안 따옴표/이동된 오류 값 표현을 일반 expression으로 수정했다.
  이후 전체 Windows Rust/PG17 **218개 통과, 실패·무시 0개 (175.36초)**.
  본인/타인·삭제·숨김·20+5페이지·장문·위조/폐기 세션·ban을 검사했고,
  기존 합성 Phoenix 로그인 검사에서 옛 UUID의 댓글 본문·서버 귀속이 그대로 나오는 것도 확인했다.
  Linux release Rust/PG14는 새 `own_comments` 집중 검사 **1개 통과 (0.74초)**.
- Windows Dioxus fullstack 개발 빌드 **55.85초**, `127.0.0.1:12239`에서 실행.
  Playwright의 실제 PG/HTTP/브라우저 검사: 빈 목록 **13개**, 공개 서버 개인 기록 **28개**,
  강제 숨김 서버 개인 기록 **29개**, 각각 pageerror 0개. 반복되는 공통 assertion을 포함한
  실행별 숫자다. 비로그인 API 401/SSR 로그인 안내, no-store/Vary Cookie, 내 계정 진입,
  서버 상세의 기존 댓글 UI 이동, 페이지 넘김, 로그아웃 뒤 재조회 시 글 제거,
  1440/390/320px·일반 텍스트 이스케이프를 확인했다. API 응답 가로채기 모의는 없다.
- 가상 장문 생성의 첫 Windows psql 호출은 인코딩 오류로 전체 요청이 거절됐다.
  회원/서버 0개를 확인하고 ASCII SQL의 `chr(44032)`로 생성하도록 수정 후 재실행했다.
- `output/playwright/own-comments-{1440,390,320}.png`; 390px 화면을 직접 확인했다.
  기존 membership/댓글 스타일과 Playwright 스킬의 브라우저 검증 흐름을 사용했다.
  검사 뒤 만든 회원·서버·댓글 31개를 정리했다. dev/test 회원·서버·댓글 및
  임시 snapshot/rehearsal DB 0개 확인. 실제 회원 압축파일은 읽지 않았다.
- 이번 변경에 대해 Linux 전체 suite 또는 새 최종 배포물 수명주기 검증을 주장하지 않는다.
  Linux `bundle-v7`은 여전히 사진 갱신까지의 배포물이며 현재 소스의 추가 화면과 구분한다.
  `cargo fmt --check`, `git diff --check` 통과.

## 2026-09-14 서버 상세 응답 이력

- Phoenix 서버 상세의 최신 48회 범위를 새 DB 조회/API/SSR 화면에 연결했다.
  [조회 계약](server-discovery.md)의 숨김·빈 기록·2초 기준·KST·정렬 규칙을 따른다.
  Cargo.toml/lock 해시는 이전 사진 갱신 완료 시점과 동일하며 migration은 계속 **22개**다.
- 최초 집중 검사 **3개 통과 (0.19초)**. 합성 Phoenix import 후 조회 검사를 추가하면서
  `Vec.first()`와 Diesel `QueryDsl.first()`의 이름 충돌로 Linux test 컴파일이 실패했다.
  slice 메서드를 명시한 뒤 Windows PG17 **4개 통과 (8.61초)**,
  Linux PG14 release Rust/PG **4개 통과 (2.73초)**, 실패·무시 0개로 재검증했다.
  280개의 보존 기록 중 최신 48개 조회, 빈/종료/숨김/강제 숨김/없는 서버, 동률 순서,
  한국 시각, 내부 오류/작업 식별자 비공개를 검사한다. 실제 운영 백업을 읽은 것은 아니다.
- Windows Dioxus fullstack 개발 빌드 **49.20초** 통과, `127.0.0.1:12239`에서 실행.
  `check-health-history-browser.js`는 실제 PG 가상 서버의 빈 기록 **10개**,
  64개 중 최신 48개 표시 **37개** 통과했다. 공통 assertion을 포함한 두 실행의 숫자이며
  서로 다른 47개 기능을 뜻하지 않는다. 1440/390/320px, 키보드 펼침/접힘/내부 스크롤,
  펼친 표의 페이지 가로 넘침 없음, API/SSR 표시를 확인했고 pageerror는 0개였다.
- 별도 실제 HTTP 검사에서 가상 서버를 숨김/강제 숨김으로 변경하자 비로그인 API/상세 페이지는
  모두 404이며 이력 markup은 없었다. 이때 열려 있던 페이지의 댓글 재조회도 의도대로 404였다.
  개발 브라우저의 이 두 네트워크 오류를 런타임 예외나 정상 경로 콘솔 오류로 혼동하지 않는다.
- 스크린샷 `output/playwright/health-history-{1440,390,320}.png`와
  `health-history-expanded-320.png`; 390px 막대와 320px 펼친 표를 직접 확인했다.
  Playwright 스킬에 따라 기본 HTML 조작·실제 브라우저를 검사했고 별도 시각화 라이브러리는 없다.
- 이번 변경에서 전체 회원/워커 회귀 suite 및 최종 배포물 수명주기를 다시 돌린 것은 아니다.
  아래 사진 갱신 시점의 전체 검증과 구분한다. Linux `--rust-only`는 최신 소스의 test 실행이며,
  기존 `bundle-v7`을 새 응답 이력 포함 배포물로 갱신했다고 주장하지 않는다.
- 검사 후 CLI 브라우저를 닫고 생성한 회원/서버/64개 기록만 정리했다. Windows dev/test의
  서버·응답 기록은 0개, dev 회원 0개, 임시 snapshot/rehearsal DB 0개를 확인했다.
  Linux 검사도 소유한 합성 PG를 정리했으며 실제 백업/회원 자료를 읽거나 삭제하지 않았다.
  `cargo fmt --check`, `git diff --check` 통과.

## 2026-09-14 회원 사진·커스텀 이모지 갱신

- [구현 계약](profile-media-refresh.md): 공개 인증 완료 후 자동 수집, `/account`에서 출처 선택과
  수동 가져오기, 실패 보존, 본인 전용 이미지, 해제/탈퇴 정리를 연결했다. AP 프로필 해석과
  crawler HTTP 경계, 공통 `image` decoder 및 OpenDAL publication을 사용한다. 내장 migration **22개**.
- 최초 집중 검사 **5개 통과 (0.78초)**. 이후 AP parser 검사와 공개 인증 verifier의 profile 정보 검사를 보강했다.
- 최초 Windows 전수는 **186 통과 / 26 실패 (153.11초)**였다. 새 runtime 테이블을 importer의
  대상 테이블 목록에 넣지 않아 재이관 검증이 거절됐다. 목록과 quarantine live-write 검사를 연결하고,
  프로필 상태가 있는 사본을 재이관으로 지우지 않는 회귀 검사를 추가했다.
- 수정 후 전체 Windows PG17 **213개 통과, 실패·무시 0개 (213.18초)**,
  Linux PG14 **214개 통과, 실패·무시 0개 (167.64초)**. 실행 뒤 변경은 해당 회귀 테스트의
  줄바꿈 형식 정리와 문서뿐이다. `cargo fmt --check`도 재확인했다.
- Windows Dioxus fullstack release **114.72초**, Linux release **57.76초** 빌드 통과.
  Linux는 기존 캐시의 공식 dx asset 처리 후 `<cache>/fediversekr2/bundle-v7`로 검증했다.
  두 배포물의 실제 수명주기 검사는 **Windows 89개 / Linux 81개 (53.52초)** 통과,
  공개 파일 34개다. 이전 87/79개에서 늘어난 2개는 공개 파일 바이트 검사이지 별도 기능이 아니다.
- `scripts/check-profile-media-browser.js`: 개발 서버와 최종 Windows release 모두 **23개 통과,
  pageerror 0개**. 실제 HTTP/PG에서 익명/외부 Origin/잘못된 계정 거절, 1분 제한, 존재하지 않는
  가상 AP 출처의 502와 기존 파일 보존, 이름/비공개 연동 유지, 1440/390/320px와 실제 이미지
  디코딩을 확인했다. 의도적인 502/429/401 응답은 오류 경로 검사이며 콘솔 오류 0개라고 주장하지 않는다.
- 별도 release UI 검사: 고정 가상 회원의 PG 요청 상태를 pending으로 바꾸자 안내/비활성 버튼이
  표시됐고, 같은 행을 완료·revision+1로 바꾸자 재탐색 없이 이미지 URL 갱신, 버튼 활성,
  pending 안내 제거, 출처 표시를 확인했다. 실제 외부 AP 성공을 흉내 낸 API 가로채기는 없다.
- 첫 release 실행은 이미지 fixture의 `legacy_members`가 남아 있어 시작 gate가 거절했다.
  gate를 해제하지 않고 fixture를 정리한 빈 DB로 시작한 뒤 가상 자료를 재생성해 검사했다.
- 실패 전수에서 남은 격리 DB **29개**는 고정 fixture 회원 UUID, 테스트 역할, 해당 실행의 생성
  시각(16:30~16:32), 활성 연결 없음을 전부 대조한 뒤 삭제했다. 가상 브라우저 회원/이미지도
  제공된 fixture 정리 경로로 삭제했다. 생성 자료는 `fixtures.sql`/Seed로 재현 가능하다.
  최종 dev/test DB의 회원·live profile·legacy member·사이트·제품·stored files·삭제 예약은 모두 0개,
  `.local/media/objects`도 0개. 실제 회원 ZIP, Phoenix 운영 DB, 원본 파일 export는 읽거나 바꾸지 않았다.
- 최종 개발 서버는 새 코드로 재빌드(18.15초)해 `127.0.0.1:12239`에 유지했다.
  health/readiness/계정 페이지 및 익명 이미지·metadata·갱신 거절 HTTP 6개를 마지막으로 확인했다.
- 최종 프로덕션 컷오버 증거는 아니다. 외부 AP 서버 성공 통신/실제 백업 이관·운영 TLS/사람의
  콘텐츠 검토는 별도로 남아 있다. [현재 준비도](../cutover-readiness.md).

## 2026-09-14 제품 로고 관리 마무리

- [로고 관리](catalog-logos.md)를 기존 관리자 화면/공통 이미지 컴포넌트에 연결했다.
  파일 선택만으로 공개하지 않으며 실제 업로드·교체·삭제와 공통 revision/비공개 사유 이력을
  처리한다. OpenDAL 실제 IO, image 래스터 디코딩, 기존 파일 publication 재사용, OS/PG 잠금과
  실패 시 정리 예약을 사용한다. 내장 migration **21개**.
- Windows PG17 전체 **206개 통과, 실패·무시 0개 (186.75초)**,
  Linux PG14 전체 **207개 통과, 실패·무시 0개 (128.34초)**.
  새 검사는 실제 디코딩/원본 보존, 잘못된 파일/크기/픽셀 제한, 업로드/교체/같은 바이트/제거,
  공유 해시 보존, 권한/충돌, 바이트 작성 후 DB 롤백과 고아 파일 정리, 프로세스 간 OS 잠금과
  IO 대기 future 취소 시 소유권 유지다. 기존 회원/이관/파일/수집 검사를 함께 재통과했다.
- 초기에 Dioxus 문자열 props의 모호한 `.into()`를 수정했다. 첫 전체 검사에서 이전
  migration 수 **20개**를 고정한 assertion이 실패해 실제 새 수인 21로 바꾸고 전체를 재실행했다.
  Linux 첫 시도는 새 image 전이 패키지가 offline cache에 없어 실행 전 실패했다.
  감사된 선택 의존성을 locked 상태로 받은 뒤 집중 7개 및 위 전체 검사를 실행했다.
- 기존 실제 카탈로그 관리자 브라우저 **49개**, 새 로고 브라우저 **24개**가 개발 서버에서 통과했다.
  관리자 변경/잠금/충돌/분류 기능과 로고의 공개 전 선택, 실제 파일 전송/표시, 교체/삭제,
  입력 보존, 1440/390/320px 레이아웃과 런타임 예외 0개를 확인했다.
- 이 과정에서 기존 제품/종류 식별자 두 input의 HTML pattern이 최신 브라우저 `v` 문법에
  맞지 않음을 콘솔에서 발견했다. 하이픈을 escape하고 두 입력의 실제 유효/무효 검사를 추가했다.
  근거: [HTML pattern 문법](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Attributes/pattern).
  위 전체 Rust 검사 뒤의 소스 수정은 이 두 HTML 속성뿐이다. 전체 Rust를 또 실행했다고
  주장하지 않으며, 이후 양 플랫폼 release와 아래 최종 브라우저로 확인했다.
- 최종 Windows fullstack release **126.19초**, Linux release **1분 35초**.
  공식 Dioxus 자산 처리한 최신 Linux 묶음은 `bundle-v6`이다. 실제 합성 활성화/HTTP/파일/워커/
  재시작 수명주기 **Windows 87개, Linux 79개 (58.64초)** 통과. 두 묶음의 공개 파일은 32개다.
  이전 85/77 대비 늘어난 2개는 파일 수이며 새 기능 검사 수로 세지 않는다.
- 최종 release의 실제 로고/입력 검증 브라우저 **28개 통과**, 런타임 예외 0개.
  의도적인 충돌 요청의 HTTP 409는 콘솔에 남는다. 초과 본문 probe는 개발 proxy에서 413,
  standalone 서버에서 송신 도중 연결 종료(ECONNRESET)일 수도 있다. 이 특정 거절과 다음
  health 성공만 허용하도록 검사를 명시했고, 다른 네트워크 오류를 성공 처리하지 않았다.
  성공 업로드·교체는 매번 HTTP 200과 실제 공개 바이트를 따로 확인한다.
- Playwright 스킬의 실제 브라우저/스크린샷 절차로 `output/playwright/catalog-logo-{1440,390,320}.png`
  를 확인했다. test fixture 계정/제품/종류/파일과 검사 전용 브라우저/12241 서버는 제거·종료했다.
  개발/테스트 DB의 회원·사이트·제품·종류·파일 매핑·삭제 예약은 모두 0개로 확인했다.
- [선택 의존성 감사](dependency-licenses.md): Windows 454/Linux 451/Wasm 286개,
  GPL 계열/라이선스 선언 누락/SQLx/bcrypt 0개. 새 앱 컨테이너/브로커는 없다.
  Cargo.toml/lock 해시와 포맷 검사를 확인했고 기존 unused/dead-code 경고는 남아 있다.
  실제 ZIP/운영 자료/실 AP/TLS/트래픽 전환과 AP 이미지 재수집까지 완료한 것이 아니다.
  이번 묶음은 로고 관리로 마무리하며 새로운 범용 기능을 시작하지 않았다. 새 커밋은 없다.
- 최종 DB 연결 개발 서버 12239를 **31.91초**에 다시 빌드해 유지했다. 공개 HTTP **16개**
  재통과, 테스트 파일 디렉터리는 비어 있다. 다른 사용자의 브라우저나 Phoenix는 건드리지 않았다.

## 2026-09-14 소프트웨어 전체 검색·페이지 이동

- MIT Phoenix의 카탈로그 조회와 현 UI를 대조해, 처음 받은 500개 안에서만 검색하던
  제한을 확인했다. 전체 DB 검색·24개 제품/12개 종류 페이지·독립적인 종류 이름 조회를
  연결했다. URL 처리는 기존 `url`, SQL은 Diesel adapter 안에만 있다. [조회 계약](catalog-browsing.md).
- 카탈로그 변경 후 Windows PG17 전체 **197개 통과, 실패·무시 0개 (189.22초)**,
  Linux PG14 전체 **198개 통과, 실패·무시 0개 (116.15초)**. 내장 migration은 20개로 동일하다.
  새 2개는 URL 계약과 513제품/205종류의 대량 조회·안정 정렬·옛 분류 대체·공개 필드 검사다.
- 실제 PG의 별도 27제품/14종류 fixture로 브라우저 **44개 통과, 런타임 예외 0개**.
  1440/390/320px, 검색/종류/페이지/뒤로 가기/새로고침/SPA, 한글 `%_+`, API/SSR 400,
  JavaScript 없는 SSR과 다음 페이지 이동을 확인했다. 세 폭의 실제 캡처도 확인했다.
- 같은 release의 DB 없는 별도 검토 서버(12241)에서 기존 콘텐츠 브라우저 **483개**도
  전부 재통과했다. 6개 스크롤 설명·메뉴 노출·소프트웨어→서버·모바일/SSR·모션 감소·SPA
  관찰자 정리, 런타임 예외 0개다. 기존 개발 서버(12239)의 DB 설정은 바꾸지 않았다.
- 다만 위 483개 검사는 예시 댓글이 실제로 나타나는지 보지 않았다. 콘솔의 댓글 권한 API
  404 3건을 추가 조사해, 검토용 `.example` 도메인을 실제 서버의 주소 검증에서 거절하는
  기존 순서 문제를 확인했다. 예시 댓글·답글·권한 읽기를 DB 미설정 분기 안에서 처리하도록
  수정했다. 설정된 DB와 모든 쓰기 검증은 유지한다. 후속 댓글/PG 집중 **8개 (9.54초)**
  통과; 이 작은 후속 뒤에 197/198개 전체 Rust 검사를 다시 돌렸다고 주장하지 않는다.
- 예시 댓글·답글 HTTP, 알 수 없는 예시 주소 거절, 세 폭의 댓글 실제 표시 검사를 추가하고
  마지막 release에서 콘텐츠 브라우저 **490개 통과, 런타임 예외 0개**를 확인했다.
  검사 전용 브라우저/12241 서버는 종료하고 최신 DB 연결 개발 서버 12239는 유지했다.
- 첫 컴파일은 RSX 문자열 뒤 `Link` 앞 공백 누락으로 실패해 수정했다. 첫 브라우저 검사도
  CLI 실행 영역에 `URL` 전역이 없어 중단됐고, 실제 브라우저의 주소를 읽도록 검사 코드를
  수정해 전부 재실행했다. 개발 서버 재빌드 중의 과거 WebSocket 접속 실패는 별도이며,
  성공 검사는 안정된 최신 빌드에서 실행했다. 오류를 무시해 통과 처리하지 않았다.
- Windows Dioxus fullstack release **109.81초**, 실제 합성 활성화/HTTP/파일/워커 수명주기
  **85개** 통과. Linux release ELF build **1분 32초**, 같은 릴리스 웹 파일과 공식 Dioxus
  자산 처리 후 실제 수명주기 **77개 (59.09초)** 통과. 이 시점의 Linux 묶음은 v4다.
  두 배포물의 정적 파일은 30개다. 앞 단계보다 늘어난 3개는 incremental 자산 파일이며
  기능 검사 증가로 세지 않는다. 기존 82/74개 검사 내용도 모두 재통과했다.
- 예시 댓글 후속도 Windows release **108.17초**, Linux release **1분 37초**로 다시 빌드하고
  실제 배포물 검사 **85/77개 (Linux 58.49초)** 재통과했다. 최신 Linux 묶음은 v5, 파일은 30개다.
- 개발 서버 첫 fullstack build **55.80초**, 공개 HTTP **11개** 통과. 마지막 예시 수정도
  **29.29초**에 재빌드하고 보강한 공개 HTTP **16개** 통과했다. 실제 DB에서는 `.example`
  댓글을 계속 404로 거절함도 포함한다. 합성 fixture 제거 후
  개발/테스트의 회원·서버·삭제 예약·소프트웨어·종류는 모두 0개다. 생성 이관 DB/파일과
  Linux 합성 PG/서버도 검사 스크립트가 정리했다. 원본 ZIP/실회원/실 AP/TLS/트래픽 전환은
  하지 않았다. 공개 목록 성능의 운영 규모 측정과 제품 본문의 사람 검토도 별도다.
- Cargo.toml/lock 해시는 앞 감사와 동일하다. 새 라이브러리·GPL 계열 소스·컨테이너는 없다.
  포맷/공백 검사 통과, 기존 unused/dead-code 경고는 남아 있다. 새 커밋은 만들지 않았다.

## 2026-09-14 OpenDAL 탈퇴 파일 정리

- Phoenix MIT `Accounts.withdraw_user/1`과 Storage 삭제 경로를 대조했다. 누락했던
  아바타·이모지 참조 정리와 PG 삭제 예약을 탈퇴 트랜잭션에 통합하고, 같은 프로세스의
  OpenDAL Fs 워커를 연결했다. 내장 migration **20개**. [처리·백업 경계](media-cleanup.md).
- 최종 Windows PG17 전체 **195개 통과, 실패·무시 0개 (175.17초)**,
  Linux PG14 전체 **196개 통과, 실패·무시 0개 (118.22초)**.
  Linux의 추가 1개는 파일/디렉터리 symlink 삭제 거절 검사다.
- 새 검사는 공유 키/같은 내용의 다른 키, 동시 탈퇴, 롤백, 삭제 실패 backoff와 새 연결 재시도,
  파일 삭제 후 DB 확인 전 취소, 매핑 게시 경합, 잘못된 해시·길이·변조·누락 경로를 다룬다.
  미완료 삭제가 있는 격리 DB 재이관 거절과, 활성화 DB의 저장소 필수 조건도 확인했다.
- 첫 집중 컴파일은 테스트 주입 closure의 `Send` 제약 누락으로 실패했다. Diesel async
  트랜잭션 계약에 맞게 수정했고 집중 6개 및 두 플랫폼 전체 검사를 다시 통과했다.
- Windows Dioxus fullstack release 재빌드 **100.85초**, 실제 명령/HTTP 수명주기 **82개**
  통과(정적 파일 **27개** 포함). 합성 Phoenix import/assets/승인 후 실제 HTTP 탈퇴,
  같은 프로세스의 아바타 바이트 삭제, 공유 로고 보존, 원본 export/이관 inventory 보존,
  삭제된 파일 없이 재시작과 승인 재시도의 live 쓰기 보존을 확인했다.
- Linux release ELF 재빌드 **59.19초**, 같은 릴리스의 웹 파일과 공식 Dioxus 자산 처리 후
  실제 수명주기 **74개 통과 (58.49초)**. 실제 HTTP 본인 이미지→탈퇴→OpenDAL 삭제와
  PG 확인, SIGTERM/요청 drain/종료 상한 등 기존 검사도 통과했다. 최종 묶음은 v3이다.
  공개 파일 수 증가 2개는 incremental 자산 해시 파일이고 기능 증가로 세지 않는다.
- 위 실행은 모두 합성 자료다. 실제 ZIP/회원 DB·원격 AP·TLS·운영 활성화·브라우저 hydration은
  이번 검사의 대상이 아니다. 생성한 SQL/파일 fixture와 Linux PG/테스트 서버는 정리했다.
- Cargo.toml/lock 해시는 앞 라이브러리 감사와 동일하다. 새 의존성·GPL 계열 소스·별도
  컨테이너/브로커는 없다. 기존 unused/dead-code 경고는 남아 있다.
- 개발 서버도 최신 코드로 재시작(실제 build 34.03초), 공개 HTTP 11개 재통과.
  개발/테스트 회원·서버·삭제 예약과 생성 이관 DB는 0개다. Linux 이전 검증 묶음 v2만
  경로 확인 후 제거했고 최신 v3/build cache는 보존했다. 테스트 fixture는 재생성 가능하다.
  포맷·공백 검사 통과. 이번 단계에서도 새 커밋은 만들지 않았다.

## 2026-09-14 Linux 실제 실행·종료와 경합 수정

- Linux WSL Ubuntu 22.04 / Rust 1.94.1 / 합성 PG14에서 최종 전체 서버 테스트
  **185개 통과, 실패·무시 0개 (114.88초)**. Windows / Rust 1.97.0 / 합성 PG17도
  **185개 통과, 실패·무시 0개 (195.64초)**. 기존 회원 보호를 유지하는 회귀 검사 1개가 늘었다.
- 첫 Linux 전수는 182/184였다. 수동 수집의 오너/관리자 경합에서 실제 `deadlock detected`를
  재현했고, 한 테스트가 남긴 작업 때문에 다음 큐 검사도 실패했다. 격리 DB를 매번 폐기해
  재현했으며, SQL template의 서버 행 `FOR UPDATE`와 다른 트랜잭션 `COMMIT` 대기를 확인했다.
- 회원 UUID가 바뀌지 않는 공통 guard를 `FOR NO KEY UPDATE`로 조정했다. 관리자 변경의
  지연된 owner FK 확인은 허용하되 회원 변경/삭제 직렬화는 유지한다. 초기 집중 경합 2개가
  0.86초에 통과했고, 새 회귀는 실제 deferred FK commit 성공과 동시 회원 변경 차단을 검사한다.
  재시도/오류 무시로 테스트를 우회하지 않았다. [잠금 계약](member-lifecycle.md).
- release 시작은 웹 묶음/router/port bind를 확인한 뒤 DB·워커를 초기화한다. 종료는 HTTP
  신규 수락을 먼저 막고 요청/워커를 동시에 정리하며, HTTP 30초/워커 25초 상한을 둔다.
- 최종 Linux ELF 배포물의 실제 프로세스 검사 **67개 통과 (65.61초)**. 빠진 웹 파일과
  점유 포트에서 migration 전 실패, 실제 SSR 자산 경로와 정적 파일 **25개** 바이트 일치,
  실제 PG 예약/수집/유지보수·만료 임대 회수·키 보존·익명 이미지 거절을 확인했다.
- 실제 SIGTERM에서 느린 POST 본문을 받는 동안 새 listener가 먼저 닫히고 408을 반환한 뒤
  정상 종료함을 확인했다. 별도 테스트 사본에만 16 MiB 가상 파일을 넣어 매우 느린 reader를
  만들고, 30초 종료 상한과 명시적인 실패 종료도 확인했다. 이 파일은 배포물에 들어가지 않는다.
- Linux의 일반 Cargo build만으로는 자산 주소가 placeholder였다. 이를 실제 SSR 검사에서
  발견하고 공식 `dx tools assets`로 ELF와 자산을 처리했다. Windows에서 만든 동일 릴리스의
  웹 WASM과 Linux 서버 조합이며, Linux에서 fullstack CLI 전체 build를 했다는 주장은 아니다.
- Linux 초기 rustc 1.94.1의 긴 진단 출력이 비정상 terminal 폭에서 ICE를 일으켰다.
  `COLUMNS=160`과 짧은 진단 형식으로 컴파일했다. 원래의 unused/dead-code 경고는 남아 있다.
- 최신 Windows Dioxus fullstack release 재빌드(104.58초), 실제 활성화 수명주기 **73개**
  재통과(정적 파일 25개 포함). Linux 최종 release build는 55.81초였다. 처음 Linux build는 2분 45초.
  추가 웹 파일 수는 incremental Dioxus 산출물의 해시 파일 증가이며 기능 테스트가 2개 늘었다는 뜻은 아니다.
- manifest/lock 및 내장 migration 19개는 그대로다. 앱 의존성/컨테이너/GPL 계열 소스를 추가하지
  않았다. 도구용 호환 런타임은 개인 캐시에서만 사용했다. [배포 구성·재현·한계](release-lifecycle.md).
- 위 검사는 전부 합성 자료다. 실제 ZIP·운영 DB·외부 AP·TLS/프록시·브라우저 hydration·운영
  트래픽 전환 증거와는 구분한다. 최초 Linux 실패 및 후속 성공 모두 테스트 클러스터/프로세스를 정리했다.
- 마지막 정리: Linux의 테스트 PG listener/서버/임시 클러스터 없음, Windows 테스트 회원·서버·승인
  및 생성 이관 DB 0개. 이전 Linux 테스트 묶음 v1은 삭제하고 재생성 가능한 build cache와
  최종 검증 묶음 v2만 남겼다. 개발 서버는 최신 코드로 재시작(빌드 32.20초)했고 공개 HTTP
  **11개** 재통과, 개발 회원/서버/승인 0개, 포맷·공백 검사 통과. 이번에도 새 커밋은 만들지 않았다.

## 2026-09-14 실제 release 배포물의 활성화 수명주기

- `dx build --web --fullstack true --release`의 서버·클라이언트·정적 파일 묶음 빌드 통과
  (247.69초). 일반 Cargo release build도 통과했지만, 단독 바이너리는 Dioxus의 `public/`
  디렉터리가 없어 웹 서버로 실행되지 않았다. 개발 웹 파일을 가져다 붙이지 않고 같은
  Dioxus release 빌드의 서버와 웹 파일로 전체 검사를 실행했다.
- `scripts/check-activation-lifecycle.ps1` 실제 프로세스 검사 **71개를 두 번 통과**
  (마지막 실행 21.44초, 매번 새로운 합성 자료).
  합성 Phoenix DB → 실제 import/assets CLI → 사본 이름 변경 → 검증/승인 CLI →
  HTTP/SSR/같은 프로세스 워커 → 강제 종료/재시작을 연결했다. DB adapter 호출만의 증거가 아니다.
- 승인 전 시작 거절, apply 확인 문구 요구, 승인 후 origin 불일치·저장소 누락/변조 거절,
  네 파일 참조를 한 내용 파일로 보존, 원래 서명 키/회원 UUID 유지, 숨김 서버 비노출,
  본인 이미지 접근/세션 폐기 후 거절과 private SSR의 비밀 비노출을 확인했다.
- 웹 파일 **23개**의 실제 HTTP 응답과 번들 SHA-256을 전부 대조했다.
  랜딩 SSR과 hydration 스크립트 삽입은 확인했지만 브라우저에서 실행한 증거는 아니다.
- 실제 scheduler/worker가 PG 작업·관측 결과를 기록하고, 두 유지보수 일정을 실행했다.
  직접 만든 프로세스만 강제 종료한 뒤 합성 만료 임대를 넣고 재시작해 2번째 시도에서 회수·완료했다.
  실제 진행 중 수집을 죽이거나 정상 종료 신호를 검증한 것은 아니다.
- 워커 주소는 네트워크 전 검증에서 거절되게 설정했다. 원격 AP 성공·NodeInfo 성공·TLS는
  이 검사에 포함하지 않았다. 회원 HTTP는 합성 DB 세션이며 공개 인증글 실인증이 아니다.
- 최초 검사 스크립트의 한글 SQL 인수는 Windows 문자 인코딩 문제로 실패했다.
  UTF-8 표준입력으로 바꿔 재현했고, SQL 원문/키를 명령행에 싣지 않도록 했다.
  테스트 DB/OID·임시 경로를 매 실행에서 검증해 정리했다. 운영 ZIP/DB/트래픽은 접근하지 않았다.
- Rust 애플리케이션과 Cargo manifest/lock은 아래 **184개** 전수 이후 변경하지 않았다.
  이번 후속은 배포 빌드와 실제 프로세스 검사이며 전수 테스트 재실행으로 세지 않는다.
  [재현 절차와 범위](release-lifecycle.md).
- 마지막 정리 확인: 생성 snapshot/rehearsal/live DB 0개, 테스트 회원/사이트/승인 행 0개,
  수명주기 임시 디렉터리/소유 release 서버 프로세스 0개. 기존 개발 서버의 공개 HTTP
  **11개**도 재통과했다. 개발 서버는 그대로 두었고 Rust/라이브러리 변경이나 새 커밋은 없다.
- Linux 환경 확인은 별도 읽기 전용 점검이다. WSL Rust 1.94.1은 있으나 고정된
  `dioxus-primitives` git checkout이 없어 offline metadata가 실패했다. Linux build/run 통과로
  세지 않으며 새 런타임·컨테이너를 설치하거나 운영 DB를 연결하지 않았다.

## 2026-09-14 검토 사본의 명시적 활성화

- 내장 migration **19개**. `--legacy-activate` 기본 검사/`--apply` 및 확인 문구,
  원본/보존 행·projection·서명 키·전체 파일 재대조와 별도 승인 행을 연결했다.
  새 라이브러리/컨테이너는 없으며 OpenDAL의 실제 바이트 검증을 재사용한다.
- 초기 집중 **6개** 통과(21.87초), 추가 잠금/빈 DB 검사를 포함한 최종 전체 서버/격리 PG
  **184개 통과, 실패·무시 0개 (155.94초)**. 합성 스냅샷을 이름 변경한 별도 사본에만 적용했다.
- 검사 후 원본/target projection/파일 변경 거절, 누락 inventory와 명시적 빈 inventory 구분,
  같은 요청의 동시 적용과 commit 후 재시도, live 수정 보존, 다른 origin 거절,
  snapshot/rehearsal/빈 live DB 시작 거절, ledger 삭제 우회 거절을 확인했다.
- 실제 `pg_locks`에서 잠금 획득을 관찰하며 마지막 승인 테이블을 붙잡은 상태로,
  일반 UPDATE가 검증 중에 끼어들지 못하고 승인 commit 전에는 runtime gate가 닫힘을 검사했다.
  runtime 시작 준비 함수는 missing/wrong store·변조된 파일 거절 및 정상 전체 파일을 확인했다.
- 실제 debug 실행 파일의 오프라인 분기 검사 **5개** 통과. asset 2개, activation 설정 없음 2개,
  올바른 형태의 URL이 있어도 apply 확인 문구 없음 1개다. 오류는 고정 문구이며
  접속 문자열/개인정보를 출력하거나 HTTP·워커로 진입하지 않았다.
  이 시점의 성공 적용은 위 PG adapter 테스트다. 실제 프로세스 수명주기는 위 후속 기록을 따른다.
- debug server build, release server check(27.03초), wasm32 web check(9.85초) 통과.
  기존 unused/dead-code 경고는 남았고 Cargo.toml/Cargo.lock은 이전 라이브러리 감사와 동일하다.
- 최신 dx fullstack 개발 서버 재시작, 개발 DB migration 19개/승인 0개/회원 0개,
  공개 HTTP **11개** 재통과와 포맷/공백 검사를 확인했다. 기존 회원·미디어 브라우저 전수는
  이 시작 gate 변경에서 다시 실행하지 않았고 아래 이전 실행과 구분한다. 테스트 브라우저는 종료했다.
- 검사 종료 후 테스트 DB 회원/서버/승인 행과 생성 snapshot/rehearsal/live DB는 0개였다.
  합성 자료만 생성/정리했으며 실제 운영 ZIP·원본 DB·트래픽은 건드리지 않았다.
- [운영 전제·실행 순서·승인의 한계](activation.md). 실자료 검증,
  Linux 번들·TLS/프록시·원격 AP·운영자 검토까지 끝났다고 해석하지 않는다.

## 2026-09-14 이미지 조회 및 라이브러리 교체

- 내장 migration **18개**. `stored_files` 매핑과 공개 로고/서버 아이콘,
  본인 전용 아바타/이모지 조회를 연결했다. 숨김·세션 폐기·참조 변경을 IO 전후 재검사한다.
  라이브러리 전환 전 전체 175개가 통과했고(124.83초), 최종 OpenDAL/범용 처리 전환 후
  전체 서버/격리 PG **176개 통과, 실패·무시 0개 (134.79초)**다.
- OpenDAL Fs 실제 바이트 읽기/쓰기, 원본 보존·빈 파일·중복·변조·정션 거절을 검사했다.
  첫 전환 전수는 162/175: EOF를 넘는 범위 읽기 실패 11개와 그 잔여 fixture가 전역 큐에
  들어간 연쇄 실패 2개였다. 읽기 전 확인한 정확한 파일 길이로 범위를 바꾸고, 읽기 후
  변경 검사를 유지했다. 가상 서버 7개·회원 13개와 검증된 합성 이관 DB 10개만 정리했다.
  실제 회원 데이터는 아니며 코드로 재생성할 수 있다.
- 인증 digest 비교는 subtle, Cookie 해석/생성은 cookie, percent decoding은 percent-encoding,
  SVG 이벤트 해석은 quick-xml로 교체했다. 잘못된 URL escape, 중복 쿠키/토큰,
  XML 다중/미완성 루트·깊이/DTD 거절과 응답 CSP의 역할을 분리해 검사했다.
- Playwright CLI `check-media-browser.js` **67개 통과**, 런타임 예외 0개, SVG의 금지된
  외부 요청 0개. 실제 로컬 파일의 응답·HEAD·바이트 길이/보안 헤더, 공개/본인 접근,
  익명/cross-site/로그아웃 거절, SSR 비밀 비노출, SVG 스크립트·외부 이미지 차단,
  디코딩 실패 대체 UI를 확인했다. 원격 성공 응답을 모의하지 않았다.
- 첫 56개 브라우저 검사 후 캡처에서 발견한 아바타/로고 아래로 제목이 흐르는 문제를 수정했고,
  최종 67개에는 옆 배치 검사도 포함했다. 1440/390/320px의 계정·목록·제품·사이트
  12장 캡처를 생성했다. 계정 320px/제품 320px/목록 1440px는 직접 시각 검토했다.
  `output/playwright/media-{account,catalog,software,site}-{1440,390,320}.png`.
- 기존 회원 브라우저 **49개 재통과**, 런타임 예외 0개. 실제 가상 회원/PG로 쿠키 설정·회전,
  자체 로그인·암호 변경·연결 해제·탈퇴를 확인했다. 원격 AP 실인증 증거와는 구분한다.
- 기존 서버 아이콘 브라우저 **18개 재통과**, 런타임 예외 0개. PG 캐시 우선 경로의
  실제 디코딩·오류 대체·숨김도 공통 이미지 컴포넌트에서 유지된다.
- 새 dx 서버/fullstack 빌드와 hydration, release server 검사(80초), wasm32 web 검사(20.08초),
  공개 HTTP 11개, 포맷/공백 검사가 통과했다. 기존 unused/dead-code 경고는 남았다.
- [선택된 의존성 라이선스](dependency-licenses.md): Windows 438 / Linux 435 / Wasm 286개,
  GPL 계열/선언 누락/SQLx/bcrypt 0개. [라이브러리 도입과 남긴 정책](library-boundaries.md).
- [이미지 조회 계약과 미완료](media-serving.md). 실제 암호화 운영 ZIP은 열거나 복원하지 않았다.
  S3 backend·새 업로드·운영 활성화·Linux 런타임은 이 검증에 포함되지 않는다.

## 2026-09-14 기존 파일 보존 경로

- 끊겨 있던 `legacy::assets` DB 모듈을 연결해 빌드를 복구했다. 내장 migration 017은
  비공개 파일 색인 두 테이블만 추가하며, 이미 보존된 색인이 있으면 downgrade로 지우지 않는다.
- 최종 전체 서버/격리 PG **165개 통과, 실패·무시 0개 (118.79초)**. 파일 단위 5개와
  DB 통합 7개가 새 검사다. 중간 164개 전수도 두 번 통과했다 (112.55초 / 118.17초).
  플랫폼별 파일명 검사는 Windows의 충돌 거절을 실행했으며 Linux 분기의 실행 증거는 아니다.
- 새 검사는 원본 이관 지문 대조, 기본 검증/적용/동시 실행/재실행/원본 재이관,
  283개 키의 페이지 경계, 같은 내용의 여러 키·빈 참조, 누락 후 DB 롤백/파일 재사용,
  원본·출력 파일·DB 해시/합계/참조 변조 및 완료 파일 삭제 거절을 다룬다.
- Windows 실제 정션으로 원본과 출력 경로의 이탈을 거절했고 외부 파일이 유지됨을 확인했다.
  빈 파일·능동 SVG·임의 바이트도 그대로 보존하되 공개 제공하지 않는 계약을 검사한다.
  크기 초과·빈/긴 키·경로 이탈·Windows 예약 이름/ADS를 묵살하지 않는다.
- 이전 합성 fixture의 이모지 URL 객체와 로고 prefix를 Phoenix의 실제 저장 계약에 맞췄다.
  회원 이모지 값을 포함하여 해당 fixture의 참조 수는 3개가 아닌 4개다.
- `check-legacy-assets-command.ps1`의 실제 실행 파일 오류 경로 **2개** 통과:
  필요한 입력이 없으면 DB/웹/워커 초기화 전에 종료하며 설정·회원 정보는 출력하지 않는다.
  파일 적용 성공은 위 실제 격리 PG/파일 테스트의 DB adapter 호출로 검증했다.
- release server / wasm32 web 검사 통과. Linux 런타임/실제 S3 export 검증은 아니다.
  마지막 테스트 분리 때 생긴 테스트 파일 괄호 오류는 포맷 검사에서 발견해 수정했다.
  기존 unused/dead-code 경고는 남아 있다.
- 공개 HTTP 11개 재통과. 이번 변경은 오프라인 CLI이며 기존 개발 웹 서버를 재시작하거나
  회원/설명 브라우저 전수를 다시 실행하지 않았다. 이전 브라우저 기록과 구분한다.
- Cargo.lock은 `889238F0DEE737E4CA52F8D4C4A099A0ACCD63D526B4FC82F3D0D53A71A00B7C`로 동일하다.
  새 의존성·컨테이너·커밋·운영 변경은 없다. 실제 암호화 ZIP은 열지 않았다.
- 최종 정리 확인: 개발/테스트 DB 회원·연동·서버·소프트웨어·댓글·신고·작업·이관 표식 각각 0개,
  테스트 파일 색인/manifest 및 임시 snapshot/rehearsal DB 0개, 임시 파일 fixture 디렉터리 0개.
  테스트가 만든 자료만 정리했으며 fixture 코드로 재생성 가능하다. 포맷/공백 검사도 통과했다.
- [입력 준비·복구 의미·남은 경계](legacy-assets.md). 운영 파일 제공과 활성화는 아직 없다.

## 2026-09-14 소프트웨어 관리자·종류 관리

- 전체 서버/격리 PG **153개 통과, 실패·무시 0개 (89.58초)**. 집중 DB 검사 5개(0.95초), 입력 규칙 1개, 격리 재이관 보존 1개를 추가했다. 내장 migration 16개.
- 관리자 역할 회수·차단·세션 폐기·15분 재인증, 잠금 중 관리자 수정과 일반 회원 편집/복원 거절, 잠금 해제 후 오래된 폼 거절, 메타데이터 CAS·no-op·baseline·NULL/로고 보존, 분류 연결 유지·중복·이력, 리터럴 검색·24개 페이지·관리자 삭제 후 FK 분리, 재이관 시 이력/분류 상태 보존을 실제 PG에서 확인했다.
- `check-catalog-moderation-browser.js`: 최종 **49개, 런타임 예외 0개**. 실제 HTTP/PG로 잠금→공통 편집기에서 한글 4,000자 수정→공개 이력/잠금 유지, 설정 충돌/사유 보존/명시적 최신 확인, 색상 검증, 종류 등록/수정 충돌/재확인/저장/새로 생성한 종류의 공개 선택지 반영을 확인했다. 익명/Origin/본문 상한/private SSR 포함. 성공 응답 mock은 없다.
- 첫 브라우저 48개 통과 뒤 캡처로 모바일 하위 메뉴가 길게 쌓이는 문제와 이력의 내부 JSON 노출을 확인했다. 메뉴/페이지 이동을 가로 배치하고 이력을 사람이 읽는 값으로 바꿨다. 새 가상 자료로 49개를 다시 통과했고 1440/390/320px 캡처를 확인했다. `output/playwright/catalog-admin-{1440,390,320}.png`, `category-admin-{1440,390,320}.png`.
- 기존 회원 소프트웨어 편집 브라우저 **49개**, 신고 관리자 **40개**, 각각 런타임 예외 0개. 일반 회원의 공통 편집·비교/병합·복원 흐름이 유지된다. 회원 생명주기/설명/사이트 관리자 브라우저 전수는 이번 단위에서 재실행하지 않았다.
- `check-moderation-role.ps1`에 카탈로그 읽기·쓰기·SSR을 추가했다. 실제 같은 바이너리의 역할 회수 뒤 403/no-store, 재부여·중복 부여를 확인했다. 가상 역할만 변경했다.
- 첫 컴파일에서 Dioxus prelude의 Action/History 이름 충돌, static key와 Signal closure 형식 오류를 수정했다. 이후 server check, 위 테스트, 최종 UI 보정 후 release server/wasm32 web check, 실제 dx 빌드·실행을 통과했다. 기존 unused/dead-code 경고는 남는다. 서버 전체 검사는 UI 표시 보정 전 실행이며 그 후 브라우저·양쪽 feature를 재검증했다.
- 공개 HTTP **11개**, 포맷/공백 검사 통과. 새 패키지/컨테이너 없음. Cargo.lock SHA-256 `889238F0DEE737E4CA52F8D4C4A099A0ACCD63D526B4FC82F3D0D53A71A00B7C` 유지.
- 가상 자료 정리 뒤 개발/테스트 DB의 회원·연동·서버·제품·종류·관리 이력·분류 revision·관리 역할·댓글·신고·작업 각각 0개, 임시 snapshot/rehearsal DB 0개를 확인했다. 재생성 가능한 fixture만 삭제했다. 실제 ZIP/운영 DB/원격 계정/Phoenix는 변경하지 않았다.
- 실데이터 이관·Linux 배포·로고 스토리지·공개 이력 제한 열람은 완료 범위가 아니다. [계약과 남은 부분](catalog-moderation.md).

## 2026-09-14 사이트 관리자 후속

- 최종 전체 서버/격리 PG **146개 통과, 실패·무시 0개 (85.00초)**. 사이트 관리 집중 7개와 격리 이관 이력 보존 1개를 추가했고, 15개 내장 migration을 적용했다. 직전 실행들도 146개(86.58/81.98/83.61초)를 통과했다.
- 관리자 역할 회수·차단·세션·재인증, 오너/관리자 동시 저장 CAS, 독립 숨김/종료 플래그, 긴 옛 태그·소개·소유권 보존, 태그 개수/입력 상한, 초대제 NULL, 조회 전용 동작, 필터/정렬/페이지, 수동 요청 경쟁/1시간 공유 제한·원자 이력, 관리자 탈퇴 후 이력 유지와 재이관 거절을 실제 PG에서 확인했다.
- 검토 중 태그 표시 문자열이 같아도 원본 배열이 다를 수 있는 경우를 찾아, 태그 배열 자체로 no-op을 판정하도록 수정하고 회귀를 추가했다. 원본 snapshot은 유지하고 요약에 태그 개수도 표시한다. 수정 후 최종 전체를 재실행했다.
- `check-site-moderation-browser.js`: **55개, 런타임 예외 0개**. 실제 HTTP/PG로 관리자 동선, 상태 변경·숨김 두 방향의 독립성·공개 상세 차단/복구, 태그·초대제 미확인·수동 수집, 동시 변경 409와 초안 보존/재확인, 사유/이력·원본 비노출, 익명/Origin/body/입력 제한, 검색, 1440/390/320px를 확인했다. 성공 응답 mock은 없다. 앞선 51개 검사에서 숨김 순서 비교 4개를 추가했다.
- 실제 브라우저 캡처를 확인해 모바일 관리자 메뉴를 한 줄로 정리했다. 전체 높이의 요소 캡처에 화면 밖 fixed skip-link가 끼어드는 현상은 실제 viewport와 computed position을 확인했고, 문서 원점에서 전체 페이지를 캡처하도록 바꿨다. 접근성 링크를 숨기거나 제거하지 않았다. `output/playwright/site-moderation-{1440,390,320}.png`.
- 기존 신고 관리 브라우저 **40개**, 운영자 브라우저 최종 **47개**, 런타임 예외 0개. 운영자 기존 46개에 이미 연결된 서버 등록을 ‘준비 중’이라던 잔여 문구 제거/SSR 확인을 추가했다. 회원 생명주기 49개와 설명 전수 검사는 이번 단위에서 재실행하지 않았다.
- `check-moderation-role.ps1`: 실제 같은 바이너리의 역할 회수/부여/중복 부여와, 일반 회원이 된 세션의 사이트·신고 API/SSR/쓰기 거절을 확인했다. 원래 역할로 복구하고 가상 자료만 정리했다.
- release server / wasm32 web 검사, 실제 dx fullstack 빌드·실행, 포맷/공백 검사 통과. 개발 재시작 사이의 hot-reload WebSocket 실패와 의도적인 409 응답은 성공으로 위장하지 않는다. 안정된 빌드에서 검사했다. 기존 unused/dead-code 경고는 남는다.
- 새 패키지·컨테이너 없음. Cargo.lock SHA-256 `889238F0DEE737E4CA52F8D4C4A099A0ACCD63D526B4FC82F3D0D53A71A00B7C` 유지. SQLx/bcrypt를 다시 넣지 않았다.
- 공개 HTTP 11개 통과. 마지막 가상 자료 정리 뒤 개발/테스트 DB의 회원·연동·서버·댓글·신고·관리 역할·조치 이력·사이트 편집 이력·수집 작업은 각각 0개, 임시 snapshot/rehearsal DB도 0개를 확인했다. fixture는 재생성 가능하다. 운영자의 실제 데이터는 삭제하지 않았다.
- 실제 ZIP/회원 DB/Phoenix는 변경하지 않았다. 가상 example.org 하위 서버의 단발 수집 실패를 확인한 것이며 실제 제품 호환 성공이 아니다. Linux 배포·물리적 서버 삭제·소프트웨어 관리자 기능·실데이터 활성화는 아직 완료 범위가 아니다. [기능 계약과 남은 부분](site-moderation.md).

## 2026-09-14 관리자 권한·신고 처리

- 최종 전체 서버/격리 PG **138개 통과, 실패·무시 0개, 73.51초**. 그 직전 전체 실행도 138개/79.93초 통과했다. 관리자 집중 6개(초기 1.16초)와 격리 대상의 역할/이력 보존 검사 1개를 추가했다. 내장 migration 14개다.
- 현재 DB 세션/차단/역할/freshness, 역할 회수, 동시 판정 CAS, 원문 증거와 현재 댓글 분리, 삭제 후 답글 보존, 별도 신고 상태, 관리자 계정 차단 거절, 모든 세션·연결 actor와 기존 회원 주소의 익명 proof 폐기, ban→unban 뒤 과거 요청 거절, 탈퇴 뒤 이력 FK 분리, 입력·이력 페이지 상한을 확인했다. 실제 회원이나 AP 계정은 사용하지 않았다.
- `check-moderation-browser.js`: **40개 통과, 런타임 예외 0개**. 실제 HTTP/PG의 신고 기각→충돌 거절→초안 보존/최신 확인→재검토→댓글 삭제 표시→이용 차단/해제→처리 완료→새로고침·상태 필터와 이력 저장을 확인했다. 응답 mock 없음. 익명/Origin/8 KiB/1,000자/private SSR/HTML 비실행/1440·390·320px 포함. 최초 검사는 끝까지 실행했지만 반환값을 출력하지 않았고, 반환값을 고친 뒤 새 가상 자료로 40개 결과를 다시 확인했다.
- 캡처를 직접 보고 모바일 버튼 너비와 길던 ISO 시각 표시를 정리한 뒤 재검증했다. `output/playwright/moderation-{1440,390,320}.png`. Playwright를 통한 실제 화면 확인이 이 두 보정에 사용되었다.
- `check-moderation-role.ps1`: 실제 동일 바이너리의 CLI revoke/grant/중복 grant, 권한이 회수된 일반 회원 세션의 API 읽기/쓰기·SSR 거절과 no-store를 확인했다. 처음에는 PowerShell의 다중 헤더 값을 `.ToString()`으로 검사해 잘못 실패했다. 헤더 값 결합으로 수정해 재통과했으며, 첫 시도도 finally에서 가상 역할을 복구했다. 비공개 데이터가 유출된 사례로 판정한 것은 아니다.
- 기존 회원 브라우저 **49개/런타임 예외 0개** 및 공개 HTTP **11개** 재통과. 공개 AP 성공·실회원 이관·설명 전수 브라우저 재검증으로 간주하지 않는다. 최초 컴파일의 문자열/소유권/테스트 구문 오류는 수정 후 테스트를 실행했다.
- 실제 dx 서버/WASM 빌드·hydration, release 서버 및 wasm32 web `cargo check --locked --offline`, 포맷/공백 검사를 통과했다. 기존 unused/dead-code 경고는 남는다. Linux 실행 검증은 아니다.
- 개발 서버를 재시작했고, 종료된 이전 launcher에 남아 있던 이 프로젝트의 개발 자식 프로세스 2개만 경로·부모 종료를 확인해 정리했다. 현재 서버와 관련 없는 프로세스는 중지하지 않았다.
- 가상 관리자/회원·신고/댓글·조치 이력을 정리했다. 개발/테스트 DB의 회원·연동·서버·댓글·신고·역할·이력은 각각 0개, 테스트 작업 및 임시 snapshot/rehearsal DB도 0개를 확인했다. 재생성 가능한 가상 자료이며, 실제 ZIP/회원 DB/Phoenix는 변경하지 않았다.
- Cargo.lock SHA-256은 `889238F0DEE737E4CA52F8D4C4A099A0ACCD63D526B4FC82F3D0D53A71A00B7C`로 같다. 새 패키지·컨테이너 없음. [권한 부여·보관 의미·미완료 범위](moderation.md).

## 2026-09-14 신규 서버 등록

- 최종 전체 서버/격리 PG **131개 통과, 실패·무시 0개, 72.86초**. 그 직전 전체 실행도 131개/75.79초 통과했다. 새 등록 집중 6개와 격리 재이관의 등록 귀속 보존 검사 1개를 추가했다. 내장 migration은 13개다.
- 실제 수집 검증기에 합성 HTTP 문서를 주입해 주소 확인→미리보기→등록·관측·첫 작업의 원자 저장, 소유권 미부여, 중복 경쟁/숨김·종료 보존, 생성일/현재 연동 근거, ban·세션 회수 후 거절, NodeInfo 없는 서버 미등록, 동시 일일 제한과 탈퇴 후 귀속 분리를 확인했다.
- 첫 빌드의 Diesel nullable/timestamptz 매핑 및 테스트 메서드 충돌을 수정했다. 최초 집중 실행은 테스트 AP 문서가 actor 생성일을 누락해 실패했다. 이어 요청 제한 scope의 DB 제약 누락을 발견하여 기존 migration을 고치지 않고 013으로 확장했다. 실패 때 남은 가상 회원 12개는 확인한 UUID만 정리했다. 이후 집중 6개와 전체 131개를 통과했으며 과거 실패를 성공으로 세지 않는다.
- `scripts/check-site-registration-browser.js`: **33개 통과, 런타임 예외 0개**, 간격 수정 후 다시 33개 통과. 실제 PG 회원/HTTP로 익명·Origin·8 KiB·내부 주소·중복 거절, 기존 오너 정보 불변, private SSR을 확인했다. 미리보기/등록 완료 화면은 **명시적인 브라우저 모의 응답**이다. 실제 원격 등록 성공으로 간주하지 않는다.
- UI에서는 요청 중 재제출 금지, 오류 후 입력 보존, 주소 변경 시 예전 미리보기 폐기, 0/가입 닫힘 표시, 원격 HTML 비실행, 확인 요청이 도메인만 전송함, 다음 동선을 확인했다. 1440/390/320px 가로 넘침 없음·긴 설명 내부 스크롤을 검사하고 캡처를 직접 보아 붙어 있던 카드 사이에 간격을 추가했다.
- 실제 dx fullstack 서버/WASM 빌드와 hydration, release 서버 및 wasm32 web `cargo check --locked --offline`, 포맷/공백 검사를 통과했다. 개발 서버를 재시작하는 동안 브라우저의 hot-reload WebSocket 연결 실패가 있었으나 안정된 서버에서 UI 검사를 다시 통과했다. Linux 배포 실행은 검증하지 않았다.
- 가상 브라우저 회원·서버와 종속 데이터는 정리했고 재생성 가능하다. 개발 DB 회원/연동/서버/등록 귀속 0개, 테스트 DB 회원/연동/서버/등록 귀속/작업 0개, 임시 snapshot/rehearsal DB 0개를 확인했다. 운영 ZIP/실제 회원 DB/원격 SNS 계정은 변경하지 않았다.
- Cargo.lock SHA-256은 `889238F0DEE737E4CA52F8D4C4A099A0ACCD63D526B4FC82F3D0D53A71A00B7C`로 같다. 패키지/컨테이너 추가 없음. [기능 계약 및 미확정 생성일 정책](site-registration.md).

## 2026-09-14 구 회원 첫 로그인 귀속

- 전체 서버/격리 PG **124개 통과, 실패·무시 0개, 64.38초**. 내장 migration 11개.
  `--locked --offline --no-default-features --features server --bin fediversekr2 -- --include-ignored --test-threads=1`로 실행했다.
- [구 회원 로그인 계약](legacy-member-login.md): 회원 저장소 테스트 6개와 importer 통합 2개를 추가했다. 합성 Phoenix import 후 실제 공개 AP verifier에 가상 문서를 공급하여, 옛 UUID/표시 이름/가입 시각/댓글/서버 소유권 유지와 비공개 연동, 최초 actor 고정, ban·잘못된 proof·다른 회원 세션 거절, 동시 귀속, 해제·탈퇴 후 부활 방지, 같은 actor 주소 변경·다른 옛 회원 병합 거절을 확인했다.
- 격리 대상의 actor 고정은 재이관으로 초기화되지 않는다. 신규 세션·연동이 없고 고정만 있어도 재이관은 거절한다. runtime quarantine 차단과 원본 snapshot 불변도 통과했다.
- 최초 집중 검사는 5개 통과/1개 실패였다. Rust 인증 시각의 나노초와 PG 저장 마이크로초를 그대로 비교한 테스트 오류를 저장 정밀도로 수정했다. 이어서 먼저 시작된 전체 검사는 옛 단언 실패 1개와 남은 가상 서버가 전역 큐에 들어간 연쇄 실패 2개로 121/124였다. 실패 fixture의 정확한 UUID를 확인해 합성 댓글 2개·서버 5개·회원 4개와 종속 테스트 데이터만 삭제하고 전체를 다시 실행해 124/124를 확인했다. 재생성 가능한 테스트 자료이며 실제 회원 자료가 아니다.
- 종료 후 `fedkr_test`의 회원·연동·예약·challenge·서버·작업·댓글·신고가 각각 0개, 합성 snapshot/rehearsal DB도 0개임을 확인했다. 실데이터 ZIP을 열거나 trust 클러스터에 복원하지 않았다.
- wasm32 web `cargo check --locked --offline` 통과 (14.73초), `cargo fmt --all -- --check`와 `git diff --check` 통과. 기존 unused/dead-code 경고는 남아 있다. Cargo.lock SHA-256은 `889238F0DEE737E4CA52F8D4C4A099A0ACCD63D526B4FC82F3D0D53A71A00B7C`로 같고 새 의존성/컨테이너는 없다.
- 이번 변경의 브라우저 전수 검사·실제 원격 AP 성공·Linux 배포·운영 활성화는 하지 않았다. 로그인 안내 한 문장을 추가했으며 랜딩/설명 디자인은 변경하지 않았다. 아래 브라우저 기록은 각 실행 시점의 증거다.

## 2026-09-13 댓글·답글·신고 연결

- 전체 서버/격리 PG **116개, 실패·무시 0개, 59.31초**. 댓글 간격/수정 기한/수정·삭제 경합, 숨김 재검사, 단일 깊이/같은 서버 답글, 공개 DTO 비밀 비노출, 삭제 원문 분리, 신고 증거/중복/요청 상한, 페이지 상한과 내장 migration 10개를 확인했다. 신고 증거가 있는 격리 대상의 이관 재실행 거절도 포함한다.
- `check-community-browser.js` **34개, 런타임 예외 0개**. 실제 가상 PG 회원으로 답글·수정·신고·삭제 저장, 초안 유지, 충돌 거절/명시적 최신 내용 읽기, 삭제 확인 중 경합, HTML 비실행, 익명/Origin/본문 상한, 1440/390/320px를 확인했다. 펼친 답글도 새 저장 후 갱신된다. 같은 바이너리의 변경 알림은 DB 테스트에서 확인했고 브라우저 충돌 검사는 수동 새로고침 경로를 명시적으로 사용한다.
- 실제 dx fullstack 서버/WASM 빌드와 hydration 확인. CLI 시나리오 파일의 끝 세미콜론 때문에 첫 실행이 구문 오류로 끝났고, 실행 형식을 고친 뒤 전체 34개를 통과했다. 처음 잘못 연 `/server/...`의 404는 성공으로 세지 않았다. 실제 경로는 `/servers/:slug`다.
- 테스트 회원 2개·댓글 7개·신고 1개 및 연동/세션/사이트를 정확한 fixture 범위로 정리했다. 개발 DB의 회원·연동·사이트·댓글·신고·신고 증거는 모두 0개. 가상 자료는 스크립트로 재생성할 수 있다. 실제 ZIP/Phoenix에는 접근하지 않았다.
- Cargo.lock SHA-256은 `889238F0DEE737E4CA52F8D4C4A099A0ACCD63D526B4FC82F3D0D53A71A00B7C`로 동일하다. 새로운 패키지/컨테이너 없음. [구현 범위와 관리자 미완료](community.md).
- 기존 설명·카탈로그·회원 브라우저 전수 검사는 이번 마무리에서 재실행하지 않았다. 아래 시점별 결과와 구분한다.

## 2026-09-13 소프트웨어 공동 편집

- 서버/격리 PG **108개, 실패·무시 0개, 55.47초**. 생성·다른 회원의 편집·대소문자 중복·동시 저장 경합·no-op·공개 이력·복원·NULL 원본·비편집 메타데이터 보존, 탈퇴 후 작성자 분리, 잠금·세션 폐기·ban 재검사, 도배 상한·이력 페이지와 이관 재실행의 편집 상태 변조 검출을 추가했다.
- migration 009는 두 보조 테이블만 추가한다. 원래 카탈로그 컬럼/ID를 바꾸지 않으며, 이관 대상과 격리 검사도 함께 확장했다. 원본 ZIP은 열지 않았다.
- 소프트웨어 브라우저 **49개, 런타임 예외 0개**. 실제 가상 PG 회원의 등록/수정/동시 편집/비교·겹침 확인/복원·복원 경합을 UI에서 끝까지 저장했다. 공개 이력의 회원 정보 비노출, SSR no-store, URL·본문 제한, 한글 장문, 1440/390/320px를 검증했다. 별도로 없는 편집/이력/복원 페이지의 404 및 이력 페이지 입력 400 등 HTTP 4개와 기존 공개 HTTP 11개도 통과했다.
- 첫 재검증 두 번은 dx 자동 재빌드 중 WASM 404와 겹쳐 중단됐다. 동일 소스를 watch=false의 안정된 개발 빌드에서 재실행한 49개가 최종 결과다. 테스트가 실패했던 사실을 정상 실행으로 바꾸어 세지 않았다.
- 가상 회원·연동·소프트웨어·종류·편집 이력을 정확한 fixture 조건으로 삭제했고 개발 DB와 테스트 DB에서 각각 0개를 확인했다. fixture는 재생성 가능하다. 실회원 ZIP은 사용하지 않았다.
- 원래 DB 미설정 예시 화면을 복구한 후 설명 6편·메뉴/목록의 기존 브라우저 검사 **483개를 다시 통과**, 런타임 예외 0개. 공개 HTTP 11개도 DB 설정/미설정 양쪽에서 통과했다. 이 검사만으로 사람이 처음 보고 이해하는지는 판정하지 않는다.
- [동작과 미완료 경계](software-contributions.md). 관리자 역할/개입 UI, 종류 자체 편집과 운영 자료 공동 편집은 완료 범위가 아니다.

## 2026-09-13 서버 찾기·수집 아이콘·독립 소프트웨어 상세

- 서버/격리 PG **100개, 실패·무시 0개, 54.89초**. 필터 교집합/가입 제외 개수/숨김 선택지/대소문자·미등록 계열/NULL 정렬/페이지 간 중복 방지/아이콘 바이트·공개 여부 재검사를 추가했다. 501개 합성 카탈로그를 넣어 목록 상한 밖의 직접 상세 조회도 확인했다. 새 migration은 없으며 8개를 유지한다.
- 서버 찾기 브라우저 **57개**, 설명·콘텐츠·종류→서버 브라우저 **483개**, 각각 런타임 예외 0개. 1440/390/320px, 조건 적용·URL·새로고침·뒤로/앞으로·한국어 태그·잘못된 조건 400·JavaScript 없는 필터 SSR을 확인했다. 483개는 기존 471개에 SPA 진입 시 선택란 일치 검사 12개를 더한 것이다.
- 실제 DB/HTTP 아이콘 브라우저 **18개**, 운영자 회귀 **46개**, 각각 런타임 예외 0개. PNG 실제 디코딩·GET/HEAD/405·보안 헤더·잘못된 경로·숨김 후 404/검색 제외·복구를 확인했다. 잘못된 이미지 응답은 해당 fixture URL만 브라우저에서 대체해 검증했다. 원격 이미지 수집 성공을 주장하지 않는다.
- 찾은 문제: plain prop을 캡처한 조회가 SPA 주소 변경 후 갱신되지 않음 → `use_reactive`로 연결. 새 선택란 삽입 시 기본 선택이 덮임 → option의 선택 상태를 명시. 모바일 검색 버튼 줄바꿈 → 줄바꿈/축소 방지. hydration 이전 이미지 오류가 누락됨 → SSR 이름표 후 이벤트가 연결된 상태에서 이미지 로드. 첫 변경은 핫리로드만으로 반영되지 않아 새 빌드로 다시 검사했다. 조회 완료 전 개수를 검사한 테스트 경쟁도 수정했다.
- release server / wasm32 web check 및 dx 실제 빌드 통과. SQLx/bcrypt/컨테이너 추가 없음. 기존 URL 의존성을 웹/클라이언트 feature에도 활성화했으며 Cargo.lock SHA-256은 `889238F0DEE737E4CA52F8D4C4A099A0ACCD63D526B4FC82F3D0D53A71A00B7C`로 동일하다. wasm 선택 그래프 287개에서 GPL/AGPL 표기 없음. 프로젝트 자체 라이선스 지정은 별도 남은 항목이다.
- 아이콘/운영자 fixture 정리 후 개발 DB 회원·연동·서버·작업·아이콘·DNS 대기 각각 **0개**. 테스트 DB 회원·연동·서버·작업 및 대량 카탈로그 fixture도 0개. fixture는 스크립트로 재생성 가능하며 실제 ZIP을 열지 않았다. 공개 HTTP 11개는 DB 설정/미설정 양쪽에서 통과했다.
- [새 조회 계약과 미완료 범위](server-discovery.md). 이전 49개 회원 생명주기 브라우저 검사는 이번 조회 변경 묶음에서 재실행하지 않았으며, 이전 기록과 구별한다.

## 2026-09-13 API 운영자 인증·수동 갱신 후속

- 서버/격리 PG **96개, 실패·무시 0개, 55.27초**. 모의 AP/API 응답은 실제 검증기를 통과시킨다. 계정/도메인/actor 바인딩, 안전한 v2→v1 fallback, Misskey 로컬 관리자, DNS 우선순위, 연동 변경·ban·로그아웃·revision 경합을 검증했다. 수동 큐의 1시간 제한·동시 요청·재시작 후 선점·임대 fencing·종료 서버 단발 조회·편집/아이콘 보전·실패 상태도 실제 PG에서 확인했다.
- 운영자 브라우저 **46개**, 기존 회원 브라우저 **49개** 재통과, 런타임 예외 0개. 운영자 폼의 오류 표시에는 HTTP 응답 수신 후 DOM 갱신을 기다려야 한다는 테스트 경쟁을 수정했다. 긴 계정 선택란은 모바일 가로폭을 제한했다.
- 운영자 브라우저는 DNS/공개 AP 글을 게시하지 않는다. 가상 서버의 수동 갱신만 큐에 넣어 reserved example.org 하위 도메인 수집 실패를 확인한다. 실제 제품 API 성공을 의미하지 않는다.
- 8개 내장 migration의 빈 schema/반복 적용 및 새 refresh 필드 NULL/빈 작업 상태를 포함한 importer 테스트 통과. Cargo.toml/Cargo.lock 해시는 직전과 동일. release server 및 wasm32 web cargo check 통과. 새 의존성/컨테이너 없음.
- 가상 회원·연동·서버·작업·DNS 대기는 정리 후 개발 DB에서 각각 0개. 테스트 DB도 회원·연동·서버·작업 0개. 검토용 DB 미설정 모드로 복귀한 뒤 콘텐츠 브라우저 **471개/예외 0**, 공개 HTTP **11개**를 재통과했다. 운영 ZIP과 Phoenix는 변경하지 않았다.
- 상세 계약과 남은 경계는 [site-ownership.md](site-ownership.md), 전체 상태는 [교체 준비 현황](../cutover-readiness.md).

## 2026-09-13 서버 운영자 후속

- 서버/격리 PG 90개 (실패·무시 0, 56.46초), 운영자 브라우저 33개, 기존 회원 브라우저 49개 재통과. 브라우저 두 묶음 런타임 예외 0개.
- [DNS/소유권/편집/공개 projection의 정확한 범위](site-ownership.md). 서버 수정 API만 한글 장문용 32 KiB body 제한이며 다른 회원 API 8 KiB 및 Origin 검증은 그대로다.
- 첫 브라우저 시도는 dx 재빌드 중 WASM 404로 hydration이 안 되어 중단. 이후 안정화해 재실행했다. 다음 시도는 예약 TLD `.example`인 fixture를 운영용 도메인 검사에서 거절했다. 안전 검사를 완화하지 않고 `example.org` 아래의 가상 하위 도메인으로 fixture를 수정했다. 가상 서버는 종료 상태로 생성해 수집 워커의 외부 요청을 막았다.
- 캡처 확인 후 전역 input 스타일이 체크박스를 늘리는 문제를 수정하고 3개 너비 모두 재검증했다. `output/playwright/site-owner-{1440,390,320}.png`.
- release server cargo check 및 dx WASM 실제 빌드 통과. Linux 배포·실회원·실 DNS 검증으로 간주하지 않는다.
- 정리 후 개발 DB의 회원/연동/서버/DNS 대기 요청 각각 0개 확인. DB 미설정 검토 모드로 복귀하여 공개 HTTP 11개와 설명·콘텐츠 브라우저 471개를 다시 통과했다 (런타임 예외 0개). 최신 release server 및 wasm32 web cargo check도 통과했다.

> 아래는 시점별 기록입니다. 후속 회원 관리 구현 범위는 [member-lifecycle.md](member-lifecycle.md), 최신 실행 결과는 [교체 준비 현황](../cutover-readiness.md)을 우선합니다.

## 2026-09-13 오프라인 이관 준비 검증

[이관 범위와 운영 차단](legacy-import.md). 실제 암호화 덤프는 열거나 복원하지 않았다.

- 전체 서버 테스트 **78 passed / 0 failed / 0 ignored**, 44.43초. `--locked --offline --no-default-features --features server --bin fediversekr2 -- --include-ignored --test-threads=1`.
- 새 8개 검사는 격리 URL 제약, dry-run/apply/재실행, 256행을 넘는 이력, 전체 원본 필드/NULL/UTC/소유권/숨김·제재/삭제·신고 관계, 서명키 보존, 계약 외 컬럼/테이블 거부, FK·핸들 충돌·잘못된 키에서 전량 rollback, 변경된 원본/대상 및 비어 있지 않은 대상 거부, 동시 실행 직렬화, 본문 크기/PG 로그 제한을 다룬다.
- snapshot/rehearsal URL 및 URL의 prefix 검사만으로는 걸러지지 않는 경우에도 DB 표식/원본 테이블을 확인해 HTTP·워커 초기화를 차단한다. 새 세션·연결 계정·크롤링 작업은 0개다.
- fixture는 Phoenix 스키마 계약에서 독립적으로 작성한 합성 데이터이며, 테스트마다 무작위 이름의 PG DB 두 개를 만든다. 최종 잔여 fixture DB **0개**, `fedkr_test` 회원·사이트·이관 표식 **각 0개** 확인. 초기 실패 때 남았던 합성 DB 2개도 식별 후 제거했다. fixture 코드로 재생성 가능하다.
- Release server 및 Wasm web의 `cargo check --locked --offline` 통과. 실제 Linux/운영 실행 검증은 아니다. 기존 unused/dead-code 경고는 남아 있다.
- `git diff --check` 통과. Cargo.toml/Cargo.lock SHA-256은 직전 의존성 감사와 동일하며 패키지를 추가하지 않았다. SQLx/Diesel 저장소 경계도 유지한다.
- 이번에는 브라우저 회귀를 재실행하지 않았다. 아래의 47개 브라우저 검사 결과는 이전 시점 기록이다.

## 2026-09-13 후속 검증

Diesel/diesel-async 전체 전환, 내장 migration, 동일 프로세스 워커를 추가했다. 상세 구조·미완료 경계는 [diesel-and-runtime.md](diesel-and-runtime.md)에 기록했다.

- 전체 서버 테스트 **70 passed / 0 failed / 0 ignored**, 21.19초. 격리 PG의 회원·공개 인증글 흐름에 더해 작업 재선점/fencing/소개문 보존/정리/평균, 빈 schema migration/반복 실행/서명키 보존/잘못된 이전 이력 rollback, supervisor 재시작/종료를 포함한다.
- `cargo check --release --no-default-features --features server --bin fediversekr2`: 통과. Release의 단일 프로세스 실행 경로 타입 검사이며 실제 운영/Linux 실행 증거는 아니다.
- Wasm `cargo check --target wasm32-unknown-unknown --no-default-features --features web --bin fediversekr2 --locked`: 통과. 웹 그래프에는 DB/암호 해시 의존성이 없다.
- `dx serve`: 서버+Wasm 빌드와 SSR 실행 성공. 동일 프로세스 시작 시 개발 DB에 migration 3개 적용, 기존 서명키 1개 유지, maintenance slot 2개 생성 확인. 수집 대상은 0개라 실제 외부 서버는 호출하지 않았다.
- Playwright 회원 HTTP/UI 회귀 **21/21, pageerror 0**. 선택형 ID/Argon2id 암호 설정·변경·이전 세션 폐기·재접속·로그아웃·동일 회원 로그인 확인.
- 익명·반응형·SSR 회귀 **26/26, runtimeErrors 0**. 1440/390/320px, 실제 오류 응답, 중복 제출, JavaScript 없는 초기 화면, 로그인 전 계정 접근 확인. 생성한 브라우저 테스트 회원과 그 세션/연결 계정은 정확한 UUID 조건으로 정리했다.
- HTTP 경계 **3/3** 재통과: 실제 chunked 413, Origin 우선 403, session 200/no-store.
- 명령의 `-- --include-ignored --test-threads=1`와 명시적 `FEDKR_TEST_DATABASE_URL=.../fedkr_test`를 사용했다. 마이그레이션 테스트가 만든 무작위 전용 schema는 정리했다.

아래는 이전 통합 시점의 기록이며 후속 결과를 대신하지 않는다.


2026-09-12. 개발 작업공간만 변경. 운영 DB/실제 SNS 계정/게시글/배포/커밋은 변경하지 않았다.

## 연결한 흐름

`/login` → WebFinger/AP 계정 확인 → 새 공개 인증글 문구 → AP 작성자·시각·공개 여부·문구 검증 → DB challenge 단회 소비 → UUID 회원/HttpOnly 세션 → `/account`.

추가 계정은 현재 회원·세션에 묶어 같은 인증 과정을 거친다. 자체 ID·암호는 로그인 후에만 선택적으로 등록한다. OAuth/MiAuth·자체 ID 단독 회원가입은 없다.

- 화면: `src/membership/pages.rs`, 기존 portal 스타일 + `membership.css`.
- HTTP: `membership/api.rs`의 typed Dioxus API, `membership/http.rs`의 역직렬화 전 Origin/실제 본문 제한.
- DB: `backend/auth.rs`, `flow.rs`, `migrations/202609120001_members.sql`.
- AP: `backend/federation/`, `identity.rs`, `config.rs`의 지속 RSA 키. 키는 `202609120002_instance_key.sql`의 별도 테이블에 저장된다.
- 실행: `scripts/dev.ps1`, 최초 `-Mode Setup`, 실행은 기본 모드, 테스트는 `-Mode Test`.

## 자동 검증 결과

| 검증 | 결과 | 증거 범위 |
| --- | --- | --- |
| `cargo test --no-default-features --features server --bin fediversekr2 -- --include-ignored` | **62 passed, 0 failed, 0 ignored** | 실제 별도 PostgreSQL + AP mock transport + UI/정책 함수 |
| `cargo check --target wasm32-unknown-unknown --no-default-features --features web --bin fediversekr2 --locked` | 통과 | 클라이언트 feature 분리/타입 검사 |
| `dx serve --web --fullstack true --bin fediversekr2` | 서버·Wasm 빌드/SSR 실행 성공 | `127.0.0.1:12239` |
| 회원 브라우저 흐름 | **21/21, pageerror 0** | 실제 폐기용 DB 회원 + 실제 HTTP/쿠키 + 실제 화면 조작; nonce 만료 보완 후 재통과 |
| 익명/반응형/SSR UI | **26/26, runtime exception 0** | 1440/390/320px, JS 비활성 SSR, hydration, 잘못된 계정 형식, 중복 제출, SPA 이동 |
| 랜딩·설명 회귀 | **49/49, runtime/asset 오류 0** | PC 두 창/짝 테두리/제품 전환, 모바일 한 창, 설명 4페이지×PC/모바일 양방향 스크롤 |
| 실제 HTTP 경계 | **3/3** | 길이 없는 chunked 9,000-byte 요청 413, 잘못된 Origin 먼저 403, 뒤따른 세션 조회 200/no-store |
| `git diff --check` | 통과 | 기존 CRLF 변환 경고와 소스 오류는 구분 |

기존 설치 컴포넌트의 unused/dead-code 경고 등은 남아 있으며, 전체 프로젝트가 Clippy 무경고라는 뜻은 아니다.

### DB 검증

폐기 가능한 `fedkr_test`를 사용했다. AP mock transport는 **실제 검증기**에 문서를 공급하며, 애플리케이션에 `verified=true` 등의 인증 우회 API를 추가하지 않았다.

- 새 AP 계정으로 local credentials 없는 회원 생성.
- 동일 actor로 동일 회원 재로그인, 다른 actor 추가 연결, 기본 비공개.
- 다른 브라우저 nonce/코드/세션/actor/발급시각의 proof 거절.
- 다른 challenge에서 만들어진 proof 재사용, 검증 후 만료, 이미 다른 회원에게 연결된 계정 거절.
- 동시 challenge 소비 단 한 번 성공.
- 선택형 ID 등록, 15분 freshness, 실제 Argon2id, 중복 ID, 현재 암호 확인, 암호 변경 후 전체 이전 세션 폐기.

### 브라우저 검증 재현

```powershell
./scripts/member-browser-fixture.ps1 -Mode Seed
npx --yes --package @playwright/cli playwright-cli -s=member-integration open http://127.0.0.1:12239/login --browser msedge
npx --yes --package @playwright/cli playwright-cli -s=member-integration state-load .local/browser-state.json
npx --yes --package @playwright/cli playwright-cli -s=member-integration snapshot
npx --yes --package @playwright/cli playwright-cli -s=member-integration run-code --filename scripts/check-membership-browser.js --raw
./scripts/check-http-boundaries.ps1
./scripts/member-browser-fixture.ps1 -Mode Clean
```

fixture 스크립트는 해당 작업공간의 loopback 클러스터임을 검사하고 별도 무작위 UUID 회원만 만들고 지운다. 가상 계정 `.example`만 사용한다. 테스트 토큰은 `.local`에만 생성하며 출력하지 않는다. **이 fixture는 공개 인증글 성공의 증거가 아니라, 그 이후 HTTP/화면 통합을 검증하는 준비 데이터**다. 원격 서버를 접속하거나 응답을 성공으로 위조하지 않는다.

검사: 익명 세션·Origin, 비로그인 ID 등록 거절, AP 공개키 형식/비밀키 비노출, 실제 ID 설정 UI, 쿠키 회전/HttpOnly/SameSite, 이전 세션 폐기, SSR no-store/비밀번호·토큰 비노출, 재접속, 모바일 overflow, 실제 암호 변경·기존 암호 거절·로그아웃·ID 로그인·동일 회원 UUID.

스크린샷과 익명/랜딩 회귀 스크립트는 무시되는 `output/playwright/`에 있다. `membership-account-desktop.png`, `membership-account-mobile.png`, `member-login-*.png`를 직접 확인했다. 의도적으로 실패시킨 HTTP 400은 브라우저 리소스 오류로 기록될 수 있으나 런타임 예외와 구분했다.

## 통합 중 발견해 수정한 것

- Dioxus 실제 웹 빌드에서 조건부 폼 key가 정적 문자열이면 실패함 → 첫 노드의 동적 key로 수정.
- 다른 브라우저의 암호 변경으로 폐기된 쿠키가 남았을 때 AP 재로그인이 막힘 → 검증된 현재 세션이 있을 때만 link 완료에 세션 토큰 전달. link 중 세션이 폐기된 경우는 계속 거절.
- 브라우저 바인딩 쿠키의 남은 수명이 새 challenge보다 짧아질 수 있음 → 새 인증 시작 때 동일 nonce의 쿠키 수명을 갱신. 5초 남은 쿠키를 넣은 실제 HTTP 검사로 갱신 확인.
- Content-Length만 검사하면 chunked 본문과 Dioxus 선행 역직렬화를 제한하지 못함 → HTTP middleware에서 Origin을 먼저 검사하고 실제 8 KiB까지 읽는다. 수신 5초 제한·64개 동시 요청 제한. 실제 chunked 요청으로 413 확인.
- PostgreSQL URL은 URL 라이브러리에서 non-special scheme이므로 IPv4 host enum 가정이 틀림 → 마이그레이션 대상은 정확한 `127.0.0.1:16439/{fedkr_dev,fedkr_test}` allowlist로 검사하고 테스트 추가.

## 운영 전 미완료

- 공개 HTTPS 주소에서 원격 서버가 `/actor#main-key` 공개키를 읽으며 실제 AP 조회/인증이 되는지. localhost만으로는 authorized-fetch 호환성을 확인할 수 없다.
- 원본 Phoenix의 제품별 REST fallback, actor alias/이사, 모든 소프트웨어별 호환.
- 완전한 AP actor/inbox/배달. 현재 `/actor`는 서명 공개키 발견용이며 일반 연합 서버 전체 구현이 아니다.
- 복구·탈퇴·연결 해제·공개 설정 UI·계정 병합·구 회원 귀속.
- 운영 프록시/IP 제한·키 백업·권한 분리·배포/되돌리기. 현재 개발 DB는 loopback trust이므로 외부 공개하면 안 된다.
- 전체 dependency 선택 라이선스는 별도 감사 문서에 기록. GPL/AGPL 필수 의존성은 없지만 배포용 notice 수집/자체 프로젝트 라이선스 선택은 남아 있다.

사람의 검토는 여전히 필요하다. 이 검증은 처음 방문한 사람이 설명을 이해하고 납득하는지에 대한 사용자 검증을 대신하지 않는다.
