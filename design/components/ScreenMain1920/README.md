# 주 화면 · 1920px

창 1920px: 셋 다 보이고 본문 1304px에서 여백을 뺀 Grid가 content-max(1200px)에 닿아 wide 구간이에요. 총 평가·통화별 평가·오늘 손익이 4열씩 한 줄, 보유 종목 8열 · 자산배분 4열이 한 줄이에요.

- 조립: `AppShell`(StockList · Page · ChatPanel) → `Page`(제목 “현황”, 동작: 성적표·설정·채팅 열기) → `Section` “현황”(`Grid`: 총 평가 · 통화별 평가 · 오늘 손익 · 보유 종목 · 자산배분) → `Section` “오늘”(SignalCard · DisclosureCard · BriefingCard, 최근 것부터).
- 위쪽 “화면 상태” 전환은 미리보기 전용이에요. 정상 · 로딩(Skeleton) · 빈(EmptyState) · 오류(StatusBar 한 줄 + 실패 Callout).
