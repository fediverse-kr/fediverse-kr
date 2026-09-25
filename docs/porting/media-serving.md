# 기존 이미지의 런타임 조회 — 2026-09-14

보존 도구와 웹이 같은 파일 읽기 계층을 사용한다. 이미지가 화면에 나오는 경로를
연결한 것이며, 실제 백업 이관·이관 DB 운영 활성화까지 완료한 것은 아니다.
새 로고 업로드와 AP 사진 갱신은 아래 후속 문서를 따른다.

## 저장과 접근 경계

- `backend/storage`가 파일 경로·링크/정션·16 MiB 상한·길이·SHA-256을 검사한다.
  실제 읽기와 오프라인 보존의 바이트 쓰기는 OpenDAL Fs다. 새 컨테이너는 없다.
  로컬 경로 정책과 덮어쓰기 없는 등록은 별도로 유지한다. [라이브러리 경계](library-boundaries.md).
- `FEDKR_MEDIA_DIR`는 `objects/<sha256>`를 포함하는 기존 절대 비공개 디렉터리다.
  웹 정적 자산 아래에 두지 않는다. 환경변수의 디렉터리 이름만으로 비공개 OS 권한이
  설정되는 것은 아니다. 운영에서는 서비스 계정의 읽기/파일 정리 권한을 허용하고,
  다른 쓰기 주체를 제한한다. 원본 export와 독립 백업은 운영 저장소와 분리한다.
- migration 018의 `stored_files`는 논리 키→해시/길이 매핑이다. 이전 017 파일 보존 결과도
  이 테이블에 복사한다. 이관 ledger와 런타임 매핑의 일치 여부를 오프라인 재검증한다.
  이관 DB의 격리 표식이나 HTTP/워커 차단은 해제하지 않는다.
- 파일을 읽기 전과 읽은 후에 DB 참조·숨김·현재 세션·회원 차단을 재검사한다.
  저장 키·해시·회원 UUID를 이미지 조회 권한으로 사용하지 않는다.

| 경로 | 접근 조건 |
| --- | --- |
| `/api/public/software-logo/:name` | 현재 소프트웨어의 `software-logos/` 또는 `software/` 키 |
| `/api/public/server-icon/:domain` | 현재 공개 서버. 새 PG 캐시 우선, 없으면 옛 `favicons/` 키 |
| `/api/member/avatar` | 유효한 세션의 본인에게 연결된 옛 `avatars/` 키만 |
| `/api/member/emoji?name=<shortcode>` | 유효한 세션의 본인에게 연결된 옛 `emojis/` 키만. canonical URL은 `name` 하나를 percent-encoding한다. 이전 `/api/member/emoji/:shortcode`는 simple legacy alias |
| `/api/member/profile-media` | 본인의 아바타 유무와 최대 32개 shortcode. 경로·해시는 반환하지 않음 |

공개 댓글·다른 회원의 프로필에 옛 연합 계정 이미지를 자동으로 노출하지 않는다.
비공개 연동이라는 새 방침에 맞추려면 공개 프로필의 동의·표시 모델이 별도로 필요하다.

## 응답과 화면

- GET/HEAD만 받으며, HEAD는 본문 없이 원래 길이를 반환한다. 매 조회에 파일 무결성을
  확인한다. 누락·변조·저장소 장애는 503, 모르는 참조는 404, 세션 없음은 401이다.
- 기존 `image` 라이브러리로 PNG/JPEG/GIF/WebP/ICO/AVIF/BMP/TIFF 시그니처를 판별하고,
  quick-xml로 SVG 단일 루트/종료·깊이를 확인한다. 파일명·원격 Content-Type은 판별 근거가 아니다.
  임의 바이트는 보존될 수 있지만 이미지로는 415다. SVG DTD/외부 엔티티 선언과 PI는 거절한다.
  시그니처 판별은 전체 파일 디코딩 검증이 아니며, TIFF 등은 브라우저 지원에 따라 대체 표시된다.
  새 업로드/AP 갱신의 허용 형식·디코딩 제한은 이관된 원본의 서빙과 별개로 유지한다.
- SVG를 DOM에 삽입하거나 sanitize했다고 주장하지 않는다. 별도의 이미지 응답에
  `sandbox`, `default-src 'none'`, inline style/data image만 허용하는 CSP를 적용한다.
  같은 출처의 별도 SVG 문서를 직접 열어도 스크립트·외부 이미지 요청을 차단한다.
- 전부 no-store, nosniff, same-origin CORP, no-referrer. 개인 이미지는 private/Vary Cookie,
  cross-site·same-site 형제 출처의 Fetch Metadata 요청도 거절한다.
