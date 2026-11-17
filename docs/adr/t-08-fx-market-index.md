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

프로젝트 설정의 hooks=true와 Git 훅 설치는 확인했으나 현재 세션의 프로젝트 신뢰·훅 신뢰·실제 적용은 확인할 수 없다. 실제 API와 실제 자격 증명은 사용하지 않았다. portable-r1 단독 실행 첫 시도에는 지정된 pnpm PATH를 빠뜨려 Corepack 경로 부재로 실패했다. 지정된 PATH로 다시 실행한 10개 시험은 통과했다.

## 보호 파일

- harness/arch-check/rules.toml: 합성 녹화의 기존 재생 도우미를 사용하는 tests/fx_market.rs 한 파일만 HTTP 시험 허용 목록에 추가한다. 제품 전송 경로는 늘리지 않는다.

## D-50 정규장과 연장 거래

국내 calendar의 preMarket·regularMarket·afterMarket은 각각 MarketState::PreMarket·RegularMarket·AfterMarket으로 보존하고, 미국 calendar의 dayMarket·preMarket·regularMarket·afterMarket도 구별해 보존한다. 어느 연장 세션도 정규장과 합치지 않는다. 합성 국내 응답에서 정규장 종료는 15:30 KST이고 16:00은 애프터마켓이며, 애프터마켓 종료 20:00과 서로 다르다. 미국의 정규장과 다음 날 애프터마켓도 독립 구간이다. 날짜·시각·단일가 경계를 유지하고 calendar 결과·하루·세션 객체에서 명세에 없는 필드는 세션 의미를 확정할 수 없어 응답 오류로 닫고 실측 대기로 둔다. 이 보수적 정책은 무해한 추가 메타데이터도 거부할 수 있다.

D-50: 일별 종가와 평가 가격 증거에서 사용할 장 종료를 정규장 종료로 정하고 소비자가 연장 거래 종료를 혼동하지 않도록 domain 판정을 수정한다. T-08의 이 매핑 및 테스트는 해당 domain 판정이 이미 수정되었다는 뜻이 아니다. 실제 토스 응답의 세션 형식은 T-13 실측 대상이다.

T-07의 일봉 어댑터 `crates/adapters/broker-toss/src/market_read/candles.rs`의 `assemble`도 현재 모든 세션의 가장 늦은 종료를 일봉 확정에 사용한다. D-50: domain의 정규장 종료 판정뿐 아니라 이 소비 경로도 같은 기준으로 맞추고 16:00 KST 회귀를 확인한다. T-08 범위에서 기존 일봉 구현은 변경하지 않았다.

미국 지수 대용 ETF의 양성 검증은 `us_index_proxy_etf_reuses_existing_quote_endpoint` 안의 SPY·USD 인라인 합성 응답을 쓴다. 위 표의 기존 `t07-quote.json`은 같은 `/prices` 경로의 국내 종목 합성 녹화이며 ETF 녹화라고 주장하지 않는다. 401·429의 전송 계약은 T-07 기존 현재가 테스트를 재사용한다.

## 지표 봉과 지원 기능 보강

확정 ADR 0055·0082가 T-08에 배정한 `MarketInfo::indicator_candles`의 기존 미지원 상태를 보완한다. 기존 `ReadRoute::IndicatorCandles`, 토큰 관리자와 제한기를 사용해 `/api/v1/market-indicators/{symbol}/candles`를 읽고 OHLCV를 `MarketIndicatorCandle` 생성자에 넣는다. 지수 분봉과 지표 일봉만 허용하며 미지 심볼과 국채 분봉은 전송 전에 거부한다. 최대 200행·64쪽이고 포함 `nextBefore` 페이지의 같은 봉은 값과 원래 오프셋이 모두 같을 때만 한 번 보존한다. 서로 다른 값·오프셋, 역순, 지나치게 큰 페이지와 진행하지 않는 커서는 실패한다. 분봉 시간 간격과 국내 일봉 사이의 휴장 증거를 확인하며 근거가 없는 중간 날짜는 성공으로 꾸미지 않는다.

일봉의 확정 상태는 domain 생성자의 `Unknown`을 그대로 반환한다. 해당 날짜의 실제 `MarketCalendar`를 가진 소비자만 `finalize_index_day`에 넘겨 정규장 종료를 확인한다. 합성 16:00 KST 사례에서 정규장 15:30 종료와 20:00 애프터마켓을 분리한 달력으로 확정되는 것을 시험했다. 이 검증은 앞서 인계한 T-07 종목 일봉의 최대 종료 시각 문제를 고쳤다는 뜻이 아니다. 과거 날짜의 전체 달력 공급과 실제 봉 시각은 T-13/D-50에서 확인한다.

| 엔드포인트 | 포트 메서드 | 합성 녹화 |
|---|---|---|
| GET /api/v1/market-indicators/{symbol}/candles | MarketInfo::indicator_candles | tests/market_recordings/t08-indicator-daily.json, t08-indicator-minute.json |

