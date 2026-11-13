# DS-01 코드용 토큰 추출

- 상태: 초안, 메인 평가 대기
- 범위: DS-01. CSS·TypeScript 생성과 대비 검사는 DS-13이다.

## 결정

원본은 `design/tokens.json` 하나로 유지한다. 코드가 읽을 추출물은 메인 결정에 따라 `apps/desktop/design-tokens/tokens.json`에 둔다. `src/**`의 `local/forbidden-file` 규칙을 유지하기 위해 JSON 입력을 소스 밖에 둔다. 원본은 수정하지 않는다. DS-13의 `xtask gen-tokens`는 이 경로를 읽고 `apps/desktop/src/shared/ds/tokens.css`와 이름 유니온만 담는 `tokens.ts`를 생성한다.

architecture 5.2의 “design/의 tokens.json에서 생성”과 R-14 2.2의 `design/claude-design/tokens.json` 직접 입력 설명에서 달라지는 점은 검증된 코드용 추출물을 한 단계 거친다는 것이다. 디자인 원본의 실제 경로는 `design/tokens.json`이며, 코드용 파일은 별도로 수정할 수 있는 원본이 아니다. 원본의 공통 값을 두 테마로 정규화한 내용과 검사기 안의 닫힌 유도 목록만 허용하고, 전체 객체의 정확한 일치를 검사한다. 원본 값 변경·누락과 승인되지 않은 추가 토큰은 실패한다. architecture 본문 수정은 메인이 한다.

원본의 그룹, 토큰 이름, 글자 스타일, 글꼴 경로와 메타데이터를 보존한다. 모든 `*.tokens[].value`는 `light`·`dark` 두 키를 가진 객체로 통일한다. 테마에 공통인 값은 양쪽에 같은 값을 둔다. 글꼴과 복합 글자 스타일인 `type`은 원본 구조를 유지하며 두 테마가 공유한다. 글꼴의 상대 경로는 원본 `design/` 기준이다. DS-13 생성기가 앱 기준으로 해석해서는 안 된다.

이름은 소문자·숫자·하이픈을 사용한다. 색 참조는 원본의 `{primary}` 형식을 유지하며 같은 색 그룹의 같은 테마에서 해석한다. 미해결 참조와 순환은 거부한다. px, ms, 단위 없는 정수·비율, 원본의 `bubble-max-width` 백분율을 각 그룹의 형식으로 검사한다. 원본을 바꾸면 추출물과의 일치 검사도 다시 통과해야 한다.

## 유도 근거

`space-7`, `space-9`, `space-11`은 원본 JSON에는 없다. design/README.md의 “4px 기준, space-1 … space-12”와 architecture 5.1의 같은 규칙으로 각각 7×4=28px, 9×4=36px, 11×4=44px를 유도한다. 두 테마에 동일하다. 나머지 값은 원본에서 그대로 추출한다. 새 색·크기·겹침 순서를 임의로 정하지 않는다.

## 검사와 후속 작업

`harness/ds-tokens/check.mjs`는 원본 보존, 유도값, architecture 5.1에 적힌 토큰 이름, 테마 키, 이름·값·참조 형식을 검사한다. `apps/desktop/tests/ds-tokens.test.mjs`는 각 원소 삭제, 원본 값 변경, 승인되지 않은 추가 토큰, 테마 삭제, 잘못된 형식·참조, 문서 표의 토큰 추가 변형을 검사한다. 검사기의 닫힌 유도 목록이 정확히 7·9·11인지도 독립된 기대값으로 확인하므로 목록의 원소를 빼는 변형이 실패한다. 각 유도 토큰을 추출물에서 빼는 변형도 실패한다.

추출물에 포함된 글자·움직임·불투명도·백분율을 DS-13이 처리해야 한다. R-14 조사 시점 이후 원본에 추가된 그룹이다.

## 메인·사용자에게 넘길 것

