# 연합 계정 인증과 계정 연동

## 확정 방향

- 첫 로그인은 연합 계정 소유 인증. 자체 ID·비밀번호는 로그인한 회원이 필요할 때 추가한다.
- 이후 여러 연합 계정을 같은 내부 회원 UUID에 연결한다. 공개 소개 여부는 별도 설정이고 기본 비공개다.
- 인증은 사용자가 자기 계정에 **새 공개 인증글을 작성**한 뒤 확인하는 흐름이다. OAuth/MiAuth 및 공개 프로필 코드 인증으로 바꾸지 않는다.
- 최초 인증, 재로그인, 추가 계정 연결은 같은 AP 조회 모듈을 쓰되 DB challenge의 목적과 세션 결합은 다르다.
- 가입 180일 조건은 로그인에 붙이지 않는다. 향후 서버 등록 같은 행위 권한으로 분리하며 생성일이 없으면 오래된 계정이라고 추정하지 않는다.
- 서버 추천은 포팅 대상이 아니다.

## 이번 구현

`src/backend/federation/`는 회원 DB와 독립된 읽기 전용 클라이언트다.

| 모듈 | 책임 |
|---|---|
| `webfinger.rs` | 핸들·IDN 파싱, WebFinger subject 일치 확인, AP self 링크 선택 |
| `activitypub.rs` | 서명 actor 조회, outbox/페이지/Note 조회, 공개 인증글 검증 |
| `http_signature.rs` | 외부 주입 RSA 키로 `(request-target) host date` GET 서명 |
| `transport.rs` | HTTPS, 공용 IP 검증과 DNS 핀 고정, 리디렉션 금지, 시간·본문 상한 |
| `mod.rs` | 조회·검증 API와 오류, 외부에서 임의 생성할 수 없는 `VerifiedIdentity` |
| `identity.rs` | 회원 서비스용 클라이언트 어댑터 |

공개 계약:

```rust
FederationClient::new(transport)
client.resolve_account(handle).await -> Result<ResolvedActor, FederationError>
client.verify_public_post(&actor, code, issued_at).await
    -> Result<VerifiedIdentity, FederationError>
```

`ResolvedActor`는 소유 인증 결과가 아니다. `VerifiedIdentity`도 로그인 세션 자체가 아니다. 회원 서비스가 해당 challenge를 DB에서 원자적으로 소비한 다음에만 회원/연결/세션을 만든다.

## 검증 경계

1. WebFinger `subject`가 입력 핸들과 일치해야 한다. 서버와 계정 도메인이 다를 수 있으므로 WebFinger가 가리킨 다른 HTTPS 서버의 actor는 허용한다.
2. actor 문서 `id`는 실제 조회 URL, `preferredUsername`은 핸들의 사용자명과 일치해야 한다. 반환된 outbox는 actor와 같은 origin이어야 한다.
3. 검증 시 actor를 다시 조회한다. 브라우저가 보낸 outbox, 계정 URL, `verified: true`를 신뢰하지 않는다.
4. 최근 공개 `Note` 또는 해당 계정이 작성한 `Create(Note)`만 인정한다. `Announce`, 다른 작성자, 복수 작성자, private 글, 게시일 누락, 인증 요청 이전 글, 미래 글은 제외한다.
5. `attributedTo`와 Create의 `actor`가 정확히 actor ID와 일치해야 한다. Note ID 또한 같은 origin이어야 한다. Note의 `to` 또는 `cc`에 전체 공개 URI가 있어야 한다.
6. HTML을 토큰화한 본문의 독립된 코드 토큰을 비교한다. 태그 속성·주석에 들어 있는 코드는 인정하지 않고 script/style/template 및 숨김 속성이 있는 글은 인증 후보에서 제외한다. 원격 HTML을 화면에 실행하거나 저장하지 않는다.
7. 검증 수명 최대 30분, 최대 3 collection/page, 30 item, 12 object 역참조. 각 HTTP 요청 10초·256 KiB, 전체 계정 조회 20초·글 검증 30초 이내다.

공개 조회는 계정이 속한 서버의 HTTPS 응답을 권위로 삼는다. 원격 운영자가 자기 서버의 계정/글 데이터를 조작할 수 있다는 연합 시스템의 신뢰 한계를 지우지 않는다.

## 네트워크 안전

- HTTPS 443만 허용하고 IP literal, userinfo, localhost/내부용 suffix, 특수주소를 거부한다.
- 요청 직전 A/AAAA 결과를 모두 확인하고 하나라도 사설·loopback·link-local·문서용·multicast 등이 있으면 실패한다.
- 검증한 주소를 실제 연결에 핀 고정한다. TLS 인증서와 SNI 검증은 원래 hostname으로 유지한다. 환경변수 proxy는 사용하지 않는다.
- 3xx는 따라가지 않는다. actor→outbox→page→Note 각 URL을 다시 검사한다. HTTP 승인서버로의 downgrade도 없다.
- 이 모듈은 GET만 한다. 실제 외부 계정에 로그인하거나 글을 게시·수정·삭제하지 않는다. 개발 검증은 mock transport로 한다.

