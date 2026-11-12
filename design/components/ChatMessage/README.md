# ChatMessage

AI 채팅의 메시지 하나예요. 사용자 메시지는 오른쪽 `surface-sunken` 말풍선, AI 답변은 말풍선 없이 본문(Markdown)으로 써요. 보통은 ChatPanel이 그려요.

- 제공: `role`(`user`·`assistant`), `children`(AI 답변은 마크다운 문자열), 선택 `activity`(AI가 무엇을 확인했는지 사용자 말 한 줄: “보유 종목과 자산배분을 확인했어요”), `sources`(`{label, href?}`), `note`(면책 한 줄), `streaming`(끝에 커서).
- 내부 도구 이름(`get_holdings` 같은)은 화면에 쓰지 않아요. 예전 `tools` 배열을 넘기면 그대로 배지가 아니라 한 줄 글자로 보여주니, 사용자 말로 바꿔서 넘겨주세요.
- AI가 쓴 숫자는 앱이 계산한 값에서 온 것만 쓰고 출처를 `sources`로 남겨요. AI는 주문을 제안하지 않아요. 확률과 근거는 SignalCard가 보여줘요.
- 컨테이너(채팅 패널, 스크롤)는 쓰는 쪽이 제공해요. 폭은 `chat-width`.
