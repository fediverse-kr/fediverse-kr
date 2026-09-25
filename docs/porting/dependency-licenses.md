# 의존성 라이선스 감사

## 2026-09-14 AP 프로필 이미지 갱신

`image 0.25.10`의 GIF feature를 서버에만 추가했다. 기존 lock에 들어 있던
`gif 0.14.2`(MIT OR Apache-2.0), `weezl 0.1.12`(MIT OR Apache-2.0),
`color_quant 1.1.0`(MIT)을 사용한다. 선택된 root 도달 그래프는 Windows server
**457개**, Linux server **454개**, Wasm web **286개**다. GPL/LGPL/AGPL 식,
외부 라이선스 선언 누락, SQLx/bcrypt 0개. 웹에 image/OpenDAL 없음.

- Cargo.toml SHA-256: `6E4AACDDDBEBEBDA09DED4DCB3C918A9791B44DA203BD94F2790D1268C7F94F5`
- Cargo.lock SHA-256: `768467584B92DCBBACEBA2E1126C216034848CD084AA5E05FDC344513DC7A3D0` (변경 없음)
- 직접 작성한 GIF parser나 별도 다운로드/저장 프레임워크는 추가하지 않았다.
  [공통 처리와 보존 계약](profile-media-refresh.md). 패키지 선언 감사와 최종 배포 고지는 구별한다.

## 2026-09-14 로고 디코더 및 런타임 OpenDAL 쓰기

동일한 root 도달 그래프 감사: Windows server **454개**, Linux server **451개**,
Wasm web **286개**. GPL/LGPL/AGPL 식, 외부 라이선스 선언 누락, SQLx/bcrypt 모두 0개다.

- `image =0.25.10`은 기존 lock/cache의 패키지를 직접 서버 의존성으로 사용한다.
  기본 기능을 끄고 PNG/JPEG/WebP만 선택했다. image/image-webp/png는 MIT OR Apache-2.0,
  zune-core/zune-jpeg는 MIT OR Apache-2.0 OR Zlib, moxcms/pxfm는 BSD-3-Clause OR Apache-2.0이다.
  MIT 또는 BSD-3-Clause 선택과 각각의 고지 의무를 유지한다.
- OpenDAL의 공식 `blocking` feature를 추가했다. 별도 storage/lock 라이브러리를 자작하지 않고
  OpenDAL adapter와 Rust 표준 파일 잠금을 사용한다. `base64`는 파일 전송을 위해 웹에도
  명시적으로 활성화했다. 웹 그래프에 image/OpenDAL은 없으며 외부 패키지 수는 그대로다.
- Cargo.toml SHA-256: `9C0D1214EA97824D1B2E44CA847A6EC50D2987E9AB3FEC8E24F5C66736AF2EFA`
- Cargo.lock SHA-256: `768467584B92DCBBACEBA2E1126C216034848CD084AA5E05FDC344513DC7A3D0`
- 이것은 선택된 패키지 선언 감사다. 최종 배포 고지 조립과 프로젝트 라이선스 선택이
  완료됐다는 뜻은 아니다. [실제 적용 경계](catalog-logos.md).

## 2026-09-14 OpenDAL 및 범용 처리 교체

선택된 root 도달 그래프: Windows server **438개**, Linux server **435개**, Wasm web **286개**.
각 그래프의 GPL/LGPL/AGPL 식 0개, 외부 license 선언 누락 0개, SQLx/bcrypt 0개다.
OpenDAL 3개 crate(`opendal`, `opendal-core`, `opendal-service-fs`)는 서버에만 존재한다.
Linux는 metadata 감사이며 Linux 실행 시험은 아니다.

- 새 직접 의존성: OpenDAL 0.59.1 Apache-2.0, subtle 2.6.1 BSD-3-Clause,
  cookie 0.18.2 / percent-encoding 2.3.2 MIT OR Apache-2.0, quick-xml 0.41.0 MIT.
- OpenDAL의 Fs만 활성화했다. subtle/percent-encoding/quick-xml은 기존 전이 항목을
  명시적 의존성으로 사용하며 cookie는 0.18.1에서 0.18.2로 갱신했다.
- HTTP 서명 후보 `http-signature-normalization 0.7.0`은 AGPL-3.0 선언을 확인해 제외했다.
  해당 소스를 참조하거나 프로젝트에 복사/의존성 추가하지 않았다.
- Cargo.toml SHA-256: `0D6E79BCC1E94BA938C664D05BA567917E2AD6DCD4B5E47DC951231D150F8BF7`
- Cargo.lock SHA-256: `D82DC714555130A65176A0292F28F095F28306BD48E4AE138B1D1BAA81BAE046`
- 아래와 동일한 metadata root 순회로 확인했다. OR 선택/AND 의무를 유지하며,
  프로젝트 자체 라이선스 결정과 배포 고지 조립은 여전히 별도 남아 있다.
- [교체 이유와 남긴 정책](library-boundaries.md).

## 2026-09-13 Diesel 전환 재확인

현재 Windows server 410개, Linux server 408개, Wasm web 286개의 선택된 외부 패키지 그래프를 확인했다. SQLx/bcrypt 패키지 0개, GPL/AGPL 식 0개, 외부 license 선언 누락 0개다. Diesel/diesel-async/diesel_migrations는 MIT OR Apache-2.0 중 MIT를 선택한다. bb8 및 tokio-postgres-rustls는 MIT다. AND 조건은 유지하며 최종 notice 조립/프로젝트 자체 라이선스 결정은 아래 남은 사항과 같다.