## 통합된 회원 서비스 계약

- 익명 시작 challenge는 난수 browser nonce에, 추가 연결은 현재 회원·세션·purpose에 결합한다. 코드와 nonce는 해시 저장, URL/로그에 노출 금지.
- 익명 인증 성공 시 canonical actor가 이미 연결되어 있으면 그 회원으로 로그인한다. 기존 회원의 예약 주소라면 [새 공개 인증글을 거쳐 옛 UUID에 최초 고정](legacy-member-login.md)한다. 어느 쪽에도 해당하지 않을 때 새 내부 회원을 생성한다.
- 추가 연결은 canonical actor UNIQUE와 트랜잭션으로 다른 회원에게 귀속된 계정 탈취/중복 연결을 막는다. 핸들은 변경 가능한 표시값이지 소유권 키가 아니다.
- 조회 후 `UPDATE ... WHERE consumed_at IS NULL AND expires_at > now() ... RETURNING`과 연결/로그인 작업을 원자적으로 묶는다. 실패한 proof를 성공으로 표시하지 않는다.
- 자체 로그인 등록/변경, 연결 해제는 최근 소유 인증 또는 현재 비밀번호 재확인과 CSRF 검증을 요구한다. 자체 비밀번호가 아직 없는 회원을 잠그거나 마지막 로그인 수단을 제거하면 안 된다.
- 지속 RSA 키의 생성·보관, `/actor#main-key` 공개키 제공은 서버 설정/DB 계층이 맡는다. 매 재시작마다 다른 키를 생성하지 않는다. 개인키는 로그·브라우저·저장소에 넣지 않는다.

## 기존 Phoenix와의 차이 / 후속 작업

참조 소스 `fedkr-ref/lib/fediverse_kr/accounts/{webfinger,activitypub}.ex` 및 `federation/http_signature.ex`를 확인했다. 참조 저장소 라이선스는 **MIT**다.

- 기존 actor 조회는 서명 AP 요청이 401/403인 경우 Mastodon REST → Misskey REST fallback을 사용했다. 게시물 조회는 AP 실패 시 Mastodon REST → Misskey REST 순서였다.
- 이번 사용자 지정 범위는 **인증글 AP 조회**다. REST fallback과 프로필 인증은 아직 포팅하지 않는다. AP 접근이 거절된 서버는 성공으로 위장하지 않고 명시적으로 실패한다.
- 기존의 문자열 포함 검사보다 작성자·공개범위·작성시간 검증을 강화했다. actor와 outbox의 origin이 다른 특수 구성, 리디렉션이 필요한 구성은 현재 지원하지 않는다.
- 전체 AP inbox/delivery와 자동 계정 이동 병합은 범위 밖이다. 이후 구현으로 기존 연동 계정의 새 공개 인증글에 의한 암호 복구, 구 회원의 첫 로그인 연결을 통합했다. 실데이터 이관·활성화는 별도 검증이 남아 있다.

참고 표준: [WebFinger RFC 7033](https://www.rfc-editor.org/rfc/rfc7033), [ActivityPub](https://www.w3.org/TR/activitypub/). Mastodon/Misskey 제품 소스를 복사하지 않았다. 새 직접 의존성은 permissive 라이선스만 채택하며 resolved dependency tree도 확인한다.

## 초기 단독 모듈 검증 기록

아래는 본체 통합 전 기록이다. 현재는 지속 서명키/공개 actor endpoint/회원 challenge
트랜잭션/HTTP 라우터에 연결되어 있다. 후속 전체 검증은 [integration-checks.md](integration-checks.md)를 따른다.

- 본체 Cargo/라우터/DB를 변경하지 않고 `output/identity-verification` 임시 하네스로 실제 federation 모듈을 컴파일했다. mock 전 과정·SSRF 필터·RSA 서명 검증 **11 passed / 0 failed**. 실제 계정/서버에는 요청하지 않았다.
- 라이선스 검토 결과 CSS selector 의존성이 필요 없는 `html5ever` 토크나이저를 사용한다. 해결된 의존성에 GPL/AGPL/MPL 전용 라이선스는 없다. `r-efi`는 `MIT OR Apache-2.0 OR LGPL-2.1-or-later` 중 MIT를 선택한다.
- 본체 통합 시 server-only 의존성으로 `reqwest 0.12` (`default-features = false`, `rustls-tls`, `json`), `rsa 0.9`, `base64 0.22`, `html5ever 0.35`를 추가하고 `sha2`의 `oid`, `tokio`의 `net` feature를 활성화한다. `backend::federation` 모듈 선언도 필요하다.
- **현재 이 모듈만으로 웹사이트 로그인 API가 동작하는 상태는 아니다.** 지속 서명키/공개 actor endpoint/회원 challenge 트랜잭션/HTTP 라우터의 연결은 회원 서비스 통합 작업으로 남아 있다. API 연결 전후 통합 테스트와 실제 운영 도메인의 authorized-fetch 상호운용 확인이 추가로 필요하다.

## 참조 코드 라이선스 고지

MIT License

Copyright (c) 2025-2026 Fediverse.kr contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