디자인의 본문 여백(space-6)과 architecture 5.1의 화면 가장자리(space-8)가 다른 점, 디자인에서 설정을 Dialog로 설명하는 점은 토큰 추출로 결정하지 않는다. 화면 구현은 architecture와 open-issues의 우선순위를 따른다.

## R-14 4.3 빠진 토큰의 현재 상태

2026-11-13의 `design/tokens.json`과 `design/components/bundle.css`를 직접 대조했다. 아래 값은 이번 작업에서 추가하지 않았다.

| 항목 | 현재 상태 |
|---|---|
| 움직임 | `duration-fast/base/flash`(120/200/600ms), `ease-out/in`이 추가돼 해결됨 |
| 2px 틈 | `gap-hairline` 2px로 해결됨. `space-0.5`는 없음 |
| 작은 컨트롤·촘촘한 행 | `control-height-sm` 28px, `row-height-dense` 32px로 해결됨 |
| 작은 버튼 좌우 여백 | `control-pad-sm` 8px가 있음. 과거 10px 대신 현재 bundle.css가 이 토큰을 사용 |
| 막대·점·아이콘 | `bar-height` 12px, `dot-size` 8px, `icon-sm/md/lg` 16/20/24px로 해결됨 |
| 비활성 불투명도 | `opacity-disabled` 0.45로 해결됨 |
| 포커스 링 | 제안 이름 대신 `border-focus` 2px, `focus-offset` 2px로 해결됨 |
| 겹침 순서 | `z-sticky/panel/popover/modal/toast` 10/100/200/300/400으로 해결됨 |
| 창·채팅 폭 | `window-min-width/height` 960/640px, `chat-width-min/max` 320/560px로 해결됨 |
| 버튼 글자 스타일 | `button` 복합 스타일은 없음. 현재 CSS는 `text-md`, `weight-semibold`, 단위 없는 줄높이 1을 조합함 |
| 범례 최소 폭 | `legend-min`은 없음. 현재 CSS 범례는 flex와 줄바꿈을 사용하며 150px 격자 최소 폭은 없음 |
| 차트 축·눈금 | 전용 `chart-grid`, `chart-axis`는 없음. 현재 CSS가 `line`, `line-strong`, `ink-subtle`로 대신함 |
| 차트 십자선 | `chart-crosshair`는 여전히 없음 |
| 차트 선 굵기 | 전용 `stroke-chart`, `stroke-baseline`은 없음. 현재 CSS의 선은 `border-mark` 1.5px, 축·기준선은 `border-hairline` 1px로 대신함 |

## DS-13이 정할 대응 규칙

입력은 `apps/desktop/design-tokens/tokens.json`이다. 다음은 R-14 4.1의 대응표만으로 확정할 수 없으며 DS-13에서 정한다.

- `duration`, `easing`, `zIndex`, `opacity` 그룹의 CSS 변수·Tailwind 이름 대응.
- `text` 그룹과 `type.groups`의 이름 겹침. 예를 들어 원시 크기 `text-sm`과 복합 스타일 이름을 같은 `--text-*` 공간에 넣을 때의 규칙.
- `layout`의 창·본문 `bp-*` 구간과 단위 없는 `grid-columns`의 처리.
- `size`에 섞인 폭·높이·테두리·아이콘과 `bubble-max-width`의 80% 처리. 일괄 px→rem 변환을 적용할 수 없다.
- `shadow-popover` 이름 자체에 `shadow-`가 있으므로 `--shadow-`와 결합할 때 접두어가 겹치지 않도록 할 규칙, R-14의 테마별 원값 참조 방식.
- R-14 4.2의 색 값은 현재 원본과 다르다. 예를 들어 라이트 `line-strong`은 보고서의 `#8a8c86` 대신 현재 `#7a7c76`, `chart-1`은 `#0b7258` 대신 `#0b5e49`다. R-14 4.2를 기준값으로 사용하지 않고 현재 디자인 원본과 정확히 일치하는 코드용 추출물을 읽는다.
