# 배포 고지 생성

라이선스 선택·SPDX 식 처리는 [cargo-about](https://embarkstudios.github.io/cargo-about/)
0.9.2에 맡긴다. `about.toml`은 기존 permissive 선택지를 허용하며 GPL/LGPL/AGPL을
허용 목록에 넣지 않는다. `OR` 선택과 `AND` 의무를 직접 파싱하는 대체 구현은 없다.
앱 자체는 crates.io 게시 대상이 아니므로 `publish=false`이고, 앱의 소스 라이선스를
이 설정으로 정하지 않는다. GitHub 공개 여부와도 별개다.

## 도구와 입력

공식 [0.9.2 릴리스](https://github.com/EmbarkStudios/cargo-about/releases/tag/0.9.2)의
해당 호스트용 실행 파일을 별도 작업 도구 경로에 준비한다. 전역 설치나 앱 의존성 추가는
필요 없다. 다운로드한 릴리스의 SHA-256을 공식 릴리스 메타데이터와 비교한다.
이번 Windows 도구 archive는 다음과 같았다(실행 파일 해시와는 다름).

```text
cargo-about-0.9.2-x86_64-pc-windows-msvc.tar.gz
1c03e5890238562497c2d89a3b75b02560af349c1fc3e713d3284f532a5cd748
```

Python 3.11 이상과 해당 lock의 Cargo 캐시가 필요하다. `--offline --locked`이므로
캐시가 부족하면 실패하며, 자동으로 의존성을 갱신하거나 네트워크로 보충하지 않는다.
서버 feature와 web feature를 별도로 분석한다. 배포하지 않는 desktop/mobile 또는
모든 feature의 합집합으로 라이선스를 판정하지 않는다.

```sh
python3 -B scripts/generate-third-party-notices.py \
  --cargo-about /absolute/cargo-about \
  --profile linux-server \
  --output-dir /absolute/new-notice-directory
```

`linux/aarch64` 서버 검토용은 `--profile linux-aarch64-server`로, Windows 서버
검토용은 `--profile windows-server`로 바꾼다. 각 profile에는 해당 서버 target과
`wasm32-unknown-unknown`/web 두 그래프가 들어간다. 출력 경로는 새 디렉터리여야 한다.

## 원문 보강

도구가 성공 종료해도 `source_path=null`이면 패키지의 실제 저작권자가 빠진 표준 라이선스
본문일 수 있다. `docs/licenses/supplemental.json`에서 정확한 패키지 이름·버전·선택된
라이선스별로 원문 파일을 연결한다. 패키지에 실린 VCS revision 또는 lock의 git revision을
근거로 고정된 upstream URL과 보존 파일의 SHA-256을 기록한다. 버전 추정이나 저작권 문구
재작성은 하지 않는다. components/sdk의 MIT 문구는 Dioxus 공통 파일과 같으며 EOF 개행만
추가되었다. 원격 파일 해시는 `30fefc3a7d6a0041541858293bcbea2dde4caa4c0a5802f996a7f7e8c0085652`,
보존한 LF 종료 파일은 `23f18e03dc49df91622fe2a76176497404e46ced8a715d9d2b67a7446571cca3`다.

보강은 원래 SPDX 식과 도구가 고른 라이선스를 덮어쓰지 않는다. 검토된 출처 매핑이 없으면 실패하며,
이미 원문이 있는 패키지도 추가 NOTICE가 있으면 함께 보존한다. 특히 OpenDAL의 ASF NOTICE와
Moka의 파일별 Apache 예외 고지를 별도로 포함한다. `dunce`의 기존 `CC0 OR MIT-0 OR Apache`
선택에서는 실제 배포에 실린 CC0 원문을 사용하도록 CC0를 Apache보다 우선한다.

별도 LICENSE 파일 없이 배포된 `content_disposition` 0.4.0, `mac` 0.1.1,
`sledgehammer_bindgen` 0.6.0, `sledgehammer_bindgen_macro` 0.6.5, `sledgehammer_utils` 0.3.1,
`warnings` 0.2.1, `warnings-macro` 0.2.0은 예외를 숨기지 않는다. 원본 crate archive의
SHA-256을 Cargo.lock과 대조하고 그 안의 Cargo.toml 바이트를 그대로 보존했다.
`evidence=license-declaration`으로 표기하며 패키지 이름·버전·선언 식이 cargo-about의
패키지 metadata와 같아야 한다. 이 식의 선택은 여전히 cargo-about이 담당한다.
HTML과 inventory의 `declaration_only_packages`에 별도 고지문을 찾지 못했다는 경고를
남긴다. 없는 저작권자 문구를 창작하거나 선언 자료를 완전한 LICENSE 원문이라고 부르지 않는다.
이 경고의 공개 배포 판단은 별도 검토 경계로 남는다.

`mac` 0.1.1은 원본의 `MIT/Apache-2.0`를 도구가 `MIT OR Apache-2.0`로 표시한다.
해당 버전·Cargo.lock checksum·두 정확한 문자열에 한해 옛 표기를 대조하며 일반적인
SPDX 변환기를 만들지 않는다. 두 표기를 inventory에 모두 남긴다.

아이콘·폰트·Tailwind 고지는 `docs/licenses/assets-manifest.md` 및 관련 원문을 추가한다.
보강 원문은 `.gitattributes`로 줄바꿈 자동 변환을 막아 checksum을 유지한다.

## 출력과 배포 연결

- `THIRD-PARTY-NOTICES.html`: 외부 라이선스·저작권 및 자산 고지. 모든 문구를 HTML escape하며
  상류 문서의 마크업이나 스크립트를 실행하지 않는다.
- `third-party-inventory.json`: 패키지별 전체 식과 실제 선택한 라이선스, target/feature,
  도구 버전·해시, Cargo/정책/보강/실제 자산 입력 해시와 HTML 바이트 해시.

Cargo 원본 JSON의 로컬 registry/manifest 경로는 공개 결과에 넣지 않는다. 생성 중 입력이
바뀌면 결과를 만들지 않는다. 출력 중 실패한 새 디렉터리는 `.INCOMPLETE`로 표시하고
기존 결과를 덮어쓰지 않는다. 배포 context 도구는 완성된 두 파일만 받는다.

`prepare-container-context.py --notices ...`가 입력 해시와 Linux+web profile, 그리고
server ELF와 notice target의 일치를 재검사하고 두 파일을 공개 디렉터리에 포함한다. 실제 배포 후 `/THIRD-PARTY-NOTICES.html`과
`/third-party-inventory.json`에서 제공된다. 이 경로의 HTTP 바이트 대조는 별도의 실제 실행
검사로 확인해야 하며, 고지 생성 성공만으로 웹 서빙을 주장하지 않는다.

```sh
python3 -B scripts/test-third-party-notices.py
python3 -B scripts/check-container-context.py
```

첫 검사는 모의 cargo-about 출력의 변조·누락·escape·target·덮어쓰기 거절 검사다.
두 번째도 합성 context 검사이며 실제 Rust 빌드나 운영 이관을 대신하지 않는다.

## 남는 경계

이 결과는 target별 normal/build 의존성의 보수적인 목록이지 최종 링크에서 살아남은 코드만의
목록은 아니다. license expression 충족과 개별 고지 보강은 자동 법률 검토가 아니다.
프로젝트 자체의 라이선스, 출처 미확정 로컬 자산, OS 베이스 이미지의 고지·제공 의무 및
서명은 별도 검토 대상이다. 실제 회원 자료·DB·인증서·비밀 파일은 이 도구의 입력이 아니다.
