# TextField

한 줄 입력이에요. 비밀 값(토스 API 키·시크릿)은 `type="secret"`으로 받아요.

- 제공: `label`, `value`+`onChange` 또는 `defaultValue`, `placeholder`, `hint`, `error`, `disabled`, `type`(`text`·`secret`).
- `secret`: 가려서 보여주고, 자동 완성·맞춤법 검사를 끄고, “붙여넣기” 버튼으로 클립보드를 읽어요. 보기 버튼은 값이 있을 때만.
- `error`가 있으면 테두리가 `alert`이고 문구가 `role="alert"`로 읽혀요. 문구는 무슨 일 → 어떻게 고칠지. 내부 오류 코드는 쓰지 않아요.
- 바탕 `surface-sunken`, 테두리 `line-strong`(3:1 이상).
