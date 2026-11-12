# ChatInput

AI에게 보낼 말을 쓰는 여러 줄 입력이에요.

- 제공: `onSend(text)`, `onStop`, `streaming`(답변 중이면 보내기 대신 멈추기 버튼), `disabled`, `placeholder`, `value`+`onChange`(선택).
- Enter로 보내고 Shift+Enter로 줄을 바꿔요. 한글 조합 중 Enter는 보내지 않아요.
- 높이 `chat-input-min`–`chat-input-max`, 넘치면 안에서 스크롤.
