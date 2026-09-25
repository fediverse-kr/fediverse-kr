# 기존 파일 보존 묶음 — 2026-09-14

`--legacy-assets`는 격리 이관 DB가 참조하는 파일을 별도 디렉터리에 보존한다.
운영 활성화, HTTP 이미지 제공, 파일 내용의 안전성 판정은 하지 않는다.
원본 파일을 새 PG의 bytea에 넣거나 새 컨테이너를 추가하지도 않는다.

## 원본 계약

MIT Phoenix `3323c7b`의 `storage.ex`, `accounts/avatar_fetcher.ex`,
`servers/favicon_fetcher.ex`, 소프트웨어 관리자 업로드 경로를 확인했다.
원래 저장소는 로컬 또는 S3이며, DB에는 파일 내용 대신 저장 키가 있다.

- 회원 `avatar_key` → `avatars/…`
- 회원 `emojis`의 모든 값 → `emojis/도메인/이름.확장자`
- 서버 `favicon_key` → `favicons/…`
- 소프트웨어 `logo_key` → `software-logos/…` (`software/…`도 지원)

이모지는 `{shortcode: storage_key}` 구조다. 이전 합성 fixture에 있던 URL 객체를
실제 저장 계약에 맞게 수정했고, importer 참조 수에도 이모지를 포함했다.
NULL/빈 맵은 유효하지만 배열·중첩 객체·문자열이 아닌 값은 누락으로 처리하지 않고 거절한다.
DB가 참조하지 않는 원본 파일은 이 도구의 수집 대상이 아니며, 원본 전체 백업에서 유지한다.
외부 avatar_url/thumbnail_url은 DB에 보존되지만 이 명령이 URL을 다운로드하지는 않는다.

## 준비와 실행

1. 원본 DB와 같은 백업 시점의 **전체 파일 저장소 export**를 비공개 디렉터리에 준비한다.
   S3에서 받는 과정은 별도다. 이 도구는 버킷에 접속하거나 자격증명을 받지 않는다.
2. DB는 [격리 importer](legacy-import.md)로 적용·검증된 `fedkr_rehearsal_*`여야 한다.
   운영·개발 DB, 원본 snapshot, 완료 표시가 없는 대상은 거절한다.
3. 원본 export와 출력 디렉터리는 각각 이미 존재하는 절대 경로로 지정한다.
   서로 같거나 한쪽이 다른 쪽 안에 있으면 안 된다. 웹 공개 디렉터리를 사용하지 않는다.
4. OS 권한으로 원본·출력·격리 PG를 보호한다. 원본은 작업 내내 고정해야 하며,
   다른 프로그램의 쓰기나 디렉터리 교체를 허용하지 않는다. 개발용 trust PG에
   실제 회원 데이터를 넣지 않는다.

```text
FEDKR_IMPORT_TARGET_URL  -> verified isolated loopback fedkr_rehearsal_* database
FEDKR_ASSET_SOURCE_DIR   -> absolute private original storage export
FEDKR_ASSET_BUNDLE_DIR   -> separate absolute private output directory

fediversekr2 --legacy-assets
fediversekr2 --legacy-assets --apply
```

기본 명령은 파일을 읽어 검증하되 파일 복사·색인 행 저장은 하지 않는다.
오래된 격리 DB에는 새 내장 마이그레이션의 빈 테이블/이력이 추가될 수 있다.
`--apply`만 `objects/<sha256>`에 파일을 복사하고 논리 키→해시/길이를 비공개 PG에 저장한다.
최종 복구에는 **DB와 이 파일 디렉터리 둘 다** 필요하다. 디렉터리 단독으로 파일의
원래 역할이나 회원 귀속을 복구할 수 있는 형식은 아니다.

활성화 후 `FEDKR_MEDIA_DIR`로 쓰는 묶음은 운영 저장소이므로 탈퇴 정리에 따라 바뀐다.
활성화 전 독립적인 DB/파일 백업을 남긴다. 원본 export와 이관 inventory는 정리 워커의
삭제 대상이 아니다. [운영 파일 수명주기](media-cleanup.md).

## 검증·실패 의미

