---
name: fediverse.kr
description: 간결한 한국어와 두 서버의 대화 장면으로 안내하는 라이트 UI 검토용 초안.
colors:
  ink: "#242a36"
  muted: "#616977"
  line: "#e3e5eb"
  paper: "#fff"
  wash: "#f5f6fa"
  accent: "#5145a6"
  dog: "#5145a6"
  cat: "#a24e53"
  violet-bg: "#eeebf7"
  violet-text: "#51458d"
  green-bg: "#e9f0e9"
  green-text: "#43624b"
  rose-bg: "#f7e8e6"
  rose-text: "#974f54"
  orange-bg: "#f6edde"
  orange-text: "#8b6033"
typography:
  display:
    fontFamily: 'Pretendard, "Malgun Gothic", system-ui, sans-serif'
    fontSize: "clamp(44px, 4.3vw, 64px)"
    fontWeight: 760
    lineHeight: 1.34
    letterSpacing: "-0.035em"
  headline:
    fontSize: "42px"
    fontWeight: 740
    lineHeight: 1.3
    letterSpacing: "-0.035em"
  title:
    fontSize: "21px"
    fontWeight: 650
    lineHeight: 1.3
  body:
    fontFamily: 'Pretendard, "Malgun Gothic", system-ui, sans-serif'
    fontSize: "15px"
    lineHeight: 1.9
  label:
    fontSize: "14px"
    fontWeight: 600
    lineHeight: 1.6
  conversation:
    fontSize: "15px"
    lineHeight: 1.7
  experience-caption:
    fontSize: "16px"
    fontWeight: 600
    lineHeight: 1.65
  compact-action:
    fontSize: "13px"
    fontWeight: 650
rounded:
  control: "8px"
  button: "9px"
  avatar: "10px"
  preview: "12px"
  panel: "13px"
  scene: "14px"
  pill: "30px"
  circle: "50%"
spacing:
  compact-gap: "8px"
  icon-gap: "10px"
  post-padding: "16px"
  grid-gap: "20px"
  card-padding: "24px"
components:
  button-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.paper}"
    typography: "{typography.label}"
    rounded: "{rounded.button}"
    padding: "12px 21px"
  button-secondary:
    backgroundColor: "{colors.wash}"
    textColor: "{colors.ink}"
    typography: "{typography.label}"
    rounded: "{rounded.button}"
    padding: "12px 21px"
  search:
    textColor: "{colors.ink}"
    rounded: "{rounded.button}"
    padding: "11px 14px"
  filter:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
    rounded: "{rounded.pill}"
    padding: "8px 17px"
  filter-selected:
    backgroundColor: "{colors.ink}"
    textColor: "{colors.paper}"
    rounded: "{rounded.pill}"
    padding: "8px 17px"
  server-card:
    rounded: "{rounded.panel}"
    padding: "24px"
  experience-window:
    backgroundColor: "{colors.paper}"
    rounded: "{rounded.scene}"
    height: "530px"
  reply-slot:
    height: "180px"
  composer-send:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.paper}"
    typography: "{typography.compact-action}"
    rounded: "{rounded.button}"
    padding: "9px 14px"
---

# Design System: fediverse.kr

## Overview

기존 라이트 UI를 확장한 검토용 초안이다. 간결한 한국어와 직접 조작하는 이용 장면을 중심에 두며, 글 한 편이 다른 서버에 도착하고 답글이 돌아오는 모습을 같은 콘텐츠로 보여 준다. 새로운 브랜드 은유나 사용자 승인 디자인 이름은 정하지 않았다.

추출 근거는 `assets/styling/portal.css`와 그 뒤에 적용되는 `assets/styling/experience.css`, `src/experience.rs`, `src/explore.rs`, `src/portal.rs`, `src/main.rs`, `PRODUCT.md`다. 프런트매터는 재사용되는 실제 값의 요약이며, 별도의 런타임 토큰 파일은 아니다.

**Key Characteristics:**

- 흰 바탕, 가는 구분선, 보라색 강조.
- 두 서버의 정체성은 색과 이름·모양으로 함께 구분.
- 설명은 짧게, 이용 장면과 다음 행동은 명확하게.

## Colors

Primary는 `accent`다. 링크의 강조, 주 행동, 선택 상태, 멍멍.타운의 `dog` 색으로 사용한다. Secondary인 `cat`은 냥냥.타워의 표식·주소·도착 안내에 쓰인다. 두 서버의 색은 밝은 면 위의 절제된 강조로 유지한다.

Neutral은 `paper` 바탕, `ink` 본문, `muted` 보조 설명, `line` 경계, `wash` 보조 영역으로 나뉜다. 서버 표식은 violet/green/rose/orange의 실제 배경·문자 쌍을 공유한다. 활동 예시는 독서의 종이빛, 게시판의 연한 초록처럼 콘텐츠에 맞는 옅은 면을 사용한다. 이 색들은 상태의 성공·실패 의미가 아니다.

