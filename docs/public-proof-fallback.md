# 공개 인증글 조회 호환성

## 기본 경로와 제한적 fallback

계정 본인인증은 WebFinger → canonical ActivityPub actor → **그 계정의 outbox** 순서가 기본이다. outbox 탐색이 정상 종료됐지만 인증글이 없을 때(`ProofNotFound`)만 Misskey 호환 읽기 API로 후보를 찾는다. 계정 생성일 보완 및 서버 운영자 인증과는 별개다.

- outbox 성공 시 native 인증글 조회는 0회다.
- AP 401/403/429, timeout, 잘못된 outbox, 페이지·항목 검색 한도 초과는 fallback으로 우회하지 않는다.
- native API는 canonical actor origin의 `users/show` 1회와 `users/notes` 1회만 호출한다. 사용자·local host·제공된 actor URI를 검사하고, `withCats:false`, 최대 20개, renote/channel 제외를 명시한다. 이 옵션은 프로필 `isCat`를 바꾸지 않는다.
- 정확한 native 작성자, public visibility, `localOnly:false`, 인증 문구를 만족하는 후보만 검토한다. 응답이 준 임의 URL 대신 제한된 영숫자 note ID를 해당 origin의 `/notes/<id>`로 구성한다.
- 최대 3개 canonical AP Note를 조회하며 ID·작성자·Public 수신자·문구 경계·발행 시각을 **기존 AP 증명 검증기로 다시 확인**한다. native 응답만으로 로그인 증명을 발급하지 않는다. 삭제된 개별 후보는 다음 후보를 가리지 않는다.
- 기존 challenge의 코드/발급 시각 해시 바인딩, 만료·일회성 소비 및 DB 원자성 검증을 유지한다. 새 인증글 작성이나 계정 프로필 변경은 구현/진단의 전제가 아니다.

## 부하와 안전 경계

- 최대 추가 요청: native 2회 + canonical AP Note 3회. 재시도나 native 페이지 순회 없음.
- fallback 전체 최대 12초이며, 필수 계정/공개글 확인 전체 30초 예산 안에서만 실행된다.
- HTTPS·pinned public DNS·no proxy/no redirect·개별 요청 10초·응답 256KiB 경계를 공유한다.
- 인증글 fallback 내 process-wide 동시성 4개, origin별 1개. 최대 256개 origin gate를 보유하며 사용 중/cooldown 상태를 새 gate로 바꾸지 않는다. 기존 인증 endpoint의 영속 challenge rate limit도 유지한다.
- 429는 즉시 중단하고 origin 60초 cooldown을 둔다. API 미지원(404/405/501 또는 잘못된 JSON)은 10분 negative cache를 둔다. **인증글 미발견 자체는 cache하지 않는다.** 사용자가 방금 올린 글의 다음 확인을 막지 않기 위해서다.
- 생성일 보완의 부하 제한은 별도다. AP 증명이 먼저 성공한 뒤 남은 시간에만 선택적으로 생성일을 보완하며, 보완 실패가 이미 성공한 인증을 취소하지 않는다.

## 호환성 근거와 한계

CherryPick 4.17.0의 AP outbox fanout 호출은 `withCats:true`를 고정한다. 해당 옵션의 predicate는 cats-only이며, cache 최신 구간과 DB fallback의 필터가 다르다. native 읽기 후보 탐색은 이 비대칭을 우회하는 제한적 호환 경로이지 upstream 결함 수정은 아니다.

- 영향 소스: [CherryPick 4.17.0 고정 revision](https://github.com/kokonect-link/cherrypick/blob/d099587187ea49651f52e08baf05c9352266a8ed/packages/backend/src/server/ActivityPubServerService.ts#L504-L537)
- hardcode 도입: [CherryPick e977977](https://github.com/kokonect-link/cherrypick/commit/e97797767c88e5f1bcfc3d60c1cef3de458a7908)
- 기반 FTT outbox 변경: [Misskey b2e3e65](https://github.com/misskey-dev/misskey/commit/b2e3e658965b52873dd6771cf9771b3032a0ed15). 이 변경 자체에는 `withCats`가 없다.

확인한 upstream Misskey에는 동일 cats-only 결함이 없었다. 원격 서버의 실제 바이너리·Redis·DB 내부 또는 과거 실패한 특정 challenge를 조사한 것은 아니므로, 소스 분석과 회귀 테스트를 과거 로그인 성공의 입증으로 간주하지 않는다.
