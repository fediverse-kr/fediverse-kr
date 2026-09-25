# 서버 찾기 — Phoenix 조회 기능 이식 (현재 상태 2026-09-14)

2026-09-13. 추천은 제외. 회원 등록/편집 정책이나 승인된 랜딩 장면은 변경하지 않는다.

## 화면과 주소

- `/servers`에서 검색, 종류, 계열, 소프트웨어, 태그, 가입 방식, 응답 상태, 정렬, 페이지 크기를 항상 노출한다.
- Phoenix의 `q`, `cat`, `family`, `software`, `tag`, `reg`, `sort`, `dir`, `alive`, `page`, `page_size`를 읽는다. 새로고침/링크 공유/뒤로·앞으로 가기에도 조회 조건과 선택란을 유지한다.
- 기본은 이름 오름차순/모든 가입 방식/24개다. Phoenix의 기본 추천순/가입 접수 중/100개를 그대로 가져오지는 않았다. 기존 25/50/100/200개도 선택 가능하다.
- `sort=recommended`는 이름순으로 해석할 뿐 추천 점수는 사용하지 않는다. `oldest`는 등록 시각 오름차순의 옛 별칭이다.
- 가입 계정 수/확인 시각/등록 시각은 기본 내림차순, 이름/주소/평균 응답은 오름차순. 운영 종료는 마지막, 수집하지 못한 값은 방향과 무관하게 마지막이다. 마지막 동률은 도메인으로 정렬한다.
- 주소의 페이지 번호는 1부터, 내부 offset은 0부터. 입력 상한/중복 조건/알 수 없는 열거값은 HTTP 400으로 표시한다. 알 수 없는 태그·종류는 0건이며 임의로 모든 결과를 반환하지 않는다.

## 조회와 공개 경계

- `directory/search.rs`는 조회 조건·전송 모델만, `backend/db/public_directory/search.rs`는 Diesel/diesel-async SQL 어댑터다. SQL은 이 경계 밖으로 노출하지 않는다.
- 이 후속 구현은 기존 Diesel 경계를 유지하며 SQLx·bcrypt·GPL/AGPL 제품 소스를 추가하지 않았다.
- 필터 값은 bind 변수, 정렬은 고정된 SQL 조각의 허용 목록이다. `%`/`_`는 검색 wildcard가 아니라 문자 자체다.
- 결과와 전체 개수는 같은 repeatable-read/read-only 트랜잭션에서 읽는다. 가입 방식으로 제외된 수는 **다른 조건을 유지한 상태**의 차이다.
- 공개 목록, 직접 상세, 집계, 태그와 관측 소프트웨어 선택지, 아이콘은 숨김과 강제 숨김을 제외한다. 카탈로그 종류는 공개 카탈로그에서 가져온다.
- 계열을 아직 카탈로그에 등록하지 않은 소프트웨어도 자신의 이름으로 찾을 수 있다. 소프트웨어 관측값의 대소문자가 다른 경우도 카탈로그와 연결한다.
- 가입 접수 중에는 승인제도 포함한다. 초대 필요는 운영자의 초대 안내를 우선한다. 미확인을 닫힘으로 바꾸지 않으며, 운영 종료는 활성 가입 필터에 포함하지 않는다. 이는 옛 nullable bool 조건을 그대로 복사한 결과와 다를 수 있다.
- 선택지 조회는 소프트웨어 500/종류 200/태그 200 상한이 있고 초과를 화면에 알린다. **검색 결과를 500개로 제한하는 것은 아니다.** 선택지의 전체 페이지 분할은 남아 있다.
- `/api/public/servers`의 구 조회 API는 유지하고 새 화면은 `/api/public/server-search?filters=...`를 사용한다.

## 최근 응답 기록 — 2026-09-14 후속

- 서버 상세에서 Phoenix의 `recent_health_checks(server_id, 48)` 범위를 표시한다.
  이전→최근의 48칸 이하 막대와, 최신순으로 펼치는 시각·응답·소요 시간 표다.
  한국 표준시를 서버에서 계산하여 SSR/브라우저 시간대 차이를 피한다.
- 성공은 초록, 성공이며 2초 이상은 황색, 실패는 붉은 사선이다. 색 외에도 상태별 횟수와
  상세 표의 텍스트로 구분한다. 작은 칸 48개를 버튼으로 만들지 않고 기본 HTML `details`와
  스크롤 표를 사용한다. 기존 사이트 스타일이며 새 차트 라이브러리/DB migration은 없다.
- 각 칸은 한 번의 관측이다. 고정 시간 구간이나 가동률로 표시하지 않는다. 기록 없음은
  응답 실패나 100% 성공으로 바꾸지 않으며, 소요 시간 미확인은 0ms로 바꾸지 않는다.
- `/api/public/server-health?domain=...`은 기존 `directory_health_checks`에서만 읽는다.
  최근 시각·UUID 순으로 48개를 골라 같은 시각의 순서도 고정한다. 숨김 여부와 기록을
  하나의 SQL snapshot에서 읽으며 숨김/강제 숨김/없는 서버는 404, 입력 오류는 400,
  DB 장애는 503이다. 원격 재수집, 작업 생성, 내부 오류/작업 ID 공개는 하지 않는다.