## Typography

Pretendard 가변 폰트를 자체 제공한다 (`assets/fonts/PretendardVariable.woff2`, 가중치 100–900, `font-display: swap`). 별도 영문·모노 서체는 없다.

Display는 홈 제목, headline은 경로별 페이지 제목, title은 서버 카드와 상세 절 제목이다. Body 토큰은 상세 설명·FAQ의 본문을 대표하며 전역 글자 크기 규칙은 아니다. Conversation은 양쪽 서버의 같은 글, experience-caption은 진행 상태, compact-action은 전송 버튼에 사용한다. 나머지 설명은 주로 13–16px, 작은 데모 메타정보는 9–12px를 사용한다. 제목은 균형 줄바꿈과 어절 보존, 한국어 본문도 주요 영역에서 어절 보존을 사용한다. 사용자 글은 줄바꿈을 보존하고 긴 문자열도 영역 안에서 줄바꿈한다. FAQ 본문 최대 폭은 70ch다.

## Layout

공통 컨테이너는 `min(1280px, calc(100% - 96px))`, 최소 문서 폭은 320px다. 학습 페이지는 최대 900px, 서버 상세는 1020px다. 홈은 설명/체험 0.75:2의 열과 40px 간격, 상하 여백 64/52px를 사용한다. 체험은 같은 폭의 두 창 사이에 28px 연결 영역을 둔다. 창은 experience-window 높이를 유지하고 피드만 내부 스크롤한다. 답글 안내와 실제 답글은 같은 reply-slot 안에서 교체하며, 하단 계정 표시는 고정한다.

활동 예시는 장면/설명 1.2:1의 열(66px 간격)이고 장면 높이는 360px다. 서버 목록은 3열(20px), 상세는 1.6:1(65px)이다. 이것은 단일 수학적 간격 스케일이 아닌 구현에서 사용한 역할별 값이다.

- ≤1100px: 좌우 여백 32px, 홈 열 비율 0.7:2와 간격 24px, 활동 열 간격 35px. 검색·필터 도구를 세로로 배치한다. 홈 제목은 45px다.
- ≤950px: 홈을 1열로 바꾸고 위 여백 36px, 간격 34px, 제목 48px를 사용한다. 체험 연결 영역은 32px, 활동 열 간격은 28px다.
- ≤800px: 진입 링크 1열, 서버 목록 2열, 내비게이션 높이 74px, 페이지 제목 34px다.
- ≤700px: 활동 장면과 설명을 1열, 20px 간격으로 배치한다.
- ≤560px: 좌우 여백 20px, 내비게이션은 브랜드 아래 한 줄. 서버 목록·상세·주소 설명은 1열. 체험은 내 화면을 기본으로 유지하며 선택 버튼으로만 관점을 바꾼다. 연결선 대신 화면 밖 수신 상태를 표시한다. 창·답글 높이는 그대로 유지한다. 홈 상하 여백 32/30px, 제목 43px; 페이지 제목 31px. 활동 장면은 350px이며 활동 선택은 줄바꿈한다.

## Elevation & Depth

UI 패널에는 그림자를 쓰지 않는다. 흰 패널, 옅은 면, 1px 경계와 구분선으로 영역을 나눈다. 서버 카드는 호버 시 경계와 배경만 바뀌며 떠오르거나 확대되지 않는다. 예외는 실물 형태를 나타내는 큰 책 표지의 작은 그림자 (`3px 7px 12px #253d3026`)다. 축소 표지에는 그림자를 쓰지 않는다.

## Shapes

컨트롤·버튼은 작은 둥근 모서리, 서버 카드는 panel, 체험 창·활동 바탕은 scene 곡률을 쓴다. 필터는 알약 모양이다. 멍멍.타운의 표식·인물 아바타는 둥근 사각형, 냥냥.타워는 원형이며 이름도 표시한다. 인물 이미지는 기본 36px, 작성자 32px, 하단 계정 30px, 대기 중 친구 58px로 같은 이미지를 재사용한다. 책 표지는 기하 도형과 타이포그래피로 만든 사물이며 4px 책등과 비대칭 모서리(1/4/4/1px)를 유지한다. 장면·체험 창은 내부 콘텐츠를 모서리에서 자른다.

## Components

