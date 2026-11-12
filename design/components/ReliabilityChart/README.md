# ReliabilityChart

AI가 낸 확률 구간(가로)과 실제로 맞힌 비율(세로)을 점으로 찍어요. 점선 대각선에 가까울수록 확률을 믿을 만해요.

- 제공: `points`(`{predicted, actual, count, judged}`, 확률은 0–100 정수 문자열), `height`, `empty`.
- `judged`는 코어가 정해요(판정에 필요한 건수가 쌓였는지). `false`인 점은 속이 빈 점으로 그리고 “판정 보류”로 표시해요. 컴포넌트는 건수를 보고 판단하지 않아요.
