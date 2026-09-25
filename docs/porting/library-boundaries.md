# 라이브러리와 프로젝트 정책의 경계 — 2026-09-14

목표는 라이브러리 수를 늘리는 것이 아니라, 범용 구현을 유지보수하는 부담을 줄이는 것이다.
전부 서버 feature 전용이며 HTTP와 워커는 계속 같은 바이너리에서 동작한다.

| 맡긴 처리 | 사용 라이브러리 | 프로젝트에 남긴 규칙 |
| --- | --- | --- |
| 파일 바이트 읽기·쓰기·flush·개별 삭제 | Apache OpenDAL 0.59.1, Fs | 비공개 경로, 용량·해시 검사, 덮어쓰기 금지, DB 참조 권한 |
| 인증 digest의 상수시간 비교 | subtle 2.6.1 | SHA-256 32바이트라는 입력 계약 |
| Cookie 헤더 읽기·Set-Cookie 작성 | cookie 0.18.2 | 중복 거절, 64자리 토큰, HttpOnly/SameSite/Secure/Host 정책 |
| URL percent decoding | percent-encoding 2.3.2 | 잘못된 escape·경로 구분자·제어문자 거절, 크기 상한 |
| SVG 문서의 XML 이벤트 해석 | quick-xml 0.41.0 | SVG 단일 루트, 깊이 128 이하, DTD/PI 거절, 별도 응답의 CSP |
| 새 로고의 PNG/JPEG/WebP 디코딩 | image 0.25.10 | 파일/픽셀 상한, 정지 이미지, 관리자 권한, 원본 바이트 보존 |
| AP 프로필 PNG/JPEG/WebP/GIF 첫 프레임 디코딩 | 같은 image 공통 모듈 | AP 계정 귀속, 사진 출처 선택, 실패 보존, 본인 전용 이미지 |
| 이관 이미지·수집 아이콘의 형식 판별 | image의 `guess_format` / `to_mime_type` | PNG/JPEG/GIF/WebP/ICO/AVIF/BMP/TIFF만 허용, 원본 바이트 유지, HTML 거절 |

프로필 갱신 다운로드는 기존 crawler의 reqwest/DNS/redirect 제한을 그대로 재사용한다.
[사진·이모지 갱신](profile-media-refresh.md)의 제품 규칙만 별도 모듈에 남겼다.

## OpenDAL은 선언만 추가한 것이 아니다

- `backend/storage/files.rs`의 공통 reader가 OpenDAL `read_with`로 실제 바이트를 읽는다.
  읽기 전 파일 크기를 확인해 정확한 범위만 요청하고, 읽은 뒤 길이·수정 시각·경로를 재확인한다.
  상한보다 큰 범위를 무조건 요청하면 짧은 파일에서 EOF 오류가 나는 회귀를 테스트로 수정했다.
- 웹 이미지 조회와 `legacy_assets`의 원본/묶음 읽기 모두 이 reader를 사용한다.
- 오프라인 보존의 실제 payload 쓰기도 `Operator::write`다. Fs의 close에서 flush/sync가 수행된다.
- 탈퇴 후 공유되지 않는 파일의 물리 삭제도 `Operator::delete`다. 삭제 예약·참조 검사·재시도는
  기존 PG/Tokio에 남기며 HTTP 트랜잭션에서 파일을 직접 지우지 않는다. [삭제 계약](media-cleanup.md).
- 완성 파일 등록은 로컬 `hard_link`로 남겼다. OpenDAL Fs의 임시 파일 rename만으로는
  경합 시 기존 목적지를 덮어쓰지 않는다는 이 프로젝트의 보존 계약을 충족하지 못한다.
  임의의 새 임시 파일을 배타적으로 예약하고, 완성된 뒤 원자적으로 등록하고 원본 바이트를 재검사한다.
  중단/장애로 남은 `.partial-*`는 조회/색인에 사용되지 않으며 자동 광역 삭제하지 않는다.
