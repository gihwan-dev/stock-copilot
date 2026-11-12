# HoldingsTable

보유종목 표예요. 모든 값은 코어가 계산하고 반올림해서 넘기고, 이 컴포넌트는 표시만 해요. 행마다 원래 통화로 보여줘요.

- 제공: `rows`(`{id, name, ticker, market?, currency, qty, avgPrice, price, value, pl, rate, weight}`, 숫자는 모두 문자열), 선택 `total`(`{value, pl, rate, fx?: {pair, rate, at}}`, 원화), `dense`(`row-height-dense`), `selectedId`, `onSelect(row)`.
- 셀 하나가 잘못된 값이면 그 셀만 주황 `—`이 되고 오류가 올라가요. 행은 숨기지 않아요.
- 소수점 주식 수는 받은 그대로 보여요(`0.5주`, 끝의 0만 지워요).
- 원화·달러 종목이 섞이면 코어가 원화 환산 합계를 `total`로 넘기고 `fx`를 채워요. 합계엔 `≈`와 환율·기준 시각, “참고 환율” 안내가 붙어요. 행 금액은 환산하지 않아요.
- 종목명 칸은 행 머리(`th scope="row"`)라 화면 낭독기가 셀마다 종목명을 같이 읽어요.
- `onSelect`가 있으면 행이 Tab으로 포커스되고 Enter·Space로 선택돼요. 선택된 행은 `primary-soft` 바탕과 `aria-current="true"`.
- 정렬·필터·가상 스크롤은 쓰는 쪽에서 처리해요.
