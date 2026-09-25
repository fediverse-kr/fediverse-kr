# `/start` intro release review

2026-09-14 · Packet A source audit

## Result

The current intro source is ready for preview. No implementation change was warranted in this pass.
The five scroll beats already form the requested short discovery:

1. familiar SNS timeline;
2. the replying person is not a member of that site;
3. `@who` plus `@where`, with an email analogy;
4. a connected personal blog;
5. the name “연합우주 / Fediverse”.

`BasicStage` reuses `SiteWindow`, `SocialPost`, `JournalArticle`, `JournalComment`, `Avatar`, `Person`, and
`Site` from `src/social_demo.rs`. No duplicate product renderer or landing change was introduced.

## Structural checks

- `src/explainer/model.rs` contains five compact intro beats and keeps the important “not a member here”
  reversal in beat 2.
- `src/explainer/mod.rs` changes scenes only from scroll geometry (with optional position shortcuts); the
  stage uses border/scene emphasis and contains no inter-server travel animation.
- `assets/explainer-scroll.js` owns browser geometry only, preserves normal scroll, supports reverse scroll,
  and disposes observers/listeners when the page leaves.
- `assets/styling/explainer.css` owns explanation layout and responsive compression; shared product appearance
  remains in `assets/styling/social-demo.css`.
- Existing captured mobile frame `output/playwright/explainer-final-mobile.png` shows both compact site
  windows and the “not a member” beat without horizontal overflow or clipped reply text.

## Validation boundary

`npx` is available (`9.6.1`). The repository’s existing Playwright artifacts document prior checks for the
intro at 1440/768/390/320px, reverse scrolling, reduced motion, SSR/no-JavaScript content, and observer
lifecycle. A fresh browser run was not started in this pass because the shared development server was stopped
and the packet explicitly reserves the build slot for the backend verification task.

Remaining human interpretation questions for preview: does a first-time reader notice the replying person is on
another site at the intended scroll position, and does the compact two-window composition remain immediately
legible on the target phone? These cannot be closed by source inspection alone.

## 2026-09-15 새 브라우저 확인

기존 `scripts/dev.ps1 -Mode Start`로 합성 PostgreSQL 17과 Dioxus 미리보기를 실행했다.
브라우저는 Edge 채널의 Playwright CLI 세션 `packetB`를 사용했다. 개발 서버는 후속 확인을 위해
계속 실행 중이다.

- PC 1440×1000: 첫 장면과 첫 반전 장면을 캡처했다. 첫 장면은 익숙한 SNS 대화로 시작하고,
  반전 장면은 같은 답글을 두 사이트 창에서 보여준다.
- 모바일 390×844, 320×667: 반전 장면을 전환·대기 후 캡처했다. 두 창이 세로로 보이며,
  단계는 `1`, 가로 넘침은 `false`였다. 스테이지는 각각 y=142..530, y=142..472로 화면 안에 있다.
- 모션 감소 1280×720: `/start/delivery` 두 번째 장면에서 `animation: none`, 스테이지 화면 내 배치를 확인했다.
- SSR/무 JavaScript: `/start`에 설명 장면 5개와 주제 링크 6개가 포함됐다.
- 잘못된 경로 `/start/not-a-topic`: 오류 페이지와 “아직 없는 페이지예요” 안내가 렌더링됐다.

캡처:

- `output/playwright/packetA-start-1440.png`
- `output/playwright/packetA-reveal-1440.png`
- `output/playwright/packetA-start-320.png`
- `output/playwright/packetA-reveal-390-settled.png`
- `output/playwright/packetA-reveal-320-settled.png`

이번 확인에서도 코드 수정은 하지 않았다. 320px 캡처의 하단 스크롤바처럼 보이는 부분은
DOM의 `documentElement.scrollWidth=320`, `body.scrollWidth=320`으로 재확인되어 실제 가로 넘침으로
판정하지 않았다. 초심자가 “다른 사이트 회원”을 의도한 위치에서 즉시 알아차리는지는 여전히
사람이 확인할 해석 질문이다.
