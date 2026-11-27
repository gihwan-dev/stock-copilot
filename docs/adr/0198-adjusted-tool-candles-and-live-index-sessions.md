# ADR 0198: AI 일봉 도구는 수정주가를 주고, 지수 일봉은 실제 장 일정으로 판정한다

- 상태: 확정 (2026-11-27). [ADR 0196](0196-live-market-tools-and-truthful-tool-errors.md)의 "남은 일" 두 가지와 시장 개요의 미국 국채 금리 표시를 정한다.

## 결정

1. **`get_candles`는 수정주가 경로를 쓴다.** 도구 일봉은 추정 판정(`platform::resolution`)·신호(`signal_flow`)가 쓰는 엄격한 수정주가 일봉(`BrokerRead::candles` + `PriceAdjustment::Adjusted`)을 그대로 쓴다. 이 경로는 종목 마스터의 가격 제한 정보로 이웃한 두 봉의 움직임을 검사하므로(±30%), 분할 전후가 섞인 원시 가격은 통과하지 못한다.
   - 수정주가 경로가 `Unsupported(AdjustedHistory)`(마스터에 종목이 없거나 가격 제한을 알 수 없는 날) 또는 응답 검증 실패로 끝나면 관대한 과거 경로(`history_daily_candles`, 원시 가격)로 한 번 더 읽는다. 네트워크·한도·접근 거부 같은 다른 실패는 그대로 도구 오류로 알린다.
   - 응답에 `adjusted`(참/거짓)를 넣는다. 수정주가 경로로 읽었으면 참, 원시 가격으로 대신 읽었으면 거짓이다. 도구 설명도 그렇게 바꾼다.
   - 앱은 시장과 심볼에 대응하는 마스터 종목 식별자를 전달한다. 다음 페이지를 읽을 때 가격 기준이 바뀌면 커서를 거부해 같은 시계열에 원시 가격과 수정주가가 섞이지 않게 한다.
   - 확정된 봉(`Finalized`)만 돌려준다. 장중의 당일 봉은 빠진다.
   - 투자자 동향의 원화 추정(순매수 주식 수 × 그 날 종가)은 주식 수와 맞도록 원시 가격 일봉을 계속 쓴다.
   - 응용 계층은 `MarketToolReads::priced_candles`(기본 구현은 `Unadjusted`)와 `CandlePriceBasis`, `PricedCandlesView`를 더한다. 기존 `candles`와 `CandlesToolView`는 그대로다.
2. **지수 일봉도 실제 장 일정을 쓴다.** `with_live_market_sessions()`로 만든 읽기에서 `market_indicator_candles`는 주식 일봉과 같은 장 일정 기억(`known_sessions`/`remember_sessions`)을 쓴다.
   - 하루가 지난 봉(봉 날짜 자정 + 1일 ≤ 수집 시각)은 장 일정 없이 확정이다(`domain::indicator_day_age`, `settle_indicator_day`).
   - 당일 봉은 그 날 KR 장 일정을 받아 정규장 마감 전이면 `InProgress`, 마감 뒤면 `Finalized`다.
   - 빈 날 검사는 주말을 휴장으로 보고, 평일 빈 날만 그 날 장 일정을 받아 휴장인지 확인한다. 받은 일정(전·당·다음 영업일과 그 사이 휴일)은 프로세스 수명 동안 기억하므로 같은 날을 다시 부르지 않는다.
   - 고정 장 일정(`with_candle_sessions`)을 쓰는 읽기는 예전과 같다. 일봉 상태는 `Unknown`이고 빈 날마다 고정 일정이 있어야 한다.
3. **미국 국채 금리는 "제공 안 함"으로 적는다.** 원천이 없으므로 값을 만들지 않는다. `get_market_overview` 응답에 `not_provided` 목록을 두고 `us_treasury_2y`, `us_treasury_10y`를 `reason: "no_source"`로 적는다. 포트가 그 값을 실제로 주면 목록에서 빠진다.

## 다른 선택지

- 수정주가 경로가 실패하면 도구도 실패시키기: 마스터가 아직 없는 첫 실행이나 해외 종목 일부에서 일봉을 전혀 못 준다. 원시 가격이라도 주고 `adjusted: false`로 밝히는 편이 AI에게 낫다.
- 지수 일봉 창의 모든 날 장 일정 받기: 한도(초당 3회) 때문에 느리다. 주말을 빼고 평일 빈 날과 당일만 받는다.
- 미국 국채 금리를 ETF로 근사: 금리와 ETF 가격은 단위와 움직임이 달라 근사로도 쓸 수 없다.

## 시험

- `broker-toss/tests/live_market_candles.rs`: 분할 전후 수정주가 일봉이 수정 가격으로 확정되고(`adjusted=true` 요청, 장 일정 호출 없음), 분할 점프가 남은 응답은 수정주가 경로에서 거절된다. 지수 일봉은 평일 휴장을 장 일정으로 확인하고, 당일 봉을 장중 `InProgress`·마감 뒤 `Finalized`로 표시하며, 두 번째 읽기에서 장 일정을 다시 부르지 않는다. 열린 평일이 빠지면 거절한다.
- `domain/src/indicator_day/tests.rs`: 나이·당일 장 일정·분봉 판정.
- `src-tauri` `mcp_market` 시험: 수정주가 경로 사용과 표시, 수정주가 없음 → 원시 가격 대체, 그 밖의 실패는 재시도 없이 보고.
- `mcp-server/tests/market_tool_basis.rs`: `adjusted` 표시와 `not_provided` 목록.
