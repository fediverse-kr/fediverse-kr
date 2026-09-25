# 계정 생성일 보완 계약

## 목적

ActivityPub actor의 `published`가 없을 때에만 계정 생성일을 보완한다. 생성일을 확인하지 못하면 기존의 180일 서버 등록 자격을 완화하지 않는다. 날짜를 계정 ID, 링크 생성일, 프로필 표시값에서 추론하지 않는다.

## 조회 순서와 동일성

1. WebFinger가 확인한 canonical actor를 ActivityPub으로 조회한다. 유효한 과거 또는 현재 시각의 RFC 3339 `published`가 있으면 그 값을 사용하고 native API를 호출하지 않는다.
2. `published`가 없으면 actor origin에 token-free `POST /api/users/show`를 한 번 보낸다. 요청 JSON은 `{"username":"<local-username>"}`뿐이다. 응답의 `username`은 canonical local username과 일치하고 `host`는 JSON `null`이어야 한다. Misskey 호환 응답에 `uri`가 실제로 포함된 경우에는 canonical actor URL과 정확히 일치해야 한다. Misskey 내부 `id`를 actor URL로 변환하거나 ID에서 시각을 추출하지 않는다. 유효한 RFC 3339 `createdAt`이 과거 또는 현재이면 사용한다.
3. Misskey 응답에 날짜가 없거나 계약상 일치하지 않으면 actor origin의 `GET /api/v1/accounts/lookup?acct=<local-username>`를 한 번 호출한다. Mastodon 응답의 `uri`는 canonical actor URL과 정확히 같아야 하며 `username`과 local `acct`도 일치해야 한다. `url`, display name 또는 임의의 숫자 `id`만으로는 일치시키지 않는다. 유효한 RFC 3339 `created_at`만 사용한다.

모든 날짜는 파싱 실패 및 미래 시각을 거부한다. Mastodon `created_at`은 응답의 RFC 3339 시각(소수 초 포함)을 그대로 보존한다.

## 부하·실패 경계

- ActivityPub 조회가 성공하고 `published`가 있으면 native 호출 수는 0이다. fallback은 한 lookup당 최대 POST 1회 + GET 1회이며 발견 즉시 short-circuit한다. discovery, cascade retry, ID decoding은 없다. 생성일 native lookup 자체는 최대 12초이며, 계정 조회 20초·인증글 확인 30초의 남은 전체 예산보다 길게 기다리지 않는다. 필수 계정/인증글 검증을 먼저 끝내고, 선택적인 날짜 보완의 timeout은 날짜 미확인으로만 처리한다.
- native POST도 기존 HTTPS `safe_url`, pinned public DNS, no-proxy, no-redirect, 10초 timeout, 256 KiB body 경계를 공유한다.
- process 내 cache는 계정별 positive(1시간)와 negative/unsupported(10분)를 보존한다. 생성일 보완 내 동일 origin native 작업은 1개, process-wide native 작업은 4개로 제한한다. origin gate map은 256개이며, 사용 중이거나 cooldown인 항목을 새 gate로 바꾸어 제한을 우회하지 않는다. 429는 origin 30초 cooldown을 건다. linked account에 성공한 값은 `member_linked_accounts.account_created_at`와 검증 시각에 영속화된다.
- native 실패는 로그인/연동 자체를 실패시키지 않는다. 429, 5xx, timeout은 날짜를 모르는 결과이며, unsupported 응답은 bounded negative 결과다.

## 인증된 기존 계정 재확인

`POST /api/member/sites/registration/recheck`만 owner-scoped mutation을 수행한다. CSRF/Origin 및 session 검사를 거친 뒤 database-backed rate limit과 network slot을 적용하고, 해당 회원의 생성일 `NULL` linked account를 SQL에서 최대 4개까지만 선택한다. 각 대상은 WebFinger와 canonical AP actor를 새로 확인하고 `published` → Misskey → Mastodon 순서를 동일 적용한다. handle이 다른 actor로 바뀌면 보완하지 않는다. 각 결과는 network 전후에 member/account/actor/handle을 재검증하고, 여전히 같은 row이며 날짜가 비어 있을 때만 날짜와 `account_created_at_verified_at`을 기록한다. 이미 날짜가 있거나 삭제·foreign·stale row이면 identity/profile/owner 필드를 덮어쓰지 않는다.

일반 `GET /api/member/sites/registration`은 eligibility를 읽기만 하며 native mutation을 시작하지 않는다. 재확인 실패는 계정 로그인과 연동 성공을 되돌리지 않는다.

## DB migration

`202609260027_account_age_retry_rate`는 기존 rate-limit `scope` CHECK에 `account_age_retry`만 추가한다. 기존 scope와 제한은 유지한다. 회원별 15분당 4회, 전체 15분당 40회를 DB에 보존하며 재시작/재접속으로 초기화하지 않는다. 일반 POST 완료(200)는 등록 자격 승인이라는 뜻이 아니며, 이후 eligibility GET이 여전히 180일 조건을 판정한다. downgrade는 이 scope의 상태가 남아 있으면 거부하고, 만료·정리 후에만 허용한다.

## 근거

- ActivityPub actor vocabulary: <https://www.w3.org/TR/activitystreams-vocabulary/#dfn-published>
- Misskey `users/show`: <https://misskey-hub.net/docs/api/users/show/> (local user lookup uses `username`; `host` is nullable and `createdAt` is the account creation field)
- Mastodon accounts lookup: <https://docs.joinmastodon.org/methods/accounts/#lookup>
- Mastodon Account entity: <https://docs.joinmastodon.org/entities/Account/> (`uri` is the ActivityPub actor URI; `created_at` is the account creation timestamp)

Contract snapshot: 2026-09-26 (UTC).
