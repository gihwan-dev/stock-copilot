# Toast

잠깐 떴다 사라지는 알림이에요. 체결 알림처럼 지금 보고 있는 곳 밖에서 일어난 일을 알릴 때만 써요.

- 제공: `tone`(`info`·`success`·`error`), `title`, `children`, `onClose`.
- AppShell의 `toast`에 넣으면 오른쪽 아래(`z-toast`)에 떠요. 폭 `chat-width-min`.
- 실패는 Toast로만 끝내지 않아요. 원래 자리(신호 카드 등)에도 실패 상태가 남아 있어야 해요.
