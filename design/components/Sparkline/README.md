# Sparkline

테이블 행이나 관심종목 옆에 붙는 작은 추세선이에요. 축과 눈금이 없어요.

- 제공: `data`(숫자 배열, 오래된 값부터), 선택 `baseline`(전일 종가), `direction`(코어가 정한 방향 `up`·`down`·`flat`), `width`·`height`(기본 96×28).
- 점을 픽셀로 옮기는 그리기만 해요. 색은 `direction`을 따르고, 없으면 마지막 값과 `baseline` 비교로 정해요. 등락 숫자와 색이 다르면 안 되니 `direction`을 넘기는 걸 권해요.
- 정확한 값을 읽어야 하면 Sparkline 말고 차트를 쓰세요.
