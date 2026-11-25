# fin-design 묶음 결정 기록

디자인 점검 묶음(사용자 지시: AI 슬롭 제거, 창 높이·배경 문제가 구조적으로 생길 수 없게)에서 문서가 정하지 않은 것을 정한 기록이다.

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| FD-1 | 창 전체 높이·배경은 새 DS 원시 요소 `AppFrame` 하나만 맡는다. `App`이 모든 화면을 `AppFrame`으로 감싸고, 경로 화면은 `AppShell`(스크롤 영역 소유) 또는 `StepScreen`(스크롤 + 하단 고정 버튼) 안에서만 그린다. 설정·상태 경로와 오류 대체 화면도 `AppShell` 안으로 옮겼다. `AppShell`·`StepScreen`·`Page`는 `bg-bg`와 `h-screen`을 더 쓰지 않는다 | `theme.css` base 층에 html/body/#root 배경·높이 지정(고정 theme·DS02/DS19 CSS 게이트가 `vh`와 theme 변경을 막고, 화면마다 높이를 따로 정하는 구조는 그대로 남음) |
| FD-1 | ESLint 넘침 정책 #22(창 단위 길이)의 허용 파일을 `AppShell`·`StepScreen`에서 `AppFrame` 하나로 줄였다. `AppShell`·`StepScreen`에 `h-screen`/`min-h-screen`을 쓰면 실패하는 위반 예제를 `harness/eslint-violations/ds19-overflow.json`에 더했다. 기능 화면은 원래 `className`을 쓸 수 없으므로 배경·높이·바깥 여백을 직접 정할 길이 없다 | 기능 화면용 새 규칙(이미 클래스 자체가 막혀 있어 중복) |
| FD-2 | 창 채움 검사는 `shared/ds/testing/overflow-geometry.ts`를 늘리지 않고 `harness/screen-audit/window-fill.mjs`에 둔다. 앱 소스 규칙(DOM 값 접근·원시 타입 금지)과 250줄 상한 때문에 브라우저 측정 코드는 하네스 쪽이 맞다 | `overflow-geometry.ts` 확장(줄 수 상한 초과, 보호 파일 허용 목록 추가 필요) |
| FD-2 | 검사 항목: `[data-app-frame]`이 있고 배경이 칠해져 있으며 창과 같은 크기, 문서 자체의 가로·세로 스크롤 없음, 세로 스크롤 영역 안 가로 잘림 없음, `[data-pinned-actions]`(StepScreen 하단 버튼)이 창 안에서 보이고 가려지지 않음, 대화상자가 창 안. 점 찍어 보기(elementFromPoint)로 배경을 재는 방식은 모달 배경막이 pointer-events를 끄면 오탐해서 버렸다 | 화면 픽셀 비교(테마별 기준 색 추정이 불안정) |
| FD-2 | 장면은 `harness/screen-audit/scenarios.mjs`에 기존 노드 시험의 가짜 자료를 옮겨 28개(온보딩 4, 현황 6, 업데이트 시트, 종목 상세 3, 브리핑 3, 거래내역 3, 설정 3, 상태 3 등)로 둔다. 시험 `apps/desktop/tests/screen-window-fill.test.mjs`가 라우터의 모든 `path`에 장면이 있는지 확인하고, 모든 장면을 1440×900·1024×700·960×640(최소 창) × 라이트·다크로 검사한다. `node --test apps/desktop/tests/*.test.mjs`에 들어 있어 `check-lite`가 돈다 | 별도 just 레시피(justfile 고정 기대값 시험이 막음) |
| FD-2 | 브라우저는 저장소에 있는 `@playwright/test`의 Chromium을 먼저 쓰고, 그 판이 설치돼 있지 않으면 같은 Playwright로 시스템 Chrome(`channel: "chrome"`)을 쓴다. 새 패키지 없음 | 브라우저 내려받기 |
| FD-2 | 스크린샷은 `node harness/screen-audit/run.mjs <이름>`이 `target/screen-audit/<이름>/`(git 무시)에 1440×900·1024×700 라이트·다크로 남긴다 | `design/audit/`(git 무시 경로가 아님) |
