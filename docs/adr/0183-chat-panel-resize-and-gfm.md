# ADR 0183: 채팅 칸을 머리·대화·바닥 입력창으로 나누고 답에 GFM을 쓴다

- 상태: 확정 (2026-11-26). 사용자가 "우측 채팅 사이드바 리사이즈 가능하게, UI 라이브러리 써서, 채팅 창과 버튼은 맨 아래, 마크다운 렌더링 잘 되게"를 요청했다. 너비 조절은 처음에 보류했다가, 사용자 요청이 보호 파일 변경을 승인하는 것으로 보고 같은 날 7항으로 더했다.

## 배경

오른쪽 AI 채팅 칸은 긴 전송 안내가 위를 차지하고, 입력창과 보내기 버튼이 대화 바로 아래(칸 가운데)에 떠 있었다. 답은 CommonMark만 그려서 표가 문단으로 무너졌다.

## 결정

1. `AppShell`에 `chatComposer`와 `chatNotice`를 더한다. 둘을 주면 채팅 칸은 머리(제목, 한 줄 요약 안내, 펼치면 보이는 전체 전송 안내), 스크롤되는 대화, 맨 아래 고정 입력창이 된다. 전체 안내는 접혀 있어도 `hidden`으로 DOM에 남아 U-17 고정 문구 시험을 지킨다. 주지 않으면 기존 배치 그대로다.
2. 기능 쪽은 `ChatSession`(렌더 함수)이 대화 상태를 갖고 `messages`와 `composer`를 따로 넘긴다. 기존 `ChatPanel`은 이것을 세로로 쌓은 것이라 기존 화면 시험을 지킨다. 입력 초안은 세션이 가져서 빈 화면의 추천 질문이 입력창을 채운다(보내지는 않는다).
3. `ChatInput`에 `layout="compact"`(숨은 라벨 대신 `aria-label`·자리표시자, 아래 줄에 Enter 안내와 작은 보내기 버튼)와 제어형 `draft`/`onDraftChange`를 더한다. 기본값은 기존 모양이다.
4. `remark-gfm` 4.0.1을 `shared/ds/Markdown.tsx`에서만 쓴다. `flavor="gfm"`일 때만 켜고, 기본(`basic`)은 보호된 DS18 시험이 지키는 "표 없음"을 유지한다. 표는 가로 `ScrollArea` 안에 그린다. 원시 HTML은 계속 `skipHtml`로 버리고, 링크·맨 주소·각주는 모두 `onOpenLink`(open_external) 버튼이 되며 `<a>`를 만들지 않는다. 각주 제목("Footnotes")은 숨기고 할 일 목록의 체크 칸은 그리지 않는다. `components`의 표 키는 `"table"`로 적는다. 그대로 쓰면 Tailwind가 `table`을 클래스 후보로 읽어 `.table` 규칙을 만들고 DS 검토 시험이 실패한다.
5. 질문은 오른쪽 정렬 말풍선(`Markdown appearance="question"`), 답은 배경 없는 본문으로 구분한다. 답이 아직 비어 있으면 뼈대 줄을 보인다.
6. 보호된 `eslint/ds-public-props.mjs`에서 `react-markdown`의 `remarkPlugins`를 `components`와 같이 `Markdown.tsx`에서만 허용한다.
7. 너비 조절은 `react-resizable-panels` 4.21.0(2026-11-08 배포, 런타임 의존성 없음)을 `shared/ds/AppShell.tsx`에서만 쓴다.
   - 창이 `window-lg`(80rem) 이상이고 `ResizeObserver`·`matchMedia`가 있을 때만(`useShellResize`) 본문과 채팅을 `Group`/`Panel`/`Separator`로 나눈다. 그 밖(좁은 창, jsdom 시험)에서는 기존 겹침 패널을 그대로 쓴다. 라이브러리는 `ResizeObserver` 없이 마운트하면 실패한다.
   - 크기는 `ds/no-style-literals`·`ds/public-props` 때문에 단위 없는 백분율 문자열만 넘긴다. 채팅 칸은 25~50%(`chatPanelLimits`), 본문은 최소 40%다. 처음 너비는 `384px / (창 너비 - 사이드바)`를 범위 안으로 맞춘 값이다. `main="fill"` 화면(홈, 거래 내역)은 패널 안에서도 창을 채우고 페이지 스크롤이 없다(`screen-dashboard-fill.test.mjs`).
   - 너비는 `shared/platform/input.ts`가 `localStorage`의 `stock-copilot.chat-share`에 백분율로 저장하고 읽으며, 읽은 값은 `clampPanelShare`로 범위 안으로 맞춘다. `try`는 끌 수 없는 린트 규칙이라 쓰기는 동기 실행되는 `Promise` 실행자 안에서 하고, 용량 초과·차단 오류는 거부로 버린다. 읽기는 `typeof` 확인만 한다(Tauri WebView에는 저장소가 늘 있다).
   - 손잡이는 라이브러리의 `role="separator"`·`tabindex="0"`·화살표 키 조절을 쓰고, 두 번 누르면 저장값을 지우고 처음 너비로 되돌린다.
   - 대화 목록은 맨 아래 근처(48px 안)에 있을 때만 새 내용을 따라 내려가고, 사용자가 위로 올리면 멈춘다(`useFollowEnd`).
8. 보호 파일 변경(별도 커밋): `harness/ts-deps/allowlist.json`에 `react-resizable-panels`, `eslint/ds-public-props.mjs`에서 이 라이브러리를 Base UI와 같은 신뢰 라이브러리로 보고, `ds-screen-bootstrap.test.mjs`의 `platform/input.ts` 내보내기 고정 목록에 `chatPanelLimits`, `useShellResize`, `initialChatShare`, `readChatShare`, `saveChatShare`, `clearChatShare`, `useFollowEnd`를 더한다.

## 지키지 못한 것

- 코드 블록 가로 스크롤: DS가 `pre`에 `chat-content`(줄바꿈)를 요구한다. 고정폭 글꼴로 줄을 바꿔 보여 준다.
- 입력창 자동 높이: `field-sizing`과 DOM 스타일 변경이 DS 규칙으로 막혀 있다. 기존 `chat-input`(최소 4rem, 최대 10rem, 넘치면 스크롤)을 쓴다.
- 손잡이 픽셀 최소·최대 너비: 숫자 props와 단위 문자열이 DS 규칙에 걸려 백분율로 둔다.
- 메시지 시각: 기록의 시각을 화면에서 바꾸려면 숫자 변환이 필요하다. 코어가 표시용 시각을 보내게 될 때 붙인다.