공통 `TossAccountRead::capabilities`는 이 묶음에서 실제 구현한 지표·달력·섹터·랭킹·수급 기능을 Supported로 명시한다. 미구현 종목 마스터, 호가·가격 제한 및 주문 관련 기능은 계속 Unsupported다. 지원 가능성 표시는 응답의 신선도·마스터 주입·휴장 근거가 이미 갖춰졌다는 보증이 아니다.

토스 문서는 국내 `integrated` 구간이 장전·장후 시간외종가를 제외한다고 명시한다. 따라서 그 구간은 이 응답에 없어 정규장이나 다른 연장 구간으로 만들지 않는다. D-50 정의 대기: 다른 원본에서 시간외종가 구간을 받게 되면 별도 종류와 일별 종가 기준을 어떻게 결합할지 정한다. T-13은 실제 응답에 이 문서와 다른 세션 필드가 있는지 실측한다.

T-07 인계: 기존 종목 봉 `market_read/candles.rs`는 응답의 `nextBefore` 키 누락과 명시적 null을 모두 마지막 쪽으로 취급한다. 합성 범위를 3월 23~26일로 넓히고 24~25일 두 행을 남긴 채 키를 제거하면 조회가 성공하는 반례를 별도 복사본에서 재현했다(`target/T-10/t07-missing-cursor-probe.log`). 기존 T-07 소스와 테스트는 수정하지 않았다. T-08 지표 봉은 키 누락을 닫힌 실패로 둔다.

지표 봉의 `nextBefore`는 명시적 null만 마지막 쪽으로 처리하고 키 자체가 없으면 완전성을 알 수 없어 응답 오류로 닫는다. OpenAPI 스키마는 해당 키를 필수로 지정하지 않으므로 실제 마지막 쪽에서 키를 생략하는지 T-13 실측이 필요하며, 생략이 정상이라면 현재 보수적 구현은 그 응답을 거부한다.

1차 리뷰 수정: 지수 분봉이 서로 1분 차이가 아니어도 빠진 구간이 실제로 닫혀 있다는 달력 증거가 있으면 반환한다. 날짜별 국내 세션 증거가 없거나 중복되면 각각 미지원 또는 응답 오류다. 날짜를 넘기는 세션을 놓치지 않도록 앞 봉 날짜의 전날부터 뒤 봉 날짜까지 확인한다. 휴장·거래 공백·이전 날짜의 자정 통과 세션과 중복 달력을 합성 시험했다. 역사 달력 공급과 실제 지수 분봉의 공백 정책은 T-13에서 실측한다.

2차 리뷰 수정: 빈 구간이 정규장과 겹치면 응답 검증 오류다. 연장 세션과만 겹치면 그 세션에서 지수 봉을 제공하는지 실측되지 않았으므로 `Unsupported(IndicatorCandles)`로 닫는다. 연장 세션과 정규장에 모두 걸치면 정규장 공백을 검증 오류로 보존한다. 연장 세션을 정규장에 합치지 않으며 토스 형태 합성 달력으로 두 경우를 회귀 시험한다.

T-13/A-14/O-01 인계: 조회 결과는 실패한 쪽을 버리지 않지만 벡터에 요청 창 양끝의 거래 또는 봉 존재에 대한 완전성 증거를 담지는 않는다. 합성 반례에서 명시적 마지막 커서와 빈 결과·한 행·양끝이 비어 있는 결과도 성공할 수 있었다. 현재 문서만으로 이를 누락이라고 확정할 수 없다. 정규장에 봉이 없는 정상 구간, 단일가 구간, 현재 진행 중인 봉 및 창 가장자리 포괄을 실측·정의하기 전에는 소비자가 벡터 성공만으로 창 전체의 봉 존재를 가정하지 않는다.

## 손 변형 실제 실행

`mktemp -d` 복사본에서 다음 변형을 각각 컴파일하고 표시된 테스트로 실행했다. 24개 모두 해당 테스트 실패로 검출됐다. 로그는 작업 사본의 `target/T-08/mutant-*.log`와 `target/T-08/indicator-mutant-*.log`다.