- DB 미설정 때만 가상 기록을 예시로 표시한다. DB 설정 오류를 예시로 가리지 않는다.
- 합성 Phoenix import의 280개 보존과 최신 48개 조회를 연결 검증했다.
  실제 백업의 개별 기록 검증이 아니라 기존 importer와 새 공개 조회의 통합 검사다.
  브라우저 재현은 `member-browser-fixture.ps1 Seed -IncludeSite -IncludeHealth` 및
  `check-health-history-browser.js`를 사용한다. `-IncludeHealth` 없이 빈 기록도 검사한다.

## 수집 아이콘

- `/api/public/server-icon/:domain`은 새 수집 결과의 `directory_icons` 캐시를 우선 읽고, 없으면 이관된 `favicons/` 매핑을 OpenDAL로 fallback한다. 브라우저가 외부 아이콘 서버에 직접 연결하거나, 요청 때 원격으로 다시 가져오지 않는다.
- GET/HEAD 전용. 정규화된 공개 도메인, 숨김 여부, 최대 512 KiB, 저장된 MIME과 PNG/JPEG/GIF/WebP/ICO/AVIF/BMP/TIFF 바이트 식별의 일치를 다시 확인한다. SVG는 공통 `quick-xml` 검사로 단일 루트/크기/깊이/문법을 확인하고 DTD·처리 명령·HTML은 거부한다. 이는 전체 디코딩 검사나 SVG sanitizer가 아니며, 원본 바이트를 별도 이미지 응답으로만 제공한다.
- 내장 migration 025로 DB의 허용 MIME 목록을 맞췄다. 기존 이미지 바이트는 유지하고, 새 형식의 행이 남아 있으면 downgrade를 거부한다. 이관 `favicons/` 파일 매핑이 있으면 자동 워커는 재수집하지 않지만 명시적 운영자 갱신은 허용한다.
- `nosniff`, 제한된 CSP/sandbox, same-origin resource policy, `no-store`. 숨김 이후의 새 요청은 404다. 이미 받은 이미지나 외부에서 만든 복사본을 회수한다는 뜻은 아니다.
- 목록/상세는 같은 아이콘 컴포넌트를 사용한다. SSR에서는 이름 첫 글자를 먼저 표시하며, hydration 후 오류 이벤트가 연결된 상태에서 이미지를 로드한다. 이미지가 없거나 디코딩에 실패하면 첫 글자를 유지한다. JavaScript가 없는 경우도 이름표와 본문은 남는다.
- 아이콘 조회·이관 파비콘 fallback·수명주기 연결은 구현했다. 옛 원격 이미지 보관소 자체의
  원본 복사·삭제·보존 정책과 운영 저장소 검증은 아직 이관하지 않았다.

## 소프트웨어 상세

`/api/public/software/:name`은 목록 조회와 별개로 한 항목을 읽는다. 목록의 500개 표시 상한 밖에 있어도 상세를 열 수 있다. 카테고리 라벨도 해당 항목에 필요한 것만 조회한다. 허용 필드/외부 링크 검사는 목록과 공유한다. 전체 목록 페이지 분할과 회원 편집·변경 이력 UI는 후속 구현으로 연결했으며, 관리자/종류 관리와 남은 공개 이력 정책은 [카탈로그 관리](catalog-moderation.md)와 [소프트웨어 기여](software-contributions.md)를 따른다.

## 검증 재현과 남은 일

```powershell
./scripts/dev.ps1 -Mode Test
playwright-cli -s=fedkr-review run-code --filename scripts/check-directory-browser.js
playwright-cli -s=fedkr-review run-code --filename scripts/check-content-browser.js
```

브라우저 두 스크립트는 DB 미설정의 명시적 예시 화면용이다. 실제 PG 필터/숨김/정렬/페이지/개수/아이콘 검사는 격리 DB 테스트에서 수행한다. 별도의 DB 설정 개발 서버와 `member-browser-fixture.ps1 -IncludeSite -IncludeIcon`을 사용하면 `check-directory-icons-browser.js`로 실제 HTTP 바이트·헤더·이미지 디코딩·숨김 연동을 검증할 수 있다. `-IncludeSvgIcon`을 더하면 SVG 표시·직접 문서 열기의 스크립트/외부 리소스 차단·인라인 스타일을 검사한다. 이 fixture는 DB에 직접 넣는 가상 아이콘으로 원격 수집 성공 검사가 아니다. 가상 서버는 자동 수집을 막기 위해 종료 상태로 만든다. 끝나면 fixture를 정리한다.

실제 운영 데이터/이미지와 수집 대상 부하는 별도 확인이 필요하다. 실제 백업 완전성, 외부 AP
상호운용, 운영 TLS/프록시와 수집 대상 부하는 이 문서의 합성 검증으로 대체하지 않는다. 상태 이력 화면은 위 후속,
신규 서버 등록은 [등록 구현](site-registration.md)으로 연결했다. 최신 실행 숫자와 실패 수정 기록은 [통합 검증](integration-checks.md)을 참고한다.

근거: MIT Phoenix의 `lib/fediverse_kr_web/live/server_list_live.ex`, `lib/fediverse_kr/servers/servers.ex`. GPL/AGPL 제품 소스를 사용하지 않았다.
