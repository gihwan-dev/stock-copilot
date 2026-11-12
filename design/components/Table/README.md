# Table

일반 표예요. 보유 종목은 HoldingsTable, 성적표는 ScorecardTable을 쓰고, 그 밖의 목록은 이걸 써요.

- 제공: `columns`(`{key, header, kind?, align?, currency?, signed?}`), `rows`(값은 코어가 준 문자열, 행에 `currency`가 있으면 그 통화), 선택 `caption`, `empty`, `dense`.
- `kind`: `text`(기본) · `money` · `percent` · `quantity` · `probability` · `change`(PriceChange props). 숫자 열은 오른쪽 정렬.
- 첫 열은 행 머리(`th scope="row"`). 행이 없으면 `empty` 문구를 보여줘요.