| 번호 | 바꾼 것 | 잡은 테스트 |
|---|---|---|
| 1 | 매수 환율에 중간 환율 값 사용 | `exchange_rate_preserves_kind_value_pair_validity_and_query` |
| 2 | 중간 환율에 매수 환율 값 사용 | `exchange_rate_preserves_kind_value_pair_validity_and_query` |
| 3 | 요청 기준 통화를 상대 통화로 변경 | `exchange_rate_preserves_kind_value_pair_validity_and_query` |
| 4 | 요청 상대 통화를 기준 통화로 변경 | `exchange_rate_preserves_kind_value_pair_validity_and_query` |
| 5 | 기준 통화 일치 판정 반전 | `exchange_rate_preserves_kind_value_pair_validity_and_query` |
| 6 | 환율 유효 기간 판정 반전 | `exchange_rate_preserves_kind_value_pair_validity_and_query` |
| 7 | 달력 KST 오프셋 판정 반전 | `calendar_keeps_regular_close_separate_from_extended_sessions` |
| 8 | 정규장에 애프터마켓 구간 대입 | `calendar_keeps_regular_close_separate_from_extended_sessions` |
| 9 | 미확인 세션 필드 검사 제거 | `calendar_rejects_unknown_session_kinds` |
| 10 | KOSPI 요청을 KOSDAQ로 변경 | `indicator_quotes_keep_requested_codes_and_distinct_points` |
| 11 | 지수 응답 건수 일치 판정 반전 | `indicator_quotes_keep_requested_codes_and_distinct_points` |
| 12 | 지표 시가를 종가로 변경 | `indicator_daily_candles_preserve_points_time_and_unknown_finality` |
| 13 | 지표 고가를 저가로 변경 | `indicator_daily_candles_preserve_points_time_and_unknown_finality` |
| 14 | 200봉 경계 거부 | `indicator_candle_page_accepts_200_rows` |
| 15 | 64쪽 상한을 63쪽으로 축소 | `indicator_candles_accept_complete_64_page_window` |
| 16 | 중복 봉의 원본 오프셋 비교 반전 | `indicator_candles_use_inclusive_cursor_and_deduplicate_equal_overlap` |
| 17 | 중복 봉 값 일치 판정 반전 | `indicator_candles_use_inclusive_cursor_and_deduplicate_equal_overlap` |
| 18 | 조회 시작 경계 비교 반전 | `indicator_daily_candles_preserve_points_time_and_unknown_finality` |
| 19 | 일봉 요청을 분봉으로 변경 | `indicator_daily_candles_preserve_points_time_and_unknown_finality` |
| 20 | 분봉 간격 검증 반전 | `indicator_minute_candles_preserve_finality_boundary` |
| 21 | 휴장 증거 판정 반전 | `indicator_daily_gaps_require_closed_calendar_evidence` |
| 22 | 구현된 지표 현재가 지원 표시 제거 | `capabilities_match_implemented_market_endpoints` |
| 23 | 미구현 종목 마스터를 지원으로 표시 | `capabilities_match_implemented_market_endpoints` |
| 24 | 다음 페이지 커서 키 누락 검사 제거 | `indicator_candles_reject_missing_next_before_instead_of_assuming_last_page` |

1차 리뷰 수정의 다음 여섯 변형도 별도 복사본에서 각각 컴파일하고 실행해 검출했다. 로그는 `target/T-08/review1-mutant-*.log`다.

| 번호 | 바꾼 것 | 잡은 테스트 |
|---|---|---|
| 25 | 전날 세션 대조 생략 | `indicator_minute_gap_checks_previous_days_cross_midnight_session` |
| 26 | 마지막 날짜 세션 대조 전에 성공 반환 | `indicator_minute_gap_checks_the_later_days_pre_market` |
| 27 | 마지막 날짜 검사 후 하루 더 진행 | `indicator_minute_gap_at_last_date_stops_after_checking_coverage` |
| 28 | 세션과 빈 구간의 겹침 검사 반전 | `indicator_minute_gap_rejects_missing_session_evidence_and_trading_overlap` |
| 29 | 날짜별 세션 중복 검사 제거 | `indicator_minute_gap_rejects_duplicate_calendar_days` |
| 30 | 없는 달력을 서버 응답 오류로 분류 | `indicator_minute_gap_requires_every_intervening_day` |

2차 리뷰 수정의 다음 여덟 변형은 별도 복사본에서 각각 컴파일하고 실행해 검출했다. 첫 두 시도는 타입 추론 오류로 컴파일되지 않아 세지 않았고, 해당 분기를 컴파일 가능한 다른 형태로 바꿔 다시 실행했다. 로그는 `target/T-08/review2-mutant-*.log`, 최초 미컴파일 결과는 `target/T-08/review2-mutants-initial.json`이다.

| 번호 | 바꾼 것 | 잡은 테스트 |
|---|---|---|
| 31 | 연장 세션만 겹칠 때 응답 오류로 분류 | `indicator_minute_gap_distinguishes_extended_and_regular_sessions` |
| 32 | 정규장 공백을 미지원으로 분류 | `indicator_minute_gap_rejects_missing_minute_during_regular_session` |
| 33 | 연장 세션을 발견하자마자 반환해 뒤 정규장을 확인하지 않음 | `indicator_minute_gap_distinguishes_extended_and_regular_sessions` |
| 34 | 프리마켓 구간을 무시 | `indicator_minute_gap_checks_the_later_days_pre_market` |
| 35 | 애프터마켓 구간을 무시 | `indicator_minute_gap_rejects_missing_session_evidence_and_trading_overlap` |
| 36 | 뒤 세션 시작 경계를 닫힌 구간으로 변경 | `indicator_minute_gap_accepts_only_calendar_proven_closed_interval` |
| 37 | 앞 세션 종료 경계를 닫힌 구간으로 변경 | `indicator_minute_gap_accepts_only_calendar_proven_closed_interval` |
| 38 | 날짜가 바뀔 때 연장 세션 증거를 버림 | `indicator_minute_gap_rejects_missing_session_evidence_and_trading_overlap` |
