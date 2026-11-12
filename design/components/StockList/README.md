# StockList

왼쪽 사이드바예요. 화면 전환 메뉴가 아니라 **보유·관심 종목 목록**이고, 누르면 그 종목 상세가 본문에 보여요.

- 제공: `held`, `watch`(StockListItem props 배열), `selectedId`, `onSelect(item)`, `loading`.
- AppShell 안에서 창 폭에 따라 모양이 바뀌어요: 1080px 이상은 전체(`sidebar-width`), 미만은 레일(`sidebar-collapsed`: 두 글자 + ▲▼). 레일의 펼치기 버튼을 누르면 전체 목록이 본문 위로 펼쳐지고, 종목을 고르거나 Esc를 누르면 닫혀요.
- 비어 있으면 “아직 종목이 없어요”를 보여줘요.
