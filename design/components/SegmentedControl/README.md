# SegmentedControl

차트 기간이나 보기 기준처럼 2–5개 중 하나를 고르는 컨트롤이에요.

- 제공: `options`(문자열 또는 `{value, label}`), `value`+`onChange`(제어) 또는 `defaultValue`(비제어), 접근성 `label`.
- 선택은 `aria-pressed`로 전달돼요. 6개 이상이면 드롭다운을 쓰세요.
