# ActivityPub actor/inbox 호환 — 2026-09-14

MIT Phoenix 원본의 `ActorController`는 `GET /actor`에서 Application actor와 공개키,
`/actor/inbox` URL을 알리고, `POST /actor/inbox`에는 활동을 저장하거나 처리하지 않고
빈 `202 Accepted`만 돌려준다.

현행도 actor ID와 공개키 ID/owner를 `FEDKR_PUBLIC_ORIGIN`에서 그대로 만들고, 원본처럼
`preferredUsername`에는 제품 상수가 아니라 origin의 host를 넣는다. actor JSON은
`inbox`를 `<origin>/actor/inbox`로 광고한다.

`POST /actor/inbox`는 익명 원격 요청을 cookie·Origin·회원 CSRF 검사와 분리해 받는다.
`application/json`, `application/activity+json`, ActivityStreams profile의
`application/ld+json`을 허용하되, 실제 바이트를 64 KiB와 5초 안에서만 읽고 곧바로
빈 202로 끝낸다. 동시에 읽는 익명 본문은 64개로 제한하며 포화 시 본문을 읽지 않고
429로 거절한다. 모든 inbox 응답은 `Cache-Control: no-store`이고 cookie를 설정하지
않는다. POST 외 exact path 요청은 SSR로 넘기지 않고 `405 Allow: POST`로 끝낸다.
본문을 파싱·저장·출력·로그·서명 검증·외부 전송하지 않는다. 다른 타입은 415,
초과 본문은 413, 읽기 시간 초과는 408이다.

이는 Phoenix의 endpoint 계약만 복구한다. 일반 ActivityPub inbox, delivery, queue,
follower state, 서명 검증은 구현하지 않으며 실제 원격 호환 검증도 아직 하지 않았다.

## 확인 경계

2026-09-14 공개 `https://fediverse.kr/actor`의 **GET만** 읽기 전용으로 확인했다.
`200 application/activity+json`, `Application`, actor ID, inbox URL,
`preferredUsername=fediverse.kr`, 공개키 ID/owner가 원본 계약과 일치했다.
키 본문은 기록하지 않았고 실제 inbox POST, 서명 검증, 원격 ActivityPub 성공은
실행하거나 주장하지 않았다.
