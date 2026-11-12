# ChatPanel

오른쪽 AI 채팅 패널이에요. 머리줄, 대화 기록, 입력창으로 이뤄져요.

- 제공: `messages`(`{role, content(마크다운), activity?, sources?, note?}`), `status`, `activity`(조회 중 문구), `error`, `onSend`, `onStop`, `onRetry`, `disabled`.
- `status`:
  - `idle`: 기본.
  - `looking`: 답을 쓰기 전에 데이터를 보는 중. “보유 종목을 확인하는 중” 같은 한 줄(도구 이름은 쓰지 않아요).
  - `streaming`: 답변이 들어오는 중. 마지막 답 끝에 커서, 입력창엔 멈추기 버튼.
  - `error`: 실패 Callout과 다시 시도.
- AppShell 안에서는 닫기 버튼이 생겨요.
