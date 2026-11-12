# StockListItem

종목 목록의 한 줄이에요. 이름·코드와 오늘 등락률을 보여줘요.

- 제공: `name`, `ticker`, `rate`(오늘 등락률 문자열), `selected`(`aria-current`), `onSelect`, `compact`(접힌 레일용: 이름 앞 두 글자와 ▲▼, 툴팁에 전체).
- 높이 `list-item-height`. 보통은 StockList가 알아서 그려요.
