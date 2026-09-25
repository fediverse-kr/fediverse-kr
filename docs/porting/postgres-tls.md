# PostgreSQL TLS와 사설 CA

앱은 기존 Rustls + tokio-postgres-rustls를 Diesel 공통 연결 함수에 사용한다.
PEM/X.509 파서를 새로 작성하거나 libpq/OpenSSL을 앱 의존성으로 추가하지 않는다.
연결 풀, 내장 migration, 격리 import/activation은 같은 연결 함수를 공유한다.
DB 쿼리/ORM은 계속 도메인 모델 밖의 인프라 모듈에 둔다.

## 선택형 설정

`FEDKR_DATABASE_CA_FILE`은 운영자가 신뢰하는 **DB CA 인증서 묶음**의 절대 파일 경로다.
환경변수가 없으면 기존 공인 루트 목록만 사용한다. 설정하면 그 목록에 CA를 추가한다.

- 비어 있지 않은 일반 파일, 최대 256 KiB의 PEM certificate 묶음만 받는다.
- 상대 경로·최종 경로의 symlink·디렉터리·빈/없는 파일·권한 오류·크기 초과·깨진 PEM/인증서,
  인식된 private key/CSR/CRL 같은 비인증서 섹션은 거절한다. 비밀키 파일을 넣지 않는다.
- 앱 사용자에게 읽기 권한이 있어야 한다. 컨테이너에는 일반 파일로 읽기 전용 마운트한다.
  symlink를 만드는 projected mount라면 이 파일 규칙에 맞는 별도 파일 마운트가 필요하다.
- 이 설정은 **DB 전용**이다. AP, 크롤러, 시스템 신뢰 저장소를 변경하지 않는다.
- CA를 지정하면 loopback URL에도 TLS를 강제한다. TLS가 없거나 인증서/호스트 검증에
  실패하면 평문으로 재시도하지 않는다. 잘못된 설정을 무시하고 공인 루트만으로 연결하지 않는다.
- CA를 지정하지 않은 기존 개발 예외는 그대로다. query 없는 `127.0.0.1`/`::1` URL만
  평문을 사용하며, 다른 DB 연결은 인증서를 검증하는 TLS가 필요하다.
- CA 파일 변경은 새 연결부터 반영될 수 있지만 기존 연결을 재검증하지는 않는다.
  운영 CA 교체 시 앱 재기동과 readiness를 포함해 확인한다. 클라이언트 인증서/mTLS는 별도 미지원이다.

PEM 섹션은 기존 `rustls-pki-types`의 `PemObject`로 읽고 각 certificate를
[Rustls RootCertStore::add](https://docs.rs/rustls/0.23.43/rustls/struct.RootCertStore.html#method.add)에
전달한다. 자체 인증서 해석, hostname 검사 우회, 시스템 인증서 설치는 없다.
파일 경로·인증서 바이트·DB URL·원격 오류는 외부 오류 메시지에 싣지 않는다.

설정된 서버는 HTTP 제공 전에 DB/저장소/키 초기화를 한다. CA 설정 오류나 TLS 거절은
시작 실패이며 DB 없는 예시 화면으로 대체하지 않는다. 준비된 DB에서만 readiness 200이 가능하다.

## 재현 가능한 실제 TLS 검사

Linux 일반 사용자 환경과 기존 PG14/OpenSSL 실행 도구, Dioxus 처리된 Linux release가 필요하다.
이들 도구는 테스트 인증서/합성 DB 생성용이며 앱 이미지에 포함하지 않는다.

```sh
python3 scripts/check-postgres-tls.py \
  --binary /absolute/verified-release/server \
  --public-dir /absolute/verified-release/public
```

검사마다 private 사용자 캐시에 CA/서버 인증서와 합성 PG를 새로 만든다. PG는 동적으로
선택한 loopback 포트만 듣고 시스템 trust/DNS/다른 클러스터를 변경하지 않는다.
libpq verify-full 제어 요청으로 fixture 인증서/호스트 자체를 확인하고, 실제 앱을 기동해
내장 migration과 연결 풀의 `pg_stat_ssl`을 확인한다. 별도 실제 회원/운영 데이터는 없다.

없는·빈·깨진·과대·상대·디렉터리·symlink·키가 섞인 CA, 다른 issuer, 신뢰 CA 없는 TLS, hostaddr를 고정한
호스트 불일치, 실제 TLS를 끈 PG를 검사한다. 마지막 경우 loopback 평문 제어 연결이
성공하는 것도 확인해, 단순 접속 불가를 downgrade 거절로 잘못 해석하지 않는다.
각 실패 기동의 비밀 비노출, 정상 SIGTERM 종료와 기존 서명 키 보존도 확인한다.

현재 실행 결과는 [통합 검증 기록](integration-checks.md)을 따른다. 로컬 생성 인증서의
성공/실패를 확인한 것이며, 운영 PG의 CA·접속 권한·TLS 정책·실자료 이관까지 검증한 것은 아니다.