- OS 경로 검사와 파일 내용 처리는 다르다. Fs는 공격자가 동시에 경로를 갈아끼우는 것을 막는
  OS capability sandbox가 아니다. 입력 export와 저장소는 신뢰하는 서비스/이관 계정만 쓸 수 있어야 한다.
  정적 symlink/Windows junction 및 경로 이탈은 거절하지만 악성 로컬 writer에 대한 보장을 주장하지 않는다.
- [관리자 로고 업로드·교체·삭제](catalog-logos.md)를 공통 publication에 연결했다.
  OpenDAL의 blocking adapter와 표준 OS 파일 잠금으로 런타임 작성/삭제를 직렬화한다.
  PG 참조와 실패 시 정리 예약은 DB adapter에 남기며 새로운 큐 프레임워크는 넣지 않는다.
- 현재 활성화한 backend는 **로컬 Fs만**이다. S3 설정·원격 읽기까지 구현된 것은 아니다.
  다음 backend를 붙여도 회원/숨김/DB 참조 권한은 `media`와 DB adapter에 남긴다.

## 의도적으로 교체하지 않은 것

- HTTP 서명: `http-signature-normalization 0.7.0`은 crate metadata가 AGPL-3.0이므로 제외했다.
  소스를 참조하거나 의존성으로 넣지 않았다. 현재 RSA 암호 연산은 `rsa`/`sha2`를 쓰고,
  기존 Phoenix와 호환되는 `(request-target) host date` 조립만 작은 adapter로 남긴다.
  다른 규격의 HTTP Signature로 바꿔 호환성이 좋아졌다고 가정하지 않는다.
- 이미지 형식 추정용 `infer 0.22.0`(MIT)은 적용하지 않았다. 현재는 이미 사용하는 `image`의
  시그니처 판별을 재사용한다. 디코더 feature를 늘리지 않아도 기존 AVIF/BMP/TIFF를 원래 MIME과
  바이트로 제공할 수 있다. 이 판별은 전체 파일이 정상이라는 검증이나 모든 브라우저에서의
  표시 보장이 아니다. 새 업로드/AP 갱신의 디코딩·메모리 제한과 기존 파일 조회는 구분하며,
  이관 이미지의 조회를 재인코딩/삭제 정책으로 바꾸지는 않았다.
- 작업 큐의 점유·재시도·복구와 프로세스 감독은 범용 실행 기반이다. 인증글 검증·공개 범위·
  수집 대상/주기 같은 제품 정책과 구분한다. 현재는 이미 구현된 PG/Diesel 큐와 Tokio 실행부를
  [이번 릴리즈 판정](worker-release-gate.md)에 따라 유지한다. 자작이 항상 더 적합하거나,
  actor/broker가 불필요하다는 일반적인 결론이 아니다. 구조 수정이 필요하면 기성 대안을 검토한다.
- Argon2id, HTML 파싱의 html5ever, DNS의 Hickory, URL의 url, TLS의 Rustls 등 이미 쓰는
  라이브러리는 유지한다. SQLx/bcrypt/Kameo/NATS/RabbitMQ는 추가하지 않았다.

## 라이선스와 검증

OpenDAL은 Apache-2.0, subtle은 BSD-3-Clause, cookie/percent-encoding은 MIT OR Apache-2.0,
quick-xml은 MIT, image는 MIT OR Apache-2.0이다. 선택된 전이 의존성까지의 감사는 [dependency-licenses.md](dependency-licenses.md),
실행 결과는 [integration-checks.md](integration-checks.md)를 따른다.
라이선스 선언 감사가 최종 배포 고지 조립이나 운영 검증을 대신하지 않는다.

공식 API: [OpenDAL](https://docs.rs/opendal/0.59.1/opendal/),
[subtle](https://docs.rs/subtle/2.6.1/subtle/trait.ConstantTimeEq.html),
[cookie](https://docs.rs/cookie/0.18.2/cookie/struct.Cookie.html),
[percent-encoding](https://docs.rs/percent-encoding/2.3.2/percent_encoding/),
[quick-xml](https://docs.rs/quick-xml/0.41.0/quick_xml/).
