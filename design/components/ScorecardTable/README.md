# ScorecardTable

확률 구간별로 AI가 얼마나 맞혔는지 보여주는 성적표예요.

- 제공: `rows`(`{band, predicted, actual, count, judged}`), 선택 `caption`, `empty`.
- `judged`는 코어가 정해요. `false`면 실제 적중률 대신 “판정 보류”를 보여줘요. 화면은 건수를 세거나 기준과 비교하지 않아요.