- 원본 이관 지문과 보존된 8종 테이블 전체를 다시 대조한 뒤 파일을 읽는다.
  회원 연결·projection·새 편집 상태도 확인하므로 변형된 대상에 파일만 덧씌우지 않는다.
- 256개씩 저장 키를 조회한다. 여러 필드가 같은 키를 참조하면 한 번 읽는다.
  서로 다른 키의 내용이 같으면 물리 파일 하나를 공유하지만 키 매핑은 모두 보존한다.
- 파일 바이트와 SHA-256·길이를 비교한다. SVG·빈 파일·알 수 없는 형식도 **원본 그대로**
  보존한다. 이는 브라우저에서 안전하다는 뜻이 아니다. 파일당 16 MiB를 넘으면
  건너뛰거나 자르지 않고 전체 색인 작업을 거절한다.
- 경로 이탈·예약 이름·Windows ADS·심볼릭 링크/정션을 거절한다. Windows에서는 ASCII
  대소문자만 다른 저장 키도 충돌로 거절한다. 이런 S3 키가 있다면 이름을 바꾸지 말고
  대소문자를 구별하는 Linux 파일시스템에 다시 export하여 그 환경에서 보존한다.
  이 검사는 악의적인 로컬 관리자의 동시 경로 교체나
  모든 파일시스템의 Unicode/별칭 동작을 통제한다는 보장이 아니다. 고정된 비공개 export가 전제다.
- 파일은 새 임시 파일에 기록·동기화한 뒤 hard link로 원자적으로 등록한다. 기존 파일을
  덮어쓰지 않는다. hard link를 지원하지 않는 저장 매체에서는 실패한다.
- 같은 importer advisory lock으로 동시 실행을 직렬화한다. 색인·완료 표시는 한 트랜잭션이다.
  파일 중간에 실패하면 DB는 롤백하지만 이미 복사된 내용 주소 파일은 남을 수 있다.
  재실행은 내용이 정확히 같은 파일만 재사용한다. filesystem까지 롤백됐다고 주장하지 않는다.
- 이미 완료된 경우 원본 파일·DB 색인·출력 파일을 모두 재대조하며 자동 수리하지 않는다.
  COMMIT 응답이 끊겨 결과가 불확실해도 같은 입력으로 재검증할 수 있다.
- CLI는 건수와 고정 오류만 출력한다. 회원/도메인/저장 키/원문 DB 오류·지문을 출력하지 않는다.

보고서의 `references`는 필드별 참조 수, `objects`는 서로 다른 저장 키 수,
`bytes`는 키별 길이 합계(같은 내용의 서로 다른 키는 각각 계산), `copied_objects`는
이번 실행에서 새로 만든 물리 파일 수다. `runtime_enabled`는 항상 false다.

이후 `--legacy-import` 재실행은 보충 색인의 키 집합·건수·길이 합계·원본 지문을 확인한다.
그 명령만으로 파일 바이트까지 검증하지는 않는다. 파일 재검증에는 `--legacy-assets`가 필요하다.

## 남은 전환 작업

후속: [런타임 이미지 조회](media-serving.md)를 연결했다. 같은 checked reader로
현재 참조/권한을 확인하며, 이전 색인에서 migration 018의 런타임 매핑을 만든다.
`--legacy-assets` 자체의 오프라인·비공개 계약과 이관 DB 격리 차단은 유지한다.
후속 [명시적 활성화](activation.md)는 검토된 별도 이름의 사본만 허용한다.
이 명령 자체로 승인하거나, 승인한 DB를 다시 보존 단계로 되돌리지는 않는다.

- 실제 암호화 DB와 로컬/S3 파일 백업의 동일 시점·완전성 검증.
- 실제 운영 파일 제공 검증, 로고 업로드·아바타 재수집, 탈퇴/공유 이모지 참조 정리.
- 파일 색인과 원본/서비스 identity를 검증한 뒤의 운영 활성화 경로.
- Linux 실행 및 실제 배포 파일시스템·보관 권한 확인.

이 기능은 파일 보존 경로를 마련한 것이지 Phoenix 대체 완료가 아니다.
