# 주 화면 · 1180px

창 1180px: 채팅이 오버레이로 바뀌고 닫혀 있어요(머리줄의 열기 버튼). Grid 900px = medium 구간이라 총 평가와 통화별 평가가 6열씩 나란히 와요.

- 조립: `AppShell`(StockList · Page · ChatPanel) → `Page`(제목 “현황”, 동작: 성적표·설정·채팅 열기) → `Section` “현황”(`Grid`: 총 평가 · 통화별 평가 · 오늘 손익 · 보유 종목 · 자산배분) → `Section` “오늘”(SignalCard · DisclosureCard · BriefingCard, 최근 것부터).
- 위쪽 “화면 상태” 전환은 미리보기 전용이에요. 정상 · 로딩(Skeleton) · 빈(EmptyState) · 오류(StatusBar 한 줄 + 실패 Callout).