- Cargo.toml SHA-256: `2F0BCAF988401B9D03696E8DCB73ED094048C8118BE7565AF04670C6F0956D1A`
- Cargo.lock SHA-256: `667400A400D28C790091FA1CCF0DD0D56BEA54A89422AF4FEAD3675F0D112F4E`
- `cargo metadata --locked --offline --format-version 1 --no-default-features --features {server|web} --filter-platform {target}`의 root 도달 그래프 기준이다. Linux는 metadata 감사이며 Linux 실행 검증이 아니다.
- Kameo/NATS/RabbitMQ는 추가하지 않았다. DB TLS는 Rustls를 사용하며 libpq 설치가 필요 없다.

이하 **2026-09-12 당시 잠금파일의 보존 기록**이다. 아래 SQLx 목록·해시·패키지 수를 현재 의존성으로 해석하지 않는다.

# 2026-09-12 감사 기록

## 결론

현재 잠금파일의 실제 웹·서버 의존성 그래프에서 **GPL/AGPL을 선택해야 하는 패키지와 외부 패키지의 라이선스 선언 누락은 발견되지 않았다.** 허용 가능한 OR 선택을 적용했으며 AND 조건은 제거하지 않았다.

이것은 Cargo 패키지 선언과 수집한 라이선스 파일에 대한 감사다. 최종 배포물의 모든 저작권 고지 조립, 개별 소스 파일의 출처 포렌식, Cargo 밖의 폰트·사진·JS 배포물까지 완료했다는 뜻은 아니다.

남은 확인 사항:

