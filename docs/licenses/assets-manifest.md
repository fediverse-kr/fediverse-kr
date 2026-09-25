# 배포 자산 고지

이 문서는 Cargo 패키지 전체 고지와 별도로, 실제 웹 배포물에 들어가는
비-Cargo 자산과 아이콘 데이터의 출처를 기록한다. 해시는 이 checkout의
현재 파일을 SHA-256으로 대조하기 위한 값이다.

## 아이콘 데이터

- `src/demo_icons.rs`에서 사용하는 Heroicons 1.0.6, Ionicons 6.0.2,
  Bootstrap Icons 1.8.3은 `dioxus-free-icons` 0.10.0의 고정 데이터다.
  원문 고지는 [`icon-pack-notices.md`](../icon-pack-notices.md)에 보존한다.
  고지 파일 SHA-256: `ce98541a170e9f41e1ad04c753273d8368f82758c4d9f5ac29ef50310fb031f2`.
- `dioxus-icons` 0.1.0의 Lucide 컴포넌트는 실제 생성 주석의 Lucide
  v1.14.0 commit `56e49f12166312f04af4cfd862621c93cf583979`를 따른다.
  crate source는 [dioxus-icons 0.1.0의 upstream commit](https://github.com/dioxuslabs/dioxus-icons/tree/103fe30e2dd763da8a82d493f2d38f4096c63a02/crates/dioxus-icons)이다.
  ISC 및 Feather 유래 MIT 원문은
  [`lucide-icons.ISC-MIT.txt`](assets/lucide-icons.ISC-MIT.txt)에 둔다.
  원문 SHA-256: `b495047bd93a9b06913511076f504daba17d5bbeb3e0650f3bb53a4220329c57`.
- `lucide-dioxus` 3.26.0은 [Rust for Web commit](https://github.com/RustForWeb/lucide/tree/faaf58845b37f06af77bde4e12790a5afb032a7c/packages/dioxus)
  `faaf58845b37f06af77bde4e12790a5afb032a7c`의 MIT crate다. 이 crate는
  upstream Lucide 아이콘 데이터 revision을 별도로 기록하지 않으므로,
  그 revision을 추정하지 않는다. 해당 crate의 MIT 고지는 Cargo 의존성
  고지 생성물에서 처리한다.

## 글꼴·생성 자산

- `assets/fonts/PretendardVariable.woff2`는 Pretendard Variable 1.3.9다.
  원본 URL·저작권·OFL 1.1 전문은 [`../../assets/fonts/README.md`](../../assets/fonts/README.md)
  및 [`../../assets/fonts/OFL.txt`](../../assets/fonts/OFL.txt)에 있다.
  OFL 원문 SHA-256: `d31ddd9f2bed32fd7e302a205cf2380ba0de6529152d239ef99cfb6f261bfc04`.
  WOFF2 SHA-256: `9599f12fd42fc0bce1cd50b47a0c022e108d7aa64dd0d1bb0ed44f3282d900b4`.
- `assets/tailwind.css`는 파일 첫 줄의 Tailwind CSS 4.1.5 생성 고지를
  유지한다. 공식 MIT 원문은 [`tailwindcss.MIT.txt`](assets/tailwindcss.MIT.txt)에
  보존한다([upstream source](https://raw.githubusercontent.com/tailwindlabs/tailwindcss/v4.1.5/LICENSE)).
  MIT 원문 SHA-256: `60e0b68c0f35c078eef3a5d29419d0b03ff84ec1df9c3f9d6e39a519a5ae7985`.
  CSS SHA-256은
  `ea210276e4b2cc0eb8c3ca00cdfc06b9ee9177f68125b744ceb5524b63da53ff`다.
- `assets/images/neighborhood.png`, `shuna.png`, `patricia.png`는 2026-09-08
  내장 이미지 생성 도구로 만든 허구의 시연 자산이다. 프롬프트와 원본 보존
  사실은 [`../../assets/images/README.md`](../../assets/images/README.md)와
  [`../../assets/images/prompts.md`](../../assets/images/prompts.md)에 기록한다.
  제3자 사진·상표·실존 인물의 출처나 별도 라이선스를 주장하지 않는다.
  SHA-256: `neighborhood.png` `926bcd27c47024b5ebf3f5b66059060ac92fa03a997dc7a7ced187da3d939892`,
  `shuna.png` `a3c8b7e1e627f74905cf76b4e4726ca4ca98ad047898df600ac7e8693f453104`,
  `patricia.png` `35ff012a150b5229a6143427a7cd8942cda223d409c964518d4817955af8206b`.
- `assets/fediverse-kr-mark.svg`는 favicon과 사이트 wordmark에 쓰는 자체 제작
  브랜드 자산이다. 외부 이미지·글꼴·SVG 경로를 포함하지 않는, 두 개의 보라색 별과
  하나의 연속된 기울어진 라일락 행성 띠 mark다. SHA-256:
  `cfad2c91ed2aa0730e331753ac8bf9ea0656dbb0876c21f7128274c046b829c6`.

## 추가 생성 아바타

`assets/images/golden.png`, `film.png`, `soup.png`는 2026-09-17 프로젝트의
허구 시연 프로필용으로 요청해 GPT `gpt-image-2-medium`으로 생성한 동물 아바타다.
현재 파일은 도입 커밋 `7a221383e6dc0de8237b2028c2249f9b8b39610e`와 동일하다.
생성 기록의 범위·한계와 현재 SHA-256은 [이미지 문서](../../assets/images/README.md)에 있다.
세 파일에는 prompt/C2PA metadata가 없고 전체 prompt 및 원본 생성 캐시는 복구되지 않았다.
생성 출처가 저작권 성립이나 서비스 약관에 대한 법적 보증이라는 주장은 하지 않는다.

## Dioxus 복사·수정 소스 및 scaffold 자산

출처는 [DioxusLabs/components](https://github.com/DioxusLabs/components/tree/bf007c15d0cf4d04d3181cc46cf12325aa773955)
commit `bf007c15d0cf4d04d3181cc46cf12325aa773955`다. Cargo 의존성 고지와 별도로
저장소에 복사된 소스에도 upstream 저작권 및 MIT OR Apache-2.0 조건이 적용된다.
이 복사본에는 upstream MIT를 선택하며 [원문 고지](assets/dioxus-components.MIT.txt)를 보존한다.

- upstream `preview/src/components/` → 로컬 `src/components/`의
  `badge`, `button`, `card`, `sheet`, `tooltip` 범위다.
- `badge`, `button`, `card`, `tooltip`의 `component.rs`, 다섯 컴포넌트의
  `style.css`, `sheet/mod.rs`는 해당 upstream 파일과 동일하다.
- `badge/mod.rs`, `button/mod.rs`, `card/mod.rs`, `tooltip/mod.rs`,
  `sheet/component.rs`, 상위 `src/components/mod.rs`는 프로젝트에 맞게 수정되었다.
  수정 사실을 명시하며 upstream 전체와 동일하다고 주장하지 않는다.
- `assets/header.svg`는 upstream `test-harness/assets/header.svg`와 동일하다.
  SHA-256: `0884ed70b333d1ef4400c3ad2e8e3c3626ff44698e99345311e3d9625920ef5c`.
- 이력의 `assets/favicon.ico` 역시 같은 upstream test-harness 자산이다.
  현재 favicon은 위의 자체 제작 `fediverse-kr-mark.svg`를 사용한다.

## 그 밖의 프로젝트 자산

`assets/explainer-scroll.js` 및 프로젝트 스타일시트는 로컬 구현이다.
이 문서는 전체 인터넷 유사도 검사나 모든 행의 기원에 대한 보증을 제공하지 않는다.
프로젝트 라이선스는 별도 표기된 폰트·아이콘·Dioxus·Phoenix 등 제3자 조건을 덮어쓰지 않는다.
