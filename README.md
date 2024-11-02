# fediverse-kr

https://fediverse.kr (dead link, not yet)

여기저기 흩뿌려져있는 연합우주 정보를 한 곳에 모으고, 안내 역할을 담당할 수 있도록 구성하고 있습니다.

준비중

## todo

- [x] 기본 레이아웃
- [ ] 기본 정보 채우기 (주요 정보 링크 포함)
- [ ] Dockerfile 작성, CI 구축
- [ ] 배포
- [ ] 기여 가이드
- [ ] 고도화
    - [ ] 마크다운 파일에서 문서가 생성될 수 있게 추가
    - [ ] DB 연계, 서비스단에서 정보를 추가할 수 있게 구성

## dev

prepare env:

- rust
    - `cargo install cargo-leptos`
    - `cargo install leptosfmt`
- node.js
    - `pnpm install`

dev:

- `cargo leptos watch`
    - unocss를 우회적으로 사용하고 있어서 CSS가 일부 반영되지 않는 경우가 있습니다. 침착하시고 새로고침을 눌러주세요. (`app/build.rs` 에서 빌드스크립트로 실행)

deploy

- TODO

## license

MIT