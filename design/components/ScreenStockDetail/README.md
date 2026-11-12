# 종목 상세

종목 목록에서 하나를 고르면 본문 자리에 보여요. 화면을 바꾸는 게 아니라 같은 Page 자리에 `leading` 뒤로 가기(“현황으로”)와 함께 열려요.

- 조립: `Page`(종목명, 뒤로 가기, 더 보기 Menu) → 현재가 줄(`Money` display + `PriceChange` lg + 기준 시각) → `Grid`(시세 Card: `CandlestickChart` 8열 · 내 보유 Card 4열) → `Section` “이 종목 소식”(SignalCard · DisclosureCard).
- 상태: 로딩(Skeleton) · 빈(보유하지 않은 관심 종목, 시세 없음) · 오류(StatusBar + 시세 실패 Callout, 현재가는 `—`) · 정상.
- 정정·취소 메뉴는 없어요(토스증권 앱에서 해요).
