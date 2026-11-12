# LineChart

시간에 따른 값을 선으로 그려요. 여러 계열은 `chart-1`부터 순서대로 칠해요(최대 6).

- 제공: `series`(`{name, values}`, 값은 문자열), `labels`(x축, 처음과 끝만 표시), `height`(`chart-height-sm`·`chart-height`·`chart-height-lg`), `format`(`money` 기본·`percent`·`number`), `currency`, `empty`, `label`.
- 세로축 이름표는 데이터의 **가장 큰 값과 가장 작은 값 문자열** 그대로예요. 눈금을 만들려고 반올림하지 않아요.
- 값이 2개 미만이면 빈 상태를, 잘못된 값이 있으면 “차트 값을 읽을 수 없어요”를 그려요.