1. **프로젝트 자체 `fediversekr2 0.1.0`에는 `license`/`license-file` 선언과 루트 LICENSE 파일이 없다.** 외부 GPL 의존성 문제는 아니지만 저장소 공개 전에 소유자가 라이선스를 정해야 한다. 이 감사에서는 임의 지정하지 않았다.
2. 55개 기존 crate 배포 디렉터리에 최상위 LICENSE/COPYING/NOTICE 파일이 없다. SPDX 선언은 있으며 금지 라이선스를 뜻하지 않는다. 배포용 고지 묶음은 해당 upstream 라이선스/저작권 고지까지 보완해야 한다. 아래에 정확한 목록을 기록했다.
3. 아이콘의 crate 코드와 원본 artwork 고지는 구분해야 한다. 기존 `docs/icon-pack-notices.md`는 Heroicons/Ionicons/Bootstrap을 담지만 Lucide/Feather 고지는 별도 확인이 필요하다. Lucide upstream은 ISC 및 해당 Feather 유래 아이콘의 MIT 고지를 함께 제공한다. [Lucide 공식 LICENSE](https://raw.githubusercontent.com/lucide-icons/lucide/main/LICENSE)

## 확인 기준

- 감사 시작·종료 사이 Cargo.toml/Cargo.lock 해시 동일.
- Cargo.toml SHA-256: `6B01D58529E8142D177373F2FF3B9F812016FE272F79DAD05ED0D90A5A180B71`
- Cargo.lock SHA-256: `41BF6CDC269FCF1A262548E6C6468B37E46DEB837022FF84B31B14A85FDFCFC5`
- Rust host: `x86_64-pc-windows-msvc`, rustc 1.97.0.
- 웹: `--no-default-features --features web --filter-platform wasm32-unknown-unknown`.
- 서버: `--no-default-features --features server --filter-platform x86_64-pc-windows-msvc`.
- 각 `resolve.root`에서 실제 연결된 dependency node만 순회했다. `packages`에 나열됐지만 선택되지 않은 다른 플랫폼/feature 패키지를 합산하지 않았다.
- build/proc-macro 의존성도 그래프에 포함된다. Cargo metadata는 최종 바이너리의 dead-code 제거 목록이 아니다.
- Linux 배포, desktop/mobile feature 또는 feature 조합이 달라지면 해당 target으로 다시 감사해야 한다.
- Cargo/main/lockfile/기타 소스는 변경하지 않았다.

재현 명령:

```powershell
cargo metadata --locked --offline --format-version 1 --no-default-features --features web --filter-platform wasm32-unknown-unknown
cargo metadata --locked --offline --format-version 1 --no-default-features --features server --filter-platform x86_64-pc-windows-msvc
```

| 범위 | 외부 패키지 수 |
|---|---:|
| 웹 | 286 |
| 서버 | 405 |
| 두 그래프 합집합 | 406 |

프로젝트 자체 패키지는 위 숫자에서 제외했다. 버전이 다르면 별도 패키지로 센다.

## 선택한 SPDX 식 요약

- `OR` 및 과거 Cargo의 `/` 표기는 사용 가능한 선택지다. MIT를 우선 선택하고 없으면 Apache-2.0을 선택했다.
- `AND`는 둘 다 유지한다. 특히 `ring`, `encoding_rs`, `matchit`, `unicode-ident`, `dioxus-icons`를 단일 MIT로 축약하지 않았다.
- `content_disposition`은 명시된 선택지 중 `0BSD`를 선택했다.
- CC0, Unicode-3.0, CDLA-Permissive-2.0 등도 각 패키지의 고유 조건을 유지한다. GPL이 없다는 이유로 고지 의무도 없다고 판단하지 않는다.

| 선택한 식 | 웹 | 서버 | 합집합 |
|---|---:|---:|---:|
| `0BSD` | 1 | 1 | 1 |
| `Apache-2.0` | 7 | 7 | 7 |
| `Apache-2.0 AND ISC` | 0 | 1 | 1 |
| `BSD-3-Clause` | 0 | 1 | 1 |
| `BSL-1.0` | 1 | 1 | 1 |
| `CC0-1.0` | 1 | 1 | 1 |
| `CDLA-Permissive-2.0` | 0 | 2 | 2 |
| `ISC` | 0 | 3 | 3 |
| `MIT` | 248 | 359 | 360 |
| `MIT AND BSD-3-Clause` | 2 | 2 | 2 |
| `MIT AND ISC` | 1 | 1 | 1 |
| `MIT AND Unicode-3.0` | 1 | 1 | 1 |
| `Unicode-3.0` | 18 | 18 | 18 |
| `Zlib` | 6 | 7 | 7 |

## AP 관련 직접 의존성

아래 실제 설치 버전의 선언과 LICENSE 본문을 확인했다. 모두 MIT 선택이 가능하다.

| 패키지 | 선언 | 선택 | 확인 파일 |
|---|---|---|---|
| base64 0.22.1 | `MIT OR Apache-2.0` | MIT | LICENSE-MIT |
| reqwest 0.12.28 | `MIT OR Apache-2.0` | MIT | LICENSE-MIT |
| sha2 0.10.9 | `MIT OR Apache-2.0` | MIT | LICENSE-MIT |
| tokio 1.53.1 | `MIT` | MIT | LICENSE |
| url 2.5.8 | `MIT OR Apache-2.0` | MIT | LICENSE-MIT |
| html5ever 0.35.0 | `MIT OR Apache-2.0` | MIT | LICENSE-MIT |
| rsa 0.9.10 | `MIT OR Apache-2.0` | MIT | LICENSE-MIT |

Cargo.toml의 AP 라이브러리는 서버 feature에서만 직접 활성화한다. `reqwest`, `base64`, `sha2`, `tokio`, `url`은 Dioxus 등의 다른 경로를 통해 웹 그래프에도 등장할 수 있으므로 직접 optional 여부만으로 전체 그래프를 추정하지 않았다.

추가 확인:

- `dioxus-primitives 0.0.1` 및 `dioxus-attributes 0.1.0`: 실제 Git revision `bf007c15d0cf4d04d3181cc46cf12325aa773955`의 workspace 루트 LICENSE-MIT/Apache 확인.
- `ring 0.17.14`: LICENSE가 LICENSE-BoringSSL, LICENSE-other-bits, vendored once_cell 라이선스를 지시한다. Apache-2.0과 ISC를 함께 유지하고 내부 고지도 보존해야 한다.
- `encoding_rs 0.8.35`: LICENSE-MIT 및 LICENSE-WHATWG(BSD-3-Clause) 확인.
- `matchit 0.8.4`: LICENSE(MIT), LICENSE.httprouter(BSD-3-Clause) 확인.
- `unicode-ident 1.0.24`: LICENSE-MIT, LICENSE-UNICODE 확인.
- `dioxus-icons 0.1.0`: LICENSE(MIT), LICENSE-LUCIDE(ISC) 확인.
- `webpki-roots 0.26.11 / 1.0.9`: LICENSE의 CDLA-Permissive-2.0 본문 확인.
- `content_disposition 0.4.0`: 패키지 및 정확한 upstream commit `62330e3d606cbe32219300422f5922f55bedb3a2`의 Cargo.toml이 `MIT OR 0BSD`를 선언하고 src/lib.rs에도 선택지가 명시되어 있다. 별도 LICENSE 본문 파일은 패키지에 없다. [정확한 upstream Cargo.toml](https://raw.githubusercontent.com/y0zong/content-disposition/62330e3d606cbe32219300422f5922f55bedb3a2/Cargo.toml)
- Dioxus 패키지의 선언은 MIT/Apache 선택이다. crate에 LICENSE가 포함되지 않은 경우와 별개로 해당 0.7.10 계열의 upstream revision LICENSE-MIT도 확인했다. [Dioxus upstream LICENSE-MIT](https://raw.githubusercontent.com/DioxusLabs/dioxus/57d6794ad60b949e5bd8aa282f6f8c3dc97a365e/LICENSE-MIT)

AP 독립 테스트 하네스의 이전 잠금파일과 이 본체 잠금파일은 별개다. 그 하네스에서 보였던 `r-efi`의 LGPL 대안은 현재 두 실제 target 그래프에는 없다. 이전 감사 결과를 본체 그래프 결과로 복사하지 않았다.

## 최상위 라이선스 파일이 패키지에 없는 항목

선언 누락이나 GPL 판정 목록이 아니다. 이미 선언된 허용 라이선스의 **배포 고지 원문 확보**를 위한 후속 목록이다. Git workspace 상위 디렉터리에서 고지를 확인한 dioxus-attributes는 제외했다.

| 패키지 | Cargo 라이선스 선언 |
|---|---|
| const-serialize 0.7.2 | `MIT OR Apache-2.0` |
| const-serialize 0.8.0-alpha.0 | `MIT OR Apache-2.0` |
| const-serialize-macro 0.7.2 | `MIT OR Apache-2.0` |
| const-serialize-macro 0.8.0-alpha.1 | `MIT OR Apache-2.0` |
| content_disposition 0.4.0 | `MIT OR 0BSD` |
| crc-catalog 2.5.0 | `MIT OR Apache-2.0` |
| dioxus 0.7.9 | `MIT OR Apache-2.0` |
| dioxus-asset-resolver 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-cli-config 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-config-macro 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-config-macros 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-core 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-core-macro 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-core-types 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-devtools 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-devtools-types 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-document 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-fullstack 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-fullstack-core 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-fullstack-macro 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-history 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-hooks 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-html 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-html-internal-macro 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-interpreter-js 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-liveview 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-logger 0.7.10 | `MIT` |
| dioxus-router 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-router-macro 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-rsx 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-sdk-time 0.7.0 | `MIT OR Apache-2.0` |
| dioxus-server 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-signals 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-ssr 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-stores 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-stores-macro 0.7.10 | `MIT OR Apache-2.0` |
| dioxus-web 0.7.10 | `MIT OR Apache-2.0` |
| generational-box 0.7.10 | `MIT OR Apache-2.0` |
| gloo-net 0.6.0 | `MIT OR Apache-2.0` |
| gloo-timers 0.3.0 | `MIT OR Apache-2.0` |
| gloo-utils 0.2.0 | `MIT OR Apache-2.0` |
| lazy-js-bundle 0.7.10 | `MIT OR Apache-2.0` |
| lucide-dioxus 3.26.0 | `MIT` |
| mac 0.1.1 | `MIT/Apache-2.0` |
| manganis 0.7.10 | `MIT OR Apache-2.0` |
| manganis-core 0.7.10 | `MIT OR Apache-2.0` |
| manganis-macro 0.7.10 | `MIT OR Apache-2.0` |
| match_token 0.35.0 | `MIT OR Apache-2.0` |
| sledgehammer_bindgen 0.6.0 | `MIT` |
| sledgehammer_bindgen_macro 0.6.5 | `MIT` |
| sledgehammer_utils 0.3.1 | `MIT` |
| subsecond 0.7.10 | `MIT OR Apache-2.0` |
| subsecond-types 0.7.10 | `MIT OR Apache-2.0` |
| warnings 0.2.1 | `MIT OR Apache-2.0` |
| warnings-macro 0.2.0 | `MIT OR Apache-2.0` |

## 전체 선택 목록

웹/서버 표시는 위 두 target 그래프 기준이다.

| 패키지 | 웹 | 서버 | 선언 | 이번 선택 |
|---|:---:|:---:|---|---|
| aho-corasick 1.1.5 | ✓ | ✓ | `Unlicense OR MIT` | `MIT` |
| allocator-api2 0.2.21 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| anyhow 1.0.104 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| approx 0.5.1 | ✓ | ✓ | `Apache-2.0` | `Apache-2.0` |
| argon2 0.5.3 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| askama_escape 0.13.0 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| async-stream 0.3.6 | ✓ | ✓ | `MIT` | `MIT` |
| async-stream-impl 0.3.6 | ✓ | ✓ | `MIT` | `MIT` |
| async-trait 0.1.91 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| async-tungstenite 0.31.0 | — | ✓ | `MIT` | `MIT` |
| atoi 2.0.0 | — | ✓ | `MIT` | `MIT` |
| atomic-waker 1.1.2 | ✓ | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| autocfg 1.5.1 | ✓ | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| axum 0.8.9 | ✓ | ✓ | `MIT` | `MIT` |
| axum-core 0.5.6 | ✓ | ✓ | `MIT` | `MIT` |
| axum-extra 0.10.3 | — | ✓ | `MIT` | `MIT` |
| axum-macros 0.5.1 | ✓ | ✓ | `MIT` | `MIT` |
| base16 0.2.1 | ✓ | ✓ | `CC0-1.0` | `CC0-1.0` |
| base64 0.22.1 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| base64ct 1.8.3 | — | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| bitflags 2.13.1 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| blake2 0.10.6 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| block-buffer 0.10.4 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| bumpalo 3.20.3 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| by_address 1.2.1 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| byteorder 1.5.0 | ✓ | ✓ | `Unlicense OR MIT` | `MIT` |
| bytes 1.12.1 | ✓ | ✓ | `MIT` | `MIT` |
| cc 1.4.1 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| cfb 0.7.3 | ✓ | ✓ | `MIT` | `MIT` |
| cfg_aliases 0.2.2 | — | ✓ | `MIT` | `MIT` |
| cfg-if 1.0.4 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| chacha20 0.10.1 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| charset 0.1.5 | ✓ | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| chrono 0.4.45 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| ciborium 0.2.2 | ✓ | ✓ | `Apache-2.0` | `Apache-2.0` |
| ciborium-io 0.2.2 | ✓ | ✓ | `Apache-2.0` | `Apache-2.0` |
| ciborium-ll 0.2.2 | ✓ | ✓ | `Apache-2.0` | `Apache-2.0` |
| const_format 0.2.36 | ✓ | ✓ | `Zlib` | `Zlib` |
| const_format_proc_macros 0.2.34 | ✓ | ✓ | `Zlib` | `Zlib` |
| const-oid 0.9.6 | — | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| const-serialize 0.7.2 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| const-serialize 0.8.0-alpha.0 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| const-serialize-macro 0.7.2 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| const-serialize-macro 0.8.0-alpha.1 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| const-str 0.7.1 | ✓ | ✓ | `MIT` | `MIT` |
| content_disposition 0.4.0 | ✓ | ✓ | `MIT OR 0BSD` | `0BSD` |
| convert_case 0.10.0 | ✓ | ✓ | `MIT` | `MIT` |
| convert_case 0.8.0 | ✓ | ✓ | `MIT` | `MIT` |
| cookie 0.18.1 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| cookie_store 0.22.1 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| cpufeatures 0.2.17 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| cpufeatures 0.3.0 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| crc 3.4.0 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| crc-catalog 2.5.0 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| crossbeam-queue 0.3.14 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| crossbeam-utils 0.8.22 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| crypto-common 0.1.7 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| darling 0.21.3 | ✓ | ✓ | `MIT` | `MIT` |
| darling_core 0.21.3 | ✓ | ✓ | `MIT` | `MIT` |
| darling_macro 0.21.3 | ✓ | ✓ | `MIT` | `MIT` |
| dashmap 6.2.1 | ✓ | ✓ | `MIT` | `MIT` |
| data-encoding 2.11.1 | ✓ | ✓ | `MIT` | `MIT` |
| der 0.7.10 | — | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| deranged 0.5.8 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| derive_more 2.1.1 | ✓ | ✓ | `MIT` | `MIT` |
| derive_more-impl 2.1.1 | ✓ | ✓ | `MIT` | `MIT` |
| digest 0.10.7 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus 0.7.9 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-asset-resolver 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-attributes 0.1.0 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-cli-config 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-config-macro 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-config-macros 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-core 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-core-macro 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-core-types 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-devtools 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-devtools-types 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-document 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-free-icons 0.10.0 | ✓ | ✓ | `MIT` | `MIT` |
| dioxus-fullstack 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-fullstack-core 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-fullstack-macro 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-history 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-hooks 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-html 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-html-internal-macro 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-icons 0.1.0 | ✓ | ✓ | `MIT AND ISC` | `MIT AND ISC` |
| dioxus-interpreter-js 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-liveview 0.7.10 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-logger 0.7.10 | ✓ | ✓ | `MIT` | `MIT` |
| dioxus-primitives 0.0.1 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-router 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-router-macro 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-rsx 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-sdk-time 0.7.0 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-server 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-signals 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-ssr 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-stores 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-stores-macro 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dioxus-web 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| displaydoc 0.2.7 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| document-features 0.2.12 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| dotenvy 0.15.7 | — | ✓ | `MIT` | `MIT` |
| dunce 1.0.5 | ✓ | ✓ | `CC0-1.0 OR MIT-0 OR Apache-2.0` | `Apache-2.0` |
| either 1.17.0 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| encoding_rs 0.8.35 | ✓ | ✓ | `(Apache-2.0 OR MIT) AND BSD-3-Clause` | `MIT AND BSD-3-Clause` |
| enumset 1.1.14 | ✓ | ✓ | `MIT/Apache-2.0` | `MIT` |
| enumset_derive 0.15.0 | ✓ | ✓ | `MIT/Apache-2.0` | `MIT` |
| equivalent 1.0.2 | ✓ | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| etcetera 0.8.0 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| euclid 0.22.14 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| event-listener 5.4.2 | — | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| find-msvc-tools 0.1.10 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| flume 0.11.1 | — | ✓ | `Apache-2.0/MIT` | `MIT` |
| fnv 1.0.7 | ✓ | ✓ | `Apache-2.0 / MIT` | `MIT` |
| foldhash 0.1.5 | — | ✓ | `Zlib` | `Zlib` |
| foldhash 0.2.0 | ✓ | ✓ | `Zlib` | `Zlib` |
| form_urlencoded 1.2.2 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| futf 0.1.5 | — | ✓ | `MIT / Apache-2.0` | `MIT` |
| futures 0.3.33 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| futures-channel 0.3.33 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| futures-core 0.3.33 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| futures-executor 0.3.33 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| futures-intrusive 0.5.0 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| futures-io 0.3.33 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| futures-macro 0.3.33 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| futures-sink 0.3.33 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| futures-task 0.3.33 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| futures-util 0.3.33 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| generational-box 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| generic-array 0.14.7 | ✓ | ✓ | `MIT` | `MIT` |
| getrandom 0.2.17 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| getrandom 0.3.4 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| getrandom 0.4.3 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| gloo-net 0.6.0 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| gloo-timers 0.3.0 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| gloo-utils 0.2.0 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| h2 0.4.15 | ✓ | ✓ | `MIT` | `MIT` |
| half 2.7.1 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| hashbrown 0.14.5 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| hashbrown 0.15.5 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| hashbrown 0.16.1 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| hashbrown 0.17.1 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| hashlink 0.10.0 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| headers 0.4.1 | ✓ | ✓ | `MIT` | `MIT` |
| headers-core 0.3.0 | ✓ | ✓ | `MIT` | `MIT` |
| heck 0.5.0 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| hex 0.4.3 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| hkdf 0.12.4 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| hmac 0.12.1 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| home 0.5.12 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| html5ever 0.35.0 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| http 1.5.0 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| http-body 1.1.0 | ✓ | ✓ | `MIT` | `MIT` |
| http-body-util 0.1.4 | ✓ | ✓ | `MIT` | `MIT` |
| http-range-header 0.4.2 | ✓ | ✓ | `MIT` | `MIT` |
| httparse 1.10.1 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| httpdate 1.0.3 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| hyper 1.11.0 | ✓ | ✓ | `MIT` | `MIT` |
| hyper-rustls 0.27.9 | — | ✓ | `Apache-2.0 OR ISC OR MIT` | `MIT` |
| hyper-util 0.1.20 | ✓ | ✓ | `MIT` | `MIT` |
| icu_collections 2.2.0 | ✓ | ✓ | `Unicode-3.0` | `Unicode-3.0` |
| icu_locale_core 2.2.0 | ✓ | ✓ | `Unicode-3.0` | `Unicode-3.0` |
| icu_normalizer 2.2.0 | ✓ | ✓ | `Unicode-3.0` | `Unicode-3.0` |
| icu_normalizer_data 2.2.0 | ✓ | ✓ | `Unicode-3.0` | `Unicode-3.0` |
| icu_properties 2.2.0 | ✓ | ✓ | `Unicode-3.0` | `Unicode-3.0` |
| icu_properties_data 2.2.0 | ✓ | ✓ | `Unicode-3.0` | `Unicode-3.0` |
| icu_provider 2.2.0 | ✓ | ✓ | `Unicode-3.0` | `Unicode-3.0` |
| ident_case 1.0.1 | ✓ | ✓ | `MIT/Apache-2.0` | `MIT` |
| idna 1.1.0 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| idna_adapter 1.2.2 | ✓ | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| indexmap 2.14.0 | ✓ | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| infer 0.19.0 | ✓ | ✓ | `MIT` | `MIT` |
| inventory 0.3.24 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| ipnet 2.12.1 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| itoa 1.0.18 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| js-sys 0.3.103 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| keyboard-types 0.7.0 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| konst 0.2.20 | ✓ | ✓ | `Zlib` | `Zlib` |
| konst_macro_rules 0.2.19 | ✓ | ✓ | `Zlib` | `Zlib` |
| lazy_static 1.5.0 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| lazy-js-bundle 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| libc 0.2.189 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| libloading 0.8.9 | — | ✓ | `ISC` | `ISC` |
| libm 0.2.16 | — | ✓ | `MIT` | `MIT` |
| libsqlite3-sys 0.30.1 | — | ✓ | `MIT` | `MIT` |
| litemap 0.8.2 | ✓ | ✓ | `Unicode-3.0` | `Unicode-3.0` |
| litrs 1.0.0 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| lock_api 0.4.14 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| log 0.4.33 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| longest-increasing-subsequence 0.1.0 | ✓ | ✓ | `MIT/Apache-2.0` | `MIT` |
| lru 0.16.4 | ✓ | ✓ | `MIT` | `MIT` |
| lru-slab 0.1.2 | — | ✓ | `MIT OR Apache-2.0 OR Zlib` | `MIT` |
| lucide-dioxus 3.26.0 | ✓ | ✓ | `MIT` | `MIT` |
| mac 0.1.1 | — | ✓ | `MIT/Apache-2.0` | `MIT` |
| macro-string 0.1.4 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| manganis 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| manganis-core 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| manganis-macro 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| markup5ever 0.35.0 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| match_token 0.35.0 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| matchers 0.2.0 | ✓ | ✓ | `MIT` | `MIT` |
| matchit 0.8.4 | ✓ | ✓ | `MIT AND BSD-3-Clause` | `MIT AND BSD-3-Clause` |
| md-5 0.10.6 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| memchr 2.8.3 | ✓ | ✓ | `Unlicense OR MIT` | `MIT` |
| memmap2 0.9.11 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| mime 0.3.17 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| mime_guess 2.0.5 | ✓ | ✓ | `MIT` | `MIT` |
| mio 1.2.2 | ✓ | ✓ | `MIT` | `MIT` |
| multer 3.1.0 | ✓ | ✓ | `MIT` | `MIT` |
| new_debug_unreachable 1.0.6 | — | ✓ | `MIT` | `MIT` |
| num-bigint-dig 0.8.6 | — | ✓ | `MIT/Apache-2.0` | `MIT` |
| num-conv 0.2.2 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| num-integer 0.1.46 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| num-iter 0.1.46 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| num-traits 0.2.19 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| once_cell 1.21.4 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| palette 0.7.7 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| palette_derive 0.7.7 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| palette_math 0.7.7 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| parking 2.2.1 | — | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| parking_lot 0.12.5 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| parking_lot_core 0.9.12 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| password-hash 0.5.0 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| pem-rfc7468 0.7.0 | — | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| percent-encoding 2.3.2 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| phf 0.11.3 | — | ✓ | `MIT` | `MIT` |
| phf 0.13.1 | ✓ | ✓ | `MIT` | `MIT` |
| phf_codegen 0.11.3 | — | ✓ | `MIT` | `MIT` |
| phf_generator 0.11.3 | — | ✓ | `MIT` | `MIT` |
| phf_shared 0.11.3 | — | ✓ | `MIT` | `MIT` |
| phf_shared 0.13.1 | ✓ | ✓ | `MIT` | `MIT` |
| pin-project 1.1.13 | ✓ | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| pin-project-internal 1.1.13 | ✓ | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| pin-project-lite 0.2.17 | ✓ | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| pkcs1 0.7.5 | — | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| pkcs8 0.10.2 | — | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| pkg-config 0.3.33 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| potential_utf 0.1.5 | ✓ | ✓ | `Unicode-3.0` | `Unicode-3.0` |
| powerfmt 0.2.0 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| ppv-lite86 0.2.21 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| precomputed-hash 0.1.1 | — | ✓ | `MIT` | `MIT` |
| proc-macro2 1.0.107 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| proc-macro2-diagnostics 0.10.1 | ✓ | ✓ | `MIT/Apache-2.0` | `MIT` |
| psl-types 2.0.11 | — | ✓ | `MIT/Apache-2.0` | `MIT` |
| publicsuffix 2.3.0 | — | ✓ | `MIT/Apache-2.0` | `MIT` |
| quinn 0.11.11 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| quinn-proto 0.11.16 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| quinn-udp 0.5.15 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| quote 1.0.47 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| rand 0.10.2 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| rand 0.8.7 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| rand 0.9.5 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| rand_chacha 0.3.1 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| rand_chacha 0.9.0 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| rand_core 0.10.1 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| rand_core 0.6.4 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| rand_core 0.9.5 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| rand_pcg 0.10.2 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| regex-automata 0.4.18 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| regex-syntax 0.8.11 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| reqwest 0.12.28 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| ring 0.17.14 | — | ✓ | `Apache-2.0 AND ISC` | `Apache-2.0 AND ISC` |
| rsa 0.9.10 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| rustc_version 0.4.1 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| rustc-hash 1.1.0 | ✓ | ✓ | `Apache-2.0/MIT` | `MIT` |
| rustc-hash 2.1.3 | ✓ | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| rustls 0.23.43 | — | ✓ | `Apache-2.0 OR ISC OR MIT` | `MIT` |
| rustls-pki-types 1.15.1 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| rustls-webpki 0.103.13 | — | ✓ | `ISC` | `ISC` |
| rustversion 1.0.23 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| ryu 1.0.23 | ✓ | ✓ | `Apache-2.0 OR BSL-1.0` | `Apache-2.0` |
| same-file 1.0.6 | ✓ | ✓ | `Unlicense/MIT` | `MIT` |
| scopeguard 1.2.0 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| semver 1.0.28 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| send_wrapper 0.6.0 | ✓ | ✓ | `MIT/Apache-2.0` | `MIT` |
| serde 1.0.229 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| serde_core 1.0.229 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| serde_derive 1.0.229 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| serde_json 1.0.151 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| serde_path_to_error 0.1.20 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| serde_qs 0.15.0 | ✓ | ✓ | `MIT/Apache-2.0` | `MIT` |
| serde_repr 0.1.21 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| serde_urlencoded 0.7.1 | ✓ | ✓ | `MIT/Apache-2.0` | `MIT` |
| serde-wasm-bindgen 0.6.5 | ✓ | ✓ | `MIT` | `MIT` |
| sha1 0.10.7 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| sha2 0.10.9 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| sharded-slab 0.1.7 | ✓ | ✓ | `MIT` | `MIT` |
| shlex 2.0.1 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| signature 2.2.0 | — | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| siphasher 1.0.3 | ✓ | ✓ | `MIT/Apache-2.0` | `MIT` |
| slab 0.4.12 | ✓ | ✓ | `MIT` | `MIT` |
| sledgehammer_bindgen 0.6.0 | ✓ | ✓ | `MIT` | `MIT` |
| sledgehammer_bindgen_macro 0.6.5 | ✓ | ✓ | `MIT` | `MIT` |
| sledgehammer_utils 0.3.1 | ✓ | ✓ | `MIT` | `MIT` |
| slotmap 1.1.1 | ✓ | ✓ | `Zlib` | `Zlib` |
| smallvec 1.15.2 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| socket2 0.6.5 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| spin 0.9.9 | ✓ | ✓ | `MIT` | `MIT` |
| spki 0.7.3 | — | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| sqlx 0.8.6 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| sqlx-core 0.8.6 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| sqlx-macros 0.8.6 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| sqlx-macros-core 0.8.6 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| sqlx-mysql 0.8.6 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| sqlx-postgres 0.8.6 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| sqlx-sqlite 0.8.6 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| stable_deref_trait 1.2.1 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| string_cache 0.8.9 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| string_cache_codegen 0.5.4 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| stringprep 0.1.5 | — | ✓ | `MIT/Apache-2.0` | `MIT` |
| subsecond 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| subsecond-types 0.7.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| subtle 2.6.1 | — | ✓ | `BSD-3-Clause` | `BSD-3-Clause` |
| syn 2.0.119 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| syn 3.0.3 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| sync_wrapper 1.0.2 | ✓ | ✓ | `Apache-2.0` | `Apache-2.0` |
| synstructure 0.13.2 | ✓ | ✓ | `MIT` | `MIT` |
| tendril 0.4.3 | — | ✓ | `MIT/Apache-2.0` | `MIT` |
| thiserror 1.0.69 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| thiserror 2.0.19 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| thiserror-impl 1.0.69 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| thiserror-impl 2.0.19 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| thread_local 1.1.10 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| time 0.3.55 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| time-core 0.1.9 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| time-macros 0.2.32 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| tinystr 0.8.3 | ✓ | ✓ | `Unicode-3.0` | `Unicode-3.0` |
| tinyvec 1.12.0 | — | ✓ | `Zlib OR Apache-2.0 OR MIT` | `MIT` |
| tinyvec_macros 0.1.1 | — | ✓ | `MIT OR Apache-2.0 OR Zlib` | `MIT` |
| tokio 1.53.1 | ✓ | ✓ | `MIT` | `MIT` |
| tokio-macros 2.7.2 | ✓ | ✓ | `MIT` | `MIT` |
| tokio-rustls 0.26.4 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| tokio-stream 0.1.19 | — | ✓ | `MIT` | `MIT` |
| tokio-tungstenite 0.28.0 | ✓ | ✓ | `MIT` | `MIT` |
| tokio-tungstenite 0.29.0 | ✓ | ✓ | `MIT` | `MIT` |
| tokio-util 0.7.19 | ✓ | ✓ | `MIT` | `MIT` |
| tower 0.5.3 | ✓ | ✓ | `MIT` | `MIT` |
| tower-http 0.6.11 | ✓ | ✓ | `MIT` | `MIT` |
| tower-layer 0.3.3 | ✓ | ✓ | `MIT` | `MIT` |
| tower-service 0.3.3 | ✓ | ✓ | `MIT` | `MIT` |
| tracing 0.1.44 | ✓ | ✓ | `MIT` | `MIT` |
| tracing-attributes 0.1.31 | ✓ | ✓ | `MIT` | `MIT` |
| tracing-core 0.1.36 | ✓ | ✓ | `MIT` | `MIT` |
| tracing-futures 0.2.5 | ✓ | ✓ | `MIT` | `MIT` |
| tracing-subscriber 0.3.23 | ✓ | ✓ | `MIT` | `MIT` |
| tracing-wasm 0.2.1 | ✓ | — | `MIT OR Apache-2.0` | `MIT` |
| try-lock 0.2.5 | ✓ | ✓ | `MIT` | `MIT` |
| tungstenite 0.27.0 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| tungstenite 0.28.0 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| tungstenite 0.29.0 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| typenum 1.20.1 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| unicase 2.9.0 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| unicode-bidi 0.3.18 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| unicode-ident 1.0.24 | ✓ | ✓ | `(MIT OR Apache-2.0) AND Unicode-3.0` | `MIT AND Unicode-3.0` |
| unicode-normalization 0.1.25 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| unicode-properties 0.1.4 | — | ✓ | `MIT/Apache-2.0` | `MIT` |
| unicode-segmentation 1.13.3 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| unicode-xid 0.2.6 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| untrusted 0.9.0 | — | ✓ | `ISC` | `ISC` |
| url 2.5.8 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| utf-8 0.7.6 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| utf8_iter 1.0.4 | ✓ | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| uuid 1.24.0 | ✓ | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| vcpkg 0.2.15 | — | ✓ | `MIT/Apache-2.0` | `MIT` |
| version_check 0.9.5 | ✓ | ✓ | `MIT/Apache-2.0` | `MIT` |
| walkdir 2.5.0 | ✓ | ✓ | `Unlicense/MIT` | `MIT` |
| want 0.3.1 | ✓ | ✓ | `MIT` | `MIT` |
| warnings 0.2.1 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| warnings-macro 0.2.0 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| wasm-bindgen 0.2.126 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| wasm-bindgen-futures 0.4.76 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| wasm-bindgen-macro 0.2.126 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| wasm-bindgen-macro-support 0.2.126 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| wasm-bindgen-shared 0.2.126 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| wasm-streams 0.4.2 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| web_atoms 0.1.3 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| web-sys 0.3.103 | ✓ | ✓ | `MIT OR Apache-2.0` | `MIT` |
| webpki-roots 0.26.11 | — | ✓ | `CDLA-Permissive-2.0` | `CDLA-Permissive-2.0` |
| webpki-roots 1.0.9 | — | ✓ | `CDLA-Permissive-2.0` | `CDLA-Permissive-2.0` |
| whoami 1.6.1 | — | ✓ | `Apache-2.0 OR BSL-1.0 OR MIT` | `MIT` |
| winapi-util 0.1.11 | — | ✓ | `Unlicense OR MIT` | `MIT` |
| windows_x86_64_msvc 0.48.5 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| windows-link 0.2.1 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| windows-registry 0.6.1 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| windows-result 0.4.1 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| windows-strings 0.5.1 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| windows-sys 0.48.0 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| windows-sys 0.61.2 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| windows-targets 0.48.5 | — | ✓ | `MIT OR Apache-2.0` | `MIT` |
| winnow 0.7.15 | ✓ | ✓ | `MIT` | `MIT` |
| writeable 0.6.3 | ✓ | ✓ | `Unicode-3.0` | `Unicode-3.0` |
| xxhash-rust 0.8.18 | ✓ | ✓ | `BSL-1.0` | `BSL-1.0` |
| yoke 0.8.3 | ✓ | ✓ | `Unicode-3.0` | `Unicode-3.0` |
| yoke-derive 0.8.2 | ✓ | ✓ | `Unicode-3.0` | `Unicode-3.0` |
| zerocopy 0.8.56 | ✓ | ✓ | `BSD-2-Clause OR Apache-2.0 OR MIT` | `MIT` |
| zerocopy-derive 0.8.56 | ✓ | ✓ | `BSD-2-Clause OR Apache-2.0 OR MIT` | `MIT` |
| zerofrom 0.1.8 | ✓ | ✓ | `Unicode-3.0` | `Unicode-3.0` |
| zerofrom-derive 0.1.7 | ✓ | ✓ | `Unicode-3.0` | `Unicode-3.0` |
| zeroize 1.9.0 | — | ✓ | `Apache-2.0 OR MIT` | `MIT` |
| zerotrie 0.2.4 | ✓ | ✓ | `Unicode-3.0` | `Unicode-3.0` |
| zerovec 0.11.6 | ✓ | ✓ | `Unicode-3.0` | `Unicode-3.0` |
| zerovec-derive 0.11.3 | ✓ | ✓ | `Unicode-3.0` | `Unicode-3.0` |
| zmij 1.0.23 | ✓ | ✓ | `MIT` | `MIT` |
