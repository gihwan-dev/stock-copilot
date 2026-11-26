# ADR 0195: AI 시장 도구를 토스 읽기에 연결하고, 도구 오류를 원인대로 알린다

- 상태: 확정 (2026-11-27). v0.5.13 실설치에서 확인한 실패를 고친다. [ADR 0193](0193-daily-candle-time-and-reported-cash-anchor.md)을 잇는다.

## 관찰

| 도구 | v0.5.13 결과 | 원인 |
|---|---|---|
| `get_market_overview`, `get_rankings`, `get_sectors`, `get_investor_flow` | 항상 `upstream_unavailable` | 앱 조립에서 네 도구가 늘 실패하는 빈 구현(`UnconnectedToolData`)으로 연결돼 있었다. 토스 어댑터의 읽기는 이미 있었다 |
| `get_candles` (000660, KR) | `upstream_unavailable` | 엄격한 일봉 경로(`BrokerRead::candles`)는 일봉마다 그 날의 장 일정이 있어야 한다. 실제 앱의 토스 읽기에는 장 일정이 들어가지 않아 창 안에 일봉이 하나라도 있으면 `Unsupported(DailyCandles)`가 난다. 오류를 버리고 있어서 진단 기록에도 남지 않았다 |
| 모든 시장 도구 | 실패가 모두 재시도 가능으로 보임 | 저장소 오류 하나로 바꿔서 원인을 잃었다 |

## 결정

1. **실제 읽기 연결.** `BrokerMarketTools`가 시세·일봉 원천, `MarketInfo` 원천, 종목 디렉터리, 시계를 받는다.
   - 시장 개요: KOSPI·KOSDAQ·국고채 3년·10년(지표 시세), USD/KRW(환율 중간값), KR·US 장 상태(장 일정). S&P 500·나스닥 100·금·원유·달러는 미국 ETF(SPY·QQQ·GLD·USO·UUP) 시세로 근사하고 `approximate`·`usd_price`로 표시한다. 미국 국채 금리는 원천이 없어 뺀다. 일부가 실패하면 그 값만 빼고, 모두 실패하면 첫 원인으로 실패한다.
   - 순위: 거래대금·상승·하락은 당일 순위(최대 100). 외국인·기관 순매수는 투자자별 순위의 순매수 목록이며 국내만 된다. 이름은 종목 디렉터리에서 찾고 없으면 종목 코드를 쓴다. 가격은 비운다.
   - 업종: 당일 상승률 업종 순위와 거래대금.
   - 투자자 동향: 최근 45일 종목 투자자별 매매. 토스가 주는 값은 주식 수이므로 원화 순매수는 그 날 종가 × 순매수 주식 수로 추정한다(`FlowItem::net_krw_at`, `domain`). 종가가 없으면 비운다. 공매도 비율은 비운다.
2. **일봉 경로.** `get_candles`는 과거 평가와 같은 관대한 일봉 경로(`history_daily_candles`)를 쓴다. 장 일정 없이 확정된 날만 돌려주고 가격은 수정주가가 아니다. 도구 설명에 그렇게 적는다. 요청 창은 `일수 × 2 + 14일`로 넓혀 연휴에도 요청한 거래일 수를 채운다.
3. **오류 종류.** 시장 도구는 `MarketToolError`를 돌려준다.

   | 원인 | 도구 오류 | 재시도 |
   |---|---|---|
   | 계좌 미연결(원천 없음) | `data_unavailable` "연결되지 않음" | 아니오 |
   | 기능 없음(`Unsupported`, 해외 투자자 동향 등) | `data_unavailable` | 아니오 |
   | 종목 없음, 요청 거절·접근 거부 | `data_unavailable` | 아니오 |
   | 응답 해석 실패 | `internal_error` | 아니오 |
   | 요청 한도 | `rate_limited` | 예 |
   | 네트워크·5xx·점검·토큰 만료·종목 마스터 준비 중 | `upstream_unavailable` | 예 |

   시험용 고정 구현이 주는 저장소 오류는 기존대로 `upstream_unavailable`이다. 늘 실패하던 `UnconnectedToolData`는 지운다.
4. **진단 기록.** 도구 읽기가 실패하면 `diagnostics.log`에 한 줄을 남긴다. `tool-read tool=<도구> result=failed endpoint=<끝점> status=<HTTP 분류> kind=<오류 종류>` 또는 `result=not-connected`. 열거형 값만 쓰고 종목·가격·수량·계좌·토큰은 쓰지 않는다.

## 다른 선택지

- 엄격한 일봉 경로에 장 일정을 채우기: 장 일정 API를 날짜마다 불러야 하고 화면 차트·추정 판정 경로까지 바뀐다. 이번 범위를 넘는다.
- 투자자 동향을 주식 수로 보내기: 도구 보기(`InvestorFlowToolLine`)가 금액 형식이라 기존 시험을 바꿔야 한다. 추정임을 설명에 밝히고 금액으로 보낸다.

## 남은 일

- 화면 종목 상세의 차트(`ipc::instruments`)와 추정 판정 자료(`platform::resolution`)도 엄격한 일봉 경로를 쓰므로 같은 이유로 실제 앱에서 비어 있거나 실패할 수 있다. 장 일정 공급 또는 관대한 경로 전환을 따로 정한다.
- 엄격한 경로의 `candles:parse:1`은 장중에 14:00 KST로 찍힌 당일 봉을 미래로 보고 거절한다. 장 일정을 공급하게 되면 함께 고친다.
