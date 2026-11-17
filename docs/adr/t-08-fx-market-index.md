# T-08 환율과 시장 정보
- 상태: 초안

## 결정

기존 TossAccountRead와 TokenManager의 읽기 경로를 MarketInfo로 연결한다. 환율은 응답의 rate를 Buy, midRate를 Mid로 각각 domain FxRate에 넣는다. 두 값과 통화쌍, 시간 구간을 모두 검증하고 요청 시각이 해당 구간 밖이면 실패한다. 현재 환율은 로컬 Clock이 구간 안에 있어야 한다. 과거 시각은 UTC로 정밀도를 보존해 요청한다. 환율의 숫자 계산은 하지 않는다.

MarketInfo::calendar는 시장별 3일 응답을 MarketSession과 MarketCalendar로 변환한다. 시장 시각은 명세대로 KST여야 하고 누락된 세션은 휴장으로 추측하지 않는다. 국내 지표는 기존 읽기 전송과 IndicatorQuotes 경로를 사용하고 미국 지수 대용 ETF는 기존 BrokerRead::quotes를 사용한다. 지표 행의 요청 코드·개수·중복 및 숫자와 시각을 검증한다.

StoreFxRate는 기존 MarketDataRepository에 단일 환율을 저장한 뒤 해당 쌍의 FxRateUpdated를 발행하고 구독 실패를 DeliveryReport로 반환한다. 저장 오류면 발행하지 않는다. RefreshFxRate는 수집 결과의 쌍·종류·요청 시각을 대조한 뒤 StoreFxRate를 호출한다. 기존 시장 개요 조회와 제품 조립에는 아직 연결하지 않는다.

| 엔드포인트 | 포트 메서드 | 합성 녹화 |
|---|---|---|
| GET /api/v1/exchange-rate | MarketInfo::fx_rate, order_fx_rates | tests/recordings/exchange-rate.json |
| GET /api/v1/market-calendar/KR | MarketInfo::calendar | tests/market_recordings/t08-kr.json, t08-kr-holiday.json |
| GET /api/v1/market-calendar/US | MarketInfo::calendar | tests/market_recordings/t08-us.json, t08-us-holiday.json |
| GET /api/v1/market-indicators/prices | MarketInfo::indicator_quotes | tests/market_recordings/t08-indices.json |
| GET /api/v1/prices | BrokerRead::quotes | 기존 tests/market_recordings/t07-quote.json |

## 실측 대기

부분 휴장 세션의 null·키 누락 의미와 실제 운영 시간, 시간대·환율 유효 구간 및 응답 숫자 형식은 실제 서비스로 확인하지 않았다. 키 누락은 휴장으로 변환하지 않고 닫힌 실패로 둔다. 현재 환율의 유효 종료 시각은 배타적이다.

## 인계

- A-09/S-04: 실제 수집·캐시 조립에서 RefreshFxRate를 호출하고 저장소를 구현한다. 기존 GetMarketOverview는 직접 조회만 하며 저장·이벤트 연결을 하지 않는다. 이 작업에서 경로를 추가한 사실은 런타임 조립 완료를 뜻하지 않는다.
- T-09/T-10: MarketInfo의 나머지 메서드는 구현 전까지 명시적 UnsupportedQuery다.

## 절차

프로젝트 설정의 hooks=true와 Git 훅 설치는 확인했으나 현재 세션의 프로젝트 신뢰·훅 신뢰·실제 적용은 확인할 수 없다. 실제 API와 실제 자격 증명은 사용하지 않았다.

## 보호 파일

- harness/arch-check/rules.toml: 합성 녹화의 기존 재생 도우미를 사용하는 tests/fx_market.rs 한 파일만 HTTP 시험 허용 목록에 추가한다. 제품 전송 경로는 늘리지 않는다.
