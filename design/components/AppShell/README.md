# AppShell

창 전체 틀이에요. 왼쪽 종목 목록, 가운데 본문, 오른쪽 AI 채팅을 배치하고, 창은 스크롤하지 않고 본문과 채팅이 각자 스크롤해요.

- 제공: `list`(StockList), `chat`(ChatPanel), `children`(Page 하나), 선택 `toast`(Toast), `chatOpen`·`defaultChatOpen`·`onChatOpenChange`.
- 창 폭 구간(`bp-window-lg`, `bp-window-md` 토큰을 읽어 JS로 판정):
  - 1280px 이상: 셋 다 표시. 채팅은 `chat-width`(320–560px).
  - 1080–1279px: 채팅이 본문 위 오버레이(`z-panel`)가 되고 기본으로 닫혀요.
  - 1080px 미만: 종목 목록이 `sidebar-collapsed`(56px) 레일로 접혀요. 레일엔 종목 이름 앞 두 글자와 ▲▼만 보이고, 펼치기 버튼을 누르면 목록 전체가 본문 위로 펼쳐져요.
- `useAppShell()`로 `chatOpen`, `setChatOpen`, `windowBand`, `listCollapsed`를 읽어요. Page의 동작 버튼에 채팅 열기 버튼을 둘 때 써요.
- Esc는 오버레이 채팅과 펼친 목록을 닫아요.
- 창 최소 크기는 `window-min-width`×`window-min-height`(960×640). Tauri 설정에도 같은 값을 넣어요.
