# 주 화면 · 1000px

창 1000px: 종목 목록이 56px 레일로 접혀요. Grid 896px = narrow 구간이라 카드가 한 줄씩 쌓여요.

- 조립: `AppShell`(StockList · Page · ChatPanel) → `Page`(제목 “현황”, 동작: 성적표·설정·채팅 열기) → `Section` “현황”(`Grid`: 총 평가 · 통화별 평가 · 오늘 손익 · 보유 종목 · 자산배분) → `Section` “오늘”(SignalCard · DisclosureCard · BriefingCard, 최근 것부터).
- 위쪽 “화면 상태” 전환은 미리보기 전용이에요. 정상 · 로딩(Skeleton) · 빈(EmptyState) · 오류(StatusBar 한 줄 + 실패 Callout).