- **Buttons:** 주/보조 버튼은 최소 높이 47px. 주 버튼 호버는 더 짙은 보라색, 보조 버튼은 얇은 경계와 옅은 면이다. 비활성 버튼은 불투명도 0.5와 기본 커서. 텍스트 링크는 호버 시 밑줄과 강조색을 사용한다.
- **Filters / search:** 필터는 선택 시 진한 바탕·흰 글자이며 `aria-pressed`를 제공한다. 검색은 아이콘을 포함한 라벨 경계에 `focus-within`을 표시한다. 이름·관심사·플랫폼 검색과 관심사 필터가 결합되며 빈 결과에는 초기화 버튼이 나온다.
- **Navigation:** 텍스트 메뉴는 현재 경로에서 강조색과 밑줄을 함께 사용한다. 상세 화면에서도 서버 찾기 메뉴가 활성화된다. 숨겨진 본문 건너뛰기 링크는 포커스 시 나타난다.
- **Cards / notices:** 서버 카드 전체가 상세 링크이며 표식, 플랫폼, 이름, 소개, 관심사·가입 방식을 담는다. 가상 데이터 안내와 가입 불가 버튼을 유지한다. 상세 가입 영역은 `wash` 바탕의 패널이다.
- **Federation scene:** 홈과 학습 경로가 같은 컴포넌트를 사용한다. SSR은 편집 가능한 인사 작성 장면으로 시작하되 수화 전 입력·제어는 비활성이다. 공백뿐인 글은 보낼 수 없으며 최대 90자다. 보내기를 누르면 작성→전송 중→내 서버 게시→친구 서버 수신→친구 작성 중→답장 전달→내 서버 답장 도착의 7단계를 진행한다. 동일한 글과 작성자 주소를 양쪽에 유지하며 실제 네트워크 전송은 하지 않는다. 수동 다음·단계 점 선택은 없다. 진행 중 일시정지/계속, 시작 후 다시 하기를 제공하고 마지막에는 멈춘다. 다시 하기는 작성 문구를 유지하고 내 관점으로 돌아온다. 마지막 마음 보내기만 로컬 토글이며 게시물의 반응 아이콘은 장식이다.
- **Composer / perspectives:** 전송과 재생 제어는 최소 높이 42px다. 모바일 관점 버튼은 최소 44px, 선택된 서버의 색·밑줄·`aria-pressed`를 함께 사용한다. 단계가 진행되어도 선택한 화면을 자동으로 바꾸지 않는다. 상태 문장은 `role="status"`로 알리며 모바일 수신 표시는 이를 보조한다.
- **Activity examples:** 홈에는 BookWyrm 독서 기록과 같은 내용의 Mastodon 수신 화면을 오가는 예시 하나만 둔다. `/platforms`는 독서·게시판·행사/모임·사진·영상·일상/대화의 활동 선택을 제공한다. 책장, 게시판, 행사, 사진, 영상 표지를 서로 다른 형태로 표현한다. 선택 변경은 해당 예시의 조작 상태를 초기화한다. 구독·마음·모임 소개는 로컬 예시이고 영상은 재생되지 않는 표지다. 가상 콘텐츠와 호환 범위 안내를 유지한다.
- **Motion:** 200ms 타이머로 활성 단계에 각각 0.8/1.8/2.2/1.8/1.6초를 배정한다(중단 없는 전송 후 진행 총 8.2초). 일시정지는 타이머 진행과 CSS 애니메이션을 함께 멈춘다. 게시 진입은 500ms/아래 9px, 원격 수신은 800ms/왼쪽 28px, 돌아온 답글은 800ms/오른쪽 28px에서 불투명도 0.25→1로 들어온다. 이동 완화는 `cubic-bezier(0.16, 1, 0.3, 1)`, 연결점 왕복은 1.1초다. 서버 카드 색 전환은 180ms. 시연의 짧은 상태 전환과 부드러운 스크롤은 OS `prefers-reduced-motion` 설정과 관계 없이 항상 재생한다.
- **Focus / assets:** 기본 키보드 포커스는 3px 보라색 외곽선과 5px 간격이다. 작성 입력은 3px 간격, 검색은 외곽 라벨에 2px 포커스를 표시한다. Lucide SVG와 코드로 만든 미니 UI에 제작한 래스터 아바타(`shuna.png`, `patricia.png`)와 동네 장면(`neighborhood.png`)을 함께 쓴다. 장면 이미지는 행사·사진·영상에서 크롭을 달리해 재사용하고 지연 로딩한다. 인물 옆 이름이 있으므로 아바타 대체 텍스트는 비우고, 장면에는 예시임을 밝히는 대체 텍스트를 제공한다. 출하 래스터의 내장 출처 정보를 보존한다. 책 표지는 이미지가 아닌 코드 기반 사물이다.

## Do's and Don'ts

- Do 간결한 한국어와 이름·형태·색이 함께 전달하는 구분을 유지한다.
- Do 가상 데이터 표시, 명시적 재생, 키보드 포커스와 모션 감소 처리를 유지한다.
- Don't 데모의 정적인 반응 아이콘이나 가입 불가 버튼을 실제 서비스 기능처럼 설명하지 않는다.
- Don't 이 문서를 실시간 서버 데이터나 새 브랜드 방향이 확정되었다는 근거로 사용하지 않는다.
> 이 문서는 이전 시안의 기록입니다. 현재 의사결정에는 PRODUCT.md와 docs/cutover-readiness.md를 우선합니다. 승인된 랜딩을 아래의 과거 시안으로 되돌리지 마세요.