- Phoenix가 JSON map에 남긴 원 shortcode는 storage filename과 다르다. canonical query의 shortcode는
  percent-decoding 뒤 빈값·1 KiB 초과·제어문자만 거절하며, encoded 길이는 최대 세 배로 제한한다.
  잘못된 percent escape·UTF-8, 중복 name, 알 수 없는 query도 거절한다. 표준 URL 라이브러리로
  한 번만 form decode하며 `%2F`라는 이름과 `/`라는 이름을 혼동하지 않는다.
  `/`, `.`, `@`, `-`, 비ASCII를 경로/파일명으로 재해석하지 않는다. 인증된 회원의 map에서 정확히 일치하는 `emojis/` 참조만
  다시 조회하므로, 인코딩된 이름으로 다른 파일·해시·회원에 접근할 수 없다.
- 공통 `ImageMark`는 SSR에서 대체 글자를 먼저 보여주고 hydration 후 이미지를 읽는다.
  디코딩 실패 시 대체 글자로 돌아간다. 로고/사이트 아이콘/아바타/본문 이모지는 형태만 다르다.
- 본인 표시 이름의 알려진 `:shortcode:`만 이미지로 바꾼다. 나머지 문자열은 HTML이 아닌
  이스케이프된 텍스트이며, 이미지에 실패하면 shortcode가 남는다.

## 검증과 재현

전체 검증 기록은 [integration-checks.md](integration-checks.md)를 따른다.
로컬 합성 자료 검사일 뿐 운영 백업의 이미지 완전성이나 실제 AP 재수집 성공은 아니다.

```powershell
# 현재 서버가 내장 migration을 적용했고 FEDKR_MEDIA_DIR가 .local/media의 절대 경로인 상태
./scripts/media-browser-fixture.ps1 -Mode Seed -LegacyShortcodes
npx --yes --package @playwright/cli playwright-cli -s=media open http://127.0.0.1:12239/account --browser msedge
npx --yes --package @playwright/cli playwright-cli -s=media state-load .local/browser-state.json
npx --yes --package @playwright/cli playwright-cli -s=media goto http://127.0.0.1:12239/account
npx --yes --package @playwright/cli playwright-cli -s=media snapshot
npx --yes --package @playwright/cli playwright-cli -s=media run-code --filename scripts/check-media-browser.js --raw
./scripts/media-browser-fixture.ps1 -Mode Clean
```

fixture는 workspace loopback DB만 사용한다. 임의 UUID로 만든 회원·종료 상태 서버·제품과
직접 만든 SVG/1×1 PNG를 넣으며, `-LegacyShortcodes`는 이모지를 직접 만든 1×1 BMP로 바꾸고
특수문자/한글/dot 이름을 추가한다. 스크립트 차단 확인용 SVG 이외 외부 제품 소스는 없다.
실제 회원 인증 우회 endpoint나 격리 해제 기능을 추가하지 않는다.
정리 시 정확한 생성 행과 다른 참조가 없는 생성 파일만 지운다. 운영 ZIP은 열지 않는다.

2026-09-14 최종 release에서 특수 이름/BMP 브라우저 **89개**(1440/390/320px),
합성 Phoenix 이관·파일 보존·활성화·HTTP **127개**, Linux 배포물/HTTP **121개** 통과.
Rust 집중 검사는 중복을 제외한 **27개**이며 전체 239개를 다시 실행한 것은 아니다.
AVIF/TIFF는 고정 시그니처 검사, BMP는 실제 이관·서빙·브라우저 디코딩까지 확인했다.

## 여전히 남은 것

- 실제 DB·파일 export의 동일 시점/완전성 검증과 정식 운영 활성화.
- [로고 관리](catalog-logos.md)와 [AP 아바타·이모지 재수집/새 회원 이미지 저장](profile-media-refresh.md)은 후속 연결했다.
- 탈퇴 시 파일 제거·공유 이모지 보존은 [후속 정리 워커](media-cleanup.md)로 연결했다. 전체 저장소의 무차별 orphan 스캔이나 백업 소거는 하지 않는다.
- 공개 프로필의 명시적 이미지 공개 모델, S3 런타임 어댑터, Linux 배포 권한 검증.

이관 파일을 현재 로컬 reader가 읽는 데 S3 런타임 연결은 필요하지 않지만,
원본 S3 export와 사본을 포함한 백업 운영은 여전히 별도 준비 대상이다.

후속 [활성화 명령](activation.md)의 코드는 연결했다. 승인한 DB는 시작 전에
현재 저장 매핑의 파일을 OpenDAL로 확인한다. 실제 운영 자료의 활성화는 아직 실행하지 않았다.
