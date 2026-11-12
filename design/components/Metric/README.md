# Metric

라벨, 큰 숫자, 등락, 기준 시각을 한 묶음으로 보여주는 지표예요. 대시보드 상단 요약 줄에 써요.

- 제공: `label`, `value`(`format.money`·`format.krwShort`로 만든 문자열), 선택 `change`(PriceChange props), 선택 `hint`(기준 시각·출처).
- `value`가 잘못된 값이라 `format`이 `—`를 돌려주면 주황 `—`로 보여요.
- `display`는 총 평가금액 한 곳에만(`num-display`).
- 컨테이너(`sc-card` 등)는 쓰는 쪽이 감싸요.
- `hint`에 기준 시각을 꼭 넣어요. 환산값이면 “≈”와 환율 기준도 넣어요. `krwShort`를 쓰면 근사값이니 정확한 금액은 다른 곳(표)에 있어야 해요.
