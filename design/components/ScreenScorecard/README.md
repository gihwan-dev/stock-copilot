# 성적표

AI 확률이 실제로 얼마나 맞았는지 보는 보조 화면이에요. 종목 상세와 같은 방식으로 본문 자리에 열리고, 종목 목록과 채팅은 그대로예요. 설정은 Dialog 그대로예요.

- 조립: `Page`(제목 “성적표”, `leading` 뒤로 가기 “현황으로”) → `Toolbar`(모델·기간 `SegmentedControl`) → `Grid`: 요약 `Metric` 4개(narrow·medium 6열, wide 3열) · `ScorecardTable` 구간별 적중률(6열) · `ReliabilityChart`(6열) · 신호 수익률과 벤치마크 `LineChart`(12열).
- 판정 보류는 코어가 준 `judged`로만 정해요.
- 상태: 로딩(Skeleton) · 빈(판정된 신호 없음, 기록 진행 상황 한 줄) · 오류(StatusBar + 실패 Callout) · 정상.
