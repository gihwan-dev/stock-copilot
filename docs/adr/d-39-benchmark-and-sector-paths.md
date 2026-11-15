# D-39·D-40·D-41 벤치마크와 섹터 조회 경로

- 상태: 초안

## 결정

### D-39 지수 일봉

리뷰 회차 2에서 종목 Candle 경로는 Sp500Etf의 조정된 USD 일봉에만 남긴다. BenchmarkSeries::from_candles와 이를 사용하는 SignalBenchmark::from_candles는 Kospi/Kosdaq를 InvalidBenchmark/BenchmarkMismatch로 거부한다. 국내 신호에는 확정 MarketIndicatorCandle을 받는 SignalBenchmark::from_index_candles를 사용한다. D-29 시나리오와 기존 국내 비교 테스트는 단언을 유지한 채 이 경로로 옮긴다.

finalize_index_day는 D-10·D-31의 봉인된 MarketCalendar에서 today의 국내 정규장 종료를 찾는다. 지수 일봉의 timestamp가 그 거래일의 KST 날짜에 속하고 종료가 수집 시각보다 엄격히 앞서야 한다. 일봉 timestamp가 정규장 시작 이후이거나 마감 시각이어도 같은 거래일이면 허용한다. 장전·시간외 종료를 확정 근거로 쓰지 않는다. KST 자정과 자정 직전/직후를 독립 경계 입력으로 검사해 UTC 날짜나 ±1초 오프셋으로 거래일이 바뀌지 않게 고정한다. 단, MarketCalendar도 공개 생성자로 만들 수 있으므로 확정은 호출자가 넘긴 달력의 세션에 근거한다. 실제 일봉의 시각 표기와 확정 근거는 T-08 실측이다. 어댑터의 거짓 진술까지 인증하지 않는다.

index_benchmark_series는 KOSPI/KOSDAQ의 Finalized 일봉만 받아 가격일을 KRW 기준으로 맞춘다. 지수 포인트를 Money·Price로 바꾸지 않는다. 동일 지수·유일한 KST 날짜·전체 스냅샷 가격일 대응을 요구하며 진행 중/미확정, 분봉, 금리, 혼합 지수, 누락과 중복 날짜를 거부한다. 입력 4097개 상한과 최소 두 스냅샷은 D-09와 같다. Native USD는 거부한다.

정렬·결제일·수익률 계산은 BenchmarkSeries의 crate 내부 생성자 하나로 합쳤다. kind/currency/dates는 다시 비공개다. ETF의 환산 입력만 Money를 거쳐 기존 환산식을 적용한다. 반환 관측 날짜는 가격일이 아닌 결제일을 보존한다. 한국만 휴장/미국만 휴장인 혼합 달력 반례로 가격일 통화와 휴장 이월을 구별한다.

SignalBenchmark는 두 입력 경로 뒤의 지점·구간 봉인을 공유한다. zip 앞에서 관측 수+1과 지점 수를 검사하고 불일치면 BenchmarkMismatch다. 정규장 종가 지점을 사용하며 휴장으로 같은 종가가 이월되는 지점을 보존한다.

### D-40 섹터 선택

리뷰 회차 2의 메인 결정에 따라 사용자에게 보이는 섹터는 대분류(SectorCatalog의 Major, 스펙 depth 0)다. 구성 조회와 마스터의 시장·심볼이 일치하고, 목록에 대분류로 등록된 구성만 후보로 받는다. 중분류·소분류를 후보로 삼거나 그 구성만 보고 부모 대분류의 소속을 추정하지 않는다. 목록에 없는 후보 섹터나 같은 대분류의 중복 조회는 닫힌 오류다.

한 대분류에 속하면 그 섹터다. 여러 대분류에 속하면 해당 종목의 구성 내 비중이 아니라 구성 전체 시가총액이 큰 대분류를 고른다. 같으면 SectorId의 기존 Ord 순으로 작은 식별자를 고른다. 구성 비중 계산과 exact_arithmetic에 더했던 교차 곱 비교는 제거했다. 여러 대분류 표지는 선택 뒤와 SectorAssignments 수집 뒤에도 보존한다.

후보 중 하나라도 구성원 시가총액이 미제공이면 모든 후보를 식별자 순으로 정하고 CapitalizationUnavailable 이유를 남긴다. 합계를 Money로 정확히 표현할 수 없는 경우도 전체 시가총액을 알 수 없는 것으로 보존한다. 단일 소속은 동료 시가총액이 없어도 분류하며 같은 이유 표지를 남긴다. 알려진 0 합계는 정상 값이며 비중으로 나누지 않는다.

마스터의 ETF·FOREIGN_ETF·INFRASTRUCTURE_FUND는 구성 조회 없이 EtfFund다. 일반 종목의 대분류 소속이 없으면 NoMajorMembership 이유가 있는 Unclassified다. 마스터 종목 유형이 알려지지 않으면 UnknownSecurityType 이유의 Unclassified다. ETF를 구성 주식으로 분해하지 않는다. 공개 생성 경로는 sector_assignment_from_compositions이며 기존 생성자는 crate 내부에 남긴다.

공개 SectorMembership은 Single/Multiple뿐이다. ETF·미분류와 내부 레거시 생성 결과는 Option의 None으로 나타낸다. 새 공개 경로의 이유는 항상 보존하고, 내부 레거시만 이유가 없다. Unspecified 공개 변형은 제거했다. 이 타입들은 공급자가 넘긴 목록·구성·마스터 간 형식과 대응을 검사할 뿐 어댑터의 거짓 진술이나 조회 집합의 완전성까지 인증하지 않는다.

| 예 | 구성 조회 | 결과 | 보존 표지 / 자산배분 확인 |
|---|---|---|---|
| 삼성·하이닉스가 서로 다른 소분류 | 대분류 A에 둘 다 포함, 소분류 x에 삼성·y에 하이닉스 | 둘 다 A | 각각 자산 20%와 현금 60%에서 A 40%가 합산돼 상한 30%를 넘는다 |
| 여러 대분류, 구성 내 비중과 전체 시총이 반대 | A: 삼성 100·동료 900, B: 삼성 100·동료 100 | A | Multiple, LargestMajorCapitalization |
| 전체 시총 동률 | B: 2+8, A: 9+1 | A | Multiple, 식별자 순 |
| A 또는 B의 전체 시총 미제공 | A/B 중 한 구성원 시총 null | 식별자가 작은 A | Multiple, CapitalizationUnavailable |
| 단일 대분류의 동료 시총 null | A: 삼성 2·동료 null | A | Single, CapitalizationUnavailable |
| 마스터가 ETF·펀드 | 구성 없음 | EtfFund | EtfFundFromMaster |
| 일반 종목의 대분류 조회 없음 | 중·소분류만 있거나 소속 없음 | Unclassified | NoMajorMembership |

인계: D-06·D-07 계열과 A-08은 이 대응표를 자산배분·섹터 상한에 사용한다. A-20·T-09·T-10은 대분류 목록과 전체 후보 구성, 마스터 유형을 연결하고 실제 응답의 완전성과 시가총액 제공 여부를 확인한다.

### D-41 추가 조회 사실

리뷰 회차 2에서 QueryValue<T>를 제거했다. nullable 금액은 D-37의 SectorAmount, 등락률은 RankingChange를 그대로 쓰고 섹터 부모·요약은 Option이다. 기간·랭킹 종류·투자자 구분도 기존 RankingDuration·RankingKind·InvestorClass를 재사용한다. 섹터 조회는 여섯 기간과 TopGainers/MarketTradingAmount만 생성자에서 허용한다. 정수 원화 금액, 그룹별 독립 집계 시각, 순위 빈자리와 서버 순서는 보존한다.

순위 연속성·양수/중복 검증과 nullable 등락률 파서는 market_ranking의 공통 비공개(crate 내부) 함수로 합쳤다. 금액 파서는 sector_composition의 공통 함수를 쓴다. 별도 전달 함수나 제네릭 선택 값 타입은 두지 않는다. 섹터 랭킹의 순서 검사는 null을 건너뛰고 값이 있는 것끼리 내림차순을 비교한다. null을 0으로 대체하지 않는다. 상세 응답은 여섯 기간의 정확한 순서와 자기 부모 금지를 검사한다.

scope·hierarchy·InvestorRankingScope·항상 KRW인 별도 currency·의미가 다른 FlowLevel/FlowFinality 재사용 필드는 제거했다. 관련 ETF는 /sectors/{sectorId}/etfs의 목록이며 내부 보유 자산 분해가 아니다. 하위 섹터 미포함·대분류/중분류 랭킹·KRX/NXT ETF 포함·항상 KRW인 금액 등 고정된 API 의미는 이 문서와 어댑터 계약으로 남긴다. 금액이 있으면 Money가 KRW 단위를 보존하고 입력 행의 currency 검증은 유지한다.

SectorAmount의 Debug는 금액 원문만 가린다. 섹터 식별자·이름·부모·요약과 미제공 여부는 읽을 수 있다. 새 랭킹 입력 행과 봉인된 순매수/순매도 항목도 금액 필드만 가린다. 투자자별 그룹은 각각 MarketObservation을 가지며 서버 시각이 없는 섹터 API는 collected_only로 보존한다. 공급자의 거짓 진술을 인증하거나 주식/섹터/ETF의 추가 조회 사실을 발명하지 않는다.

## 검증

실계좌·실제 API·새 의존성·자동 변이 사용 없음. 직접 전체 검사는 반복하지 않고 정상 커밋 훅의 검사를 받는다. 프로젝트 설정과 훅의 세션 신뢰 적용 여부는 별도 런타임 증거가 없으므로 Git 훅의 실제 결과와 구분한다.

## 인계

- T-08: 실제 지수 일봉의 날짜와 장 종료 증거를 조회 세션에 연결한다.
- A-09·A-15·E-08: 국내 지수 벤치마크에는 index_benchmark_series를 사용한다. 종목 거래소에 맞춘 KOSPI/KOSDAQ 선택은 ADR 0072의 후속 계약이다.

## 초기 회차 손 변형

아래는 0d46eb8 이전 코드의 실제 실행 기록이며 리뷰 수정 후의 결과와 구분한다. 수정 회차 결과는 문서 끝에 추가한다.

D-39의 최종 손 변형 결과는 아래 표에 기록한다. D-40·D-41은 해당 커밋에서 덧붙인다.

### D-39 손 변형 결과

집중 benchmark 테스트 37개(새 모듈 9개 포함)가 통과했다. 새 모듈 줄 커버리지 385/385(100%), 신호 벤치마크 줄 커버리지 345/345(100%)이며 두 파일 모두 미실행 코드 영역이 없다. 속성 테스트는 4096회다. 최대 Timestamp의 KST 범위 초과, 최소 양수 지수에서 MAX 지수로 변할 때의 산술 범위 초과도 실제 입력으로 거부한다. 첫 최소 스냅샷 변형 생존은 1개 입력 반례로 수정했다. 아래 13개는 각각 실제 컴파일·실행으로 검출했다.

| 번호 | 바꾼 것 | 검출 테스트 | 결과 |
|---|---|---|---|
| D39-01 | series.observations().count().saturating_add(1) != points.len() -> series.observations().count().saturating_add(1) == points.len() | signal_zip_rejects_both_shorter_and_longer_point_lists | 컴파일 성공·검출 |
| D39-02 | \|\| candle.status() != IndicatorCandleStatus::Finalized -> 삭제 | index_path_rejects_invalid_sources | 컴파일 성공·검출 |
| D39-03 | \|\| candle.period() != CandlePeriod::Daily -> 삭제 | index_path_rejects_invalid_sources | 컴파일 성공·검출 |
| D39-04 | candle.code() != first.code() -> candle.code() == first.code() | index_rejects_a_single_snapshot, index_range_and_arithmetic_errors_are_closed, index_path_rejects_invalid_period_and_limits, index_path_rejects_invalid_sources, index_path_preserves_points_and_rejects_unconfirmed_or_misaligned_inputs, index_returns_match_independent_integer_ratio | 컴파일 성공·검출 |
| D39-05 | IndicatorCode::Kospi => Ok(BenchmarkKind::Kospi) -> IndicatorCode::Kospi => Ok(BenchmarkKind::Kosdaq) | index_path_preserves_points_and_rejects_unconfirmed_or_misaligned_inputs | 컴파일 성공·검출 |
| D39-06 | price_return(*old, *new) -> price_return(*new, *old) | index_path_preserves_points_and_rejects_unconfirmed_or_misaligned_inputs, index_range_and_arithmetic_errors_are_closed, index_returns_match_independent_integer_ratio | 컴파일 성공·검출 |
| D39-07 | currency == PerformanceCurrency::Native(Currency::Usd) -> currency == PerformanceCurrency::Native(Currency::Krw) | index_path_rejects_invalid_sources, index_path_preserves_points_and_rejects_unconfirmed_or_misaligned_inputs | 컴파일 성공·검출 |
| D39-08 | old >= new -> old > new | index_path_rejects_invalid_period_and_limits | 컴파일 성공·검출 |
| D39-09 | candles.len() > 4097 -> candles.len() > 4098 | index_path_rejects_invalid_period_and_limits | 컴파일 성공·검출 |
| D39-10 | snapshots.len() > 4097 -> snapshots.len() > 4098 | index_path_rejects_invalid_period_and_limits | 컴파일 성공·검출 |
| D39-11 | ordered.len() < 2 -> ordered.len() < 1 | index_rejects_a_single_snapshot | 컴파일 성공·검출 |
| D39-12 | regular.session().interval().end() >= candle.observation().collected_at() -> regular.session().interval().end() > candle.observation().collected_at() | finality_rejects_equal_close_and_late_bar_start | 컴파일 성공·검출 |
| D39-13 | start > regular.session().interval().start() -> start < regular.session().interval().start() | finality_rejects_equal_close_and_late_bar_start, index_range_and_arithmetic_errors_are_closed, index_rejects_a_single_snapshot, index_path_preserves_points_and_rejects_unconfirmed_or_misaligned_inputs, index_path_rejects_invalid_sources, index_path_rejects_invalid_period_and_limits, index_returns_match_independent_integer_ratio | 컴파일 성공·검출 |

### D-40 검증과 손 변형

집중 테스트 6개 통과, 독립 정수 교차 곱 속성 대조 4096회. 새 파일 집중 줄 커버리지 244/244(100%). 외부 crate의 허용 경로는 컴파일 성공, 기존 SectorAssignment::new는 E0624로 거부된다. 기존 테스트 본문 변경과 test-diff 예외는 없다. 새 의존성과 허용 목록 항목도 없다.

| 번호 | 바꾼 것 | 검출 테스트 | 결과 |
|---|---|---|---|
| D40-01 | order == Ordering::Less -> order == Ordering::Greater | fractional_weights_and_wide_products_preserve_exact_order, largest_weight_wins_ties_use_id_and_multiple_membership_survives, selection_matches_independent_integer_cross_products | 컴파일 성공·검출 |
| D40-02 | candidate.id > previous.id -> candidate.id < previous.id | largest_weight_wins_ties_use_id_and_multiple_membership_survives | 컴파일 성공·검출 |
| D40-03 | membership = SectorMembership::Multiple -> membership = SectorMembership::Single | largest_weight_wins_ties_use_id_and_multiple_membership_survives | 컴파일 성공·검출 |
| D40-04 | let mut membership = SectorMembership::Single -> let mut membership = SectorMembership::Multiple | largest_weight_wins_ties_use_id_and_multiple_membership_survives | 컴파일 성공·검출 |
| D40-05 | if !sectors.insert -> if sectors.insert | fractional_weights_and_wide_products_preserve_exact_order, largest_weight_wins_ties_use_id_and_multiple_membership_survives, missing_weights_membership_duplicates_and_unknown_master_are_closed, selection_matches_independent_integer_cross_products | 컴파일 성공·검출 |
| D40-06 | <= rust_decimal::Decimal::ZERO -> < rust_decimal::Decimal::ZERO | missing_weights_membership_duplicates_and_unknown_master_are_closed | 컴파일 성공·검출 |
| D40-07 | member.symbol() == symbol -> member.symbol() != symbol | largest_weight_wins_ties_use_id_and_multiple_membership_survives, fractional_weights_and_wide_products_preserve_exact_order, selection_matches_independent_integer_cross_products | 컴파일 성공·검출 |
| D40-08 | 섹터 교차 곱의 십진 자리 이동 배율 10을 9로 변경 | fractional_weights_and_wide_products_preserve_exact_order | 컴파일 성공·검출 |
| D40-09 | 섹터 교차 곱의 앞자리 캐리 기여를 0으로 변경 | fractional_weights_and_wide_products_preserve_exact_order | 컴파일 성공·검출 |

초기 회차의 비중 선택과 A-09 인계는 위 D-40 리뷰 결정으로 대체한다. 수신자는 D-06·D-07 계열, A-08·A-20·T-09·T-10이다.

## H-46 실행 규약

master 8ba1f6abf3b1050017b649d1d1a5fd63104a6363의 H-46을 받았다. 이번 작업의 속성 테스트는 testkit::property_cases()를 사용한다. 기본 실행은 256회, 전체 검사 환경은 4096회이며, 앞의 4096회 및 손 변형 기록은 실제 당시 실행 결과다. domain에는 H-46이 이미 내부 testkit 개발 의존성을 추가했으므로 별도 의존성이나 패키지를 더하지 않는다. 작업 브랜치 검사는 check-fast를 사용하고 검사 계획이 요구하는 승격은 그대로 따른다.

## D-41 스펙 필드 대조

저장소의 toss-openapi-1.2.28.json을 사용한다. 네트워크 조회는 없다. required와 nullable을 별개로 대조했다. 외부 응답 래퍼 ApiResponse.result는 어댑터가 해제한다.

| 스키마.필드 | 필수 | nullable | 타입/열거형 | 도메인 보존 |
|---|---|---|---|---|
| InvestorTradingRankingResponse.foreigner | 예 | 아니오 | 객체 참조 | foreigner: InvestorRankingGroup |
| InvestorTradingRankingResponse.institution | 예 | 아니오 | 객체 참조 | institution: InvestorRankingGroup |
| InvestorTradingRankingResponse.individual | 예 | 아니오 | 객체 참조 | individual: InvestorRankingGroup |
| InvestorNetTradingRanking.rankedAt | 예 | 예 | ['string', 'null'] | 그룹별 MarketObservation.server_at, null 그대로 보존 |
| InvestorNetTradingRanking.netBuyRankings | 예 | 아니오 | array | buy: NetTradingList(NetBuy) |
| InvestorNetTradingRanking.netSellRankings | 예 | 아니오 | array | sell: NetTradingList(NetSell) |
| NetBuyRankingItem.rank | 예 | 아니오 | integer | RankingOrdinal, 양수/중복 거부, Gaps 보존 |
| NetBuyRankingItem.symbol | 예 | 아니오 | string | Symbol, 시장별 문법 검증 |
| NetBuyRankingItem.currency | 예 | 아니오 | #/components/schemas/Currency | KRW 입력 검증, Money의 통화로 보존 |
| NetBuyRankingItem.netBuyAmount | 예 | 아니오 | string | Money(KRW), 양의 정수, 매수 방향 별도 |
| NetSellRankingItem.rank | 예 | 아니오 | integer | RankingOrdinal, 양수/중복 거부, Gaps 보존 |
| NetSellRankingItem.symbol | 예 | 아니오 | string | Symbol, 시장별 문법 검증 |
| NetSellRankingItem.currency | 예 | 아니오 | #/components/schemas/Currency | KRW 입력 검증, Money의 통화로 보존 |
| NetSellRankingItem.netSellAmount | 예 | 아니오 | string | Money(KRW), 양의 정수, 매도 방향 별도 |
| SectorsResponse.sectors | 예 | 아니오 | array | SectorCatalog.sectors |
| Sector.sectorId | 예 | 아니오 | string | SectorId |
| Sector.name | 예 | 아니오 | string | DisplayInstrumentName |
| Sector.depth | 예 | 아니오 | integer | SectorDepth Major/Middle/Minor |
| Sector.parentSectorId | 예 | 예 | ['string', 'null'] | Option<SectorId>, 대분류 null / 나머지 부모 필수 |
| SectorDetailResponse.sectorId | 예 | 아니오 | string | SectorId |
| SectorDetailResponse.name | 예 | 아니오 | string | DisplayInstrumentName |
| SectorDetailResponse.depth | 예 | 아니오 | integer | SectorDepth Major/Middle/Minor |
| SectorDetailResponse.parentSectorId | 예 | 예 | ['string', 'null'] | Option<SectorId>, 대분류 null / 나머지 부모 필수 |
| SectorDetailResponse.summary | 예 | 예 | ['string', 'null'] | Option<DisplayInstrumentName> |
| SectorDetailResponse.performance | 예 | 아니오 | #/components/schemas/SectorPerformance | SectorDetail.rates |
| SectorPerformance.changeRates | 예 | 아니오 | array | 정해진 순서 여섯 기간, 누락/중복/다른 순서 거부 |
| SectorChangeRate.duration | 예 | 아니오 | ['1d', '1w', '1mo', '3mo', '6mo', '1y'] | RankingDuration의 여섯 기간 검증 |
| SectorChangeRate.changeRate | 예 | 예 | ['string', 'null'] | RankingChange, 소수비율을 그대로 보존 |
| SectorEtfsResponse.etfs | 예 | 아니오 | array | SectorEtfs.etfs, 서버 순서 보존, API 의미는 하위 섹터 미포함 |
| SectorEtf.symbol | 예 | 아니오 | string | Symbol, 시장별 문법 검증 |
| SectorEtf.name | 예 | 아니오 | string | DisplayInstrumentName |
| SectorRankingResponse.rankings | 예 | 아니오 | array | SectorRanking.items, 최대 100개, Gaps 보존 |
| SectorRankingItem.rank | 예 | 아니오 | integer | RankingOrdinal, 양수/중복 거부, Gaps 보존 |
| SectorRankingItem.sectorId | 예 | 아니오 | string | SectorId |
| SectorRankingItem.name | 예 | 아니오 | string | DisplayInstrumentName |
| SectorRankingItem.changeRate | 예 | 예 | ['string', 'null'] | RankingChange, 소수비율을 그대로 보존 |
| SectorRankingItem.tradingAmount | 예 | 예 | ['string', 'null'] | SectorAmount(Money(KRW)) |
| SectorRankingItem.marketCapitalization | 예 | 예 | ['string', 'null'] | SectorAmount(Money(KRW)), 하위 섹터 미포함은 API 고정 의미 |
| SectorRankingItem.currency | 예 | 아니오 | #/components/schemas/Currency | KRW 입력 검증; 별도 상수 필드는 제거하고 Money 단위로 보존 |

요청 열거형: 투자자 랭킹 count는 RankingLimit(1..=100). 섹터 랭킹 type은 TopGainers / MarketTradingAmount, duration은 RankingDuration의 Day/Week/Month/ThreeMonths/SixMonths/Year, marketCountry는 KnownMarket이다. 소분류 제외 대상 범위는 어댑터가 섹터 목록과 대조한다. 응답이 depth를 주지 않으므로 랭킹에서 임의 계층을 발명하지 않는다.

SectorRankingItem.changeRate는 스키마상 nullable이지만 설명은 항상 값이 있다고 한다. 더 보수적으로 nullable을 보존하고 0을 발명하지 않는다. 거래대금도 nullable 그대로 보존한다. 투자자 랭킹 금액만 양의 정수로 제한한다.

ETF 구성은 해당 스펙의 /sectors/{sectorId}/etfs가 주는 관련 ETF 목록이다. ETF 내부 보유 종목이나 비중 응답은 이 스펙에 없으며 그런 분해를 주장하지 않는다. 직전 분기 시가총액 정렬은 서버 순서를 보존하고 분기 날짜나 시가총액을 만들지 않는다.

### D-41 검증과 손 변형

집중 테스트 16개 통과, 원화 정수 보존과 순위 빈자리 속성 대조 4096회. 새 파일 집중 줄 커버리지 967/967(100%). 알려진 랭킹 기준값의 역전 반례는 구현 전 테스트 실패를 확인한 뒤 두 기준 모두 거부하도록 고쳤다. nullable 기준값은 그대로 보존한다. 신규 입력 행·사실·선택 후보 Debug에 금액 원문이 없음을 확인한다. 외부 crate에서 지수 봉 확정 상태·BenchmarkSeries.kind·섹터 랭킹 필드 직접 수정은 E0616으로 거부된다.

| 번호 | 바꾼 것 | 검출 테스트 | 결과 |
|---|---|---|---|
| D41-01 | value <= rust_decimal::Decimal::ZERO -> value < rust_decimal::Decimal::ZERO | investor_invalid_values_order_counts_currency_and_identity_are_closed | 컴파일 성공·검출 |
| D41-02 | \|\| !value.fract().is_zero() -> 삭제 | investor_invalid_values_order_counts_currency_and_identity_are_closed | 컴파일 성공·검출 |
| D41-03 | row.currency != Currency::Krw -> row.currency == Currency::Krw | sector_ranking_rejects_known_basis_reversals_without_erasing_nulls, debug_redacts_amount_rows_optional_values_and_group_finality, sector_rankings_preserve_nullable_fields_krw_and_direct_scope, sector_rankings_preserve_nulls_and_reject_invalid_inputs, ranking_rejects_each_invalid_numeric_field_and_preserves_missing_amount_basis | 컴파일 성공·검출 |
| D41-04 | rows.len() > *limit.as_usize() -> rows.len() < *limit.as_usize() | investor_empty_groups_preserve_unavailable_time, investor_invalid_values_order_counts_currency_and_identity_are_closed, each_investor_group_rejects_invalid_buy_and_sell_payloads, investor_groups_keep_independent_times_directions_units_and_gaps, net_amount_round_trip_and_gaps_are_independent | 컴파일 성공·검출 |
| D41-05 | rank <= previous -> rank < previous | investor_invalid_values_order_counts_currency_and_identity_are_closed, sector_rankings_preserve_nulls_and_reject_invalid_inputs | 컴파일 성공·검출 |
| D41-06 | continuity = RankingContinuity::Gaps -> continuity = RankingContinuity::Continuous | sector_rankings_preserve_nullable_fields_krw_and_direct_scope, net_amount_round_trip_and_gaps_are_independent, investor_groups_keep_independent_times_directions_units_and_gaps | 컴파일 성공·검출 |
| D41-07 | NetTradingDirection::NetSell, row.net_sell -> NetTradingDirection::NetBuy, row.net_sell | investor_groups_keep_independent_times_directions_units_and_gaps | 컴파일 성공·검출 |
| D41-08 | trading_amount: query_krw(row.trading_amount.as_ref())? -> trading_amount: query_krw(row.market_capitalization.as_ref())? | sector_ranking_rejects_known_basis_reversals_without_erasing_nulls, ranking_rejects_each_invalid_numeric_field_and_preserves_missing_amount_basis, sector_rankings_preserve_nullable_fields_krw_and_direct_scope | 컴파일 성공·검출 |
| D41-09 | value.as_decimal() < rust_decimal::Decimal::NEGATIVE_ONE -> value.as_decimal() <= rust_decimal::Decimal::NEGATIVE_ONE | sector_metadata_and_rates_reject_invalid_hierarchy_and_periods | 컴파일 성공·검출 |
| D41-10 | input.map_or(Ok(QueryValue::Unavailable), \|raw\| -> input.map_or(Err(MarketDataError::InvalidNumber), \|raw\| | sector_rankings_preserve_nulls_and_reject_invalid_inputs, sector_detail_preserves_summary_and_six_rates, sector_ranking_rejects_known_basis_reversals_without_erasing_nulls | 컴파일 성공·검출 |
| D41-11 |             DepthRelation::Adjacent ->             match parent { SectorDepth::Major => DepthRelation::Invalid, SectorDepth::Middle \| SectorDepth::Minor => DepthRelation::Adjacent } | sector_metadata_hierarchy_summary_and_all_six_rates_are_preserved | 컴파일 성공·검출 |
| D41-12 | etfs.push(SectorEtf { -> etfs.insert(0, SectorEtf { | sector_etfs_keep_response_order_without_lookthrough_or_fabricated_time | 컴파일 성공·검출 |
| D41-13 | comparison == Some(std::cmp::Ordering::Less) -> comparison == Some(std::cmp::Ordering::Greater) | sector_ranking_rejects_known_basis_reversals_without_erasing_nulls | 컴파일 성공·검출 |

계층 변형 최초 시도는 match 비소진으로 컴파일되지 않아 제외했다. 표의 D41-11은 대분류→중분류만 거부하는 컴파일 가능한 분기를 실제 재실행한 결과다. 패턴이 맞지 않아 실행되지 않은 중간 시도도 집계하지 않았다. 자동 변이는 실행하지 않았다. 기존 테스트 본문·단언 변경, 검사 끄기, 외부 의존성 추가는 없다.

D-41 인계: T-10은 required/nullable 디코딩과 응답별 집계 시각·KRW 단위·요청 열거형을 이 타입에 연결한다. T-09는 섹터 전체 목록과 구성 조회의 완전성을 공급한다. A-09·G-07은 빈 순위와 미제공 값을 정상 값으로 대체하지 않고 조회 사실을 소비한다. 이름과 요약은 기존 DisplayInstrumentName의 256바이트 경계를 재사용하며 긴 원문을 조용히 자르지 않는다. 실제 응답의 동순위·금액 정밀도·시각 갱신은 T-10 실측에 남긴다.


## 초기 회차 H-46 재실행 (0d46eb8)

초기 회차 0d46eb8의 코드 복사본에서 새 속성 테스트를 testkit::property_cases()로 전환한 상태로 다시 실행했다. 전체 검사 환경변수 STOCK_COPILOT_PROPTEST_CASES를 제거해 기본 256회를 사용했다. 기준 테스트는 D-39 새 모듈 9개, D-40 6개, D-41 16개가 각각 통과했다. 초기 회차의 위 표는 이 재실행 결과다. D-39 13개·D-40 9개·D-41 13개, 합계 35개가 모두 실제 컴파일 후 테스트 실패로 검출됐다. 최종 집계의 생존과 컴파일 실패는 각각 0개다. 앞서 컴파일되지 않았거나 패턴이 맞지 않았던 시도는 포함하지 않는다. 자동 변이는 실행하지 않았다.

실행별 로그와 소스 해시는 작업 사본의 target/D-39/final-mutations 및 final-mutation-source-hashes.json에 보존했다. 프로젝트 보호 설정은 기존 생성 스크립트로 재생성했고, 기존 테스트 본문 변경·검사 끄기·훅 우회·외부 패키지 추가는 없다. master에서 받은 H-46의 기존 테스트 전환은 이번 작업의 직접 수정에 포함하지 않는다.

## 리뷰 회차 2 기존 테스트 이관

master c27ad60470b68e1bfc394d99a612270f798d5916 대비 test-diff는 승인된 생성 경로 변경 13건(테스트 12개·지원 함수 1개)이다. 단언과 기대 수치는 보존한다. 파일별 상세는 다음과 같다.

| 파일 | 테스트 또는 지원 함수 | 전 → 후 |
|---|---|---|
| crates/domain/src/benchmark.rs | duplicate_dates_and_missing_prices_are_rejected | BenchmarkSeries::from_candles(Kospi/Kosdaq, Candle) → 확정 지수 fixture와 index_benchmark_series |
| crates/domain/src/benchmark.rs | excess_return_and_mixed_inputs_propagate_period_errors | BenchmarkSeries::from_candles(Kospi/Kosdaq, Candle) → 확정 지수 fixture와 index_benchmark_series |
| crates/domain/src/benchmark.rs | missing_stock_history_cannot_be_used_for_mixed_weights | BenchmarkSeries::from_candles(Kospi/Kosdaq, Candle) → 확정 지수 fixture와 index_benchmark_series |
| crates/domain/src/benchmark.rs | mixed_benchmark_rejects_incomplete_account_facts | BenchmarkSeries::from_candles(Kospi/Kosdaq, Candle) → 확정 지수 fixture와 index_benchmark_series |
| crates/domain/src/benchmark.rs | price_overflow_and_expired_fx_never_form_benchmark_values | BenchmarkSeries::from_candles(Kospi/Kosdaq, Candle) → 확정 지수 fixture와 index_benchmark_series |
| crates/domain/src/benchmark.rs | public_mixed_uses_only_its_performance_equities_even_with_cash | BenchmarkSeries::from_candles(Kospi/Kosdaq, Candle) → 확정 지수 fixture와 index_benchmark_series |
| crates/domain/src/benchmark.rs | research_example_ten_matches_converted_and_mixed_benchmark_legs | BenchmarkSeries::from_candles(Kospi/Kosdaq, Candle) → 확정 지수 fixture와 index_benchmark_series |
| crates/domain/src/benchmark.rs | series_rejects_misalignment_duplicates_and_input_limits | BenchmarkSeries::from_candles(Kospi/Kosdaq, Candle) → 확정 지수 fixture와 index_benchmark_series |
| crates/domain/src/signal_benchmark.rs | d29_benchmark_rejects_missing_conflicting_and_excess_calendar_facts | SignalBenchmark::from_candles의 국내 fixture → from_index_candles; ETF 입력 유지 |
| crates/domain/src/signal_benchmark.rs | d29_benchmark_rejects_reversed_native_points_and_linked_overflow | SignalBenchmark::from_candles의 국내 fixture → from_index_candles; ETF 입력 유지 |
| crates/domain/src/signal_benchmark.rs | d29_benchmark_requires_native_currency_and_exact_close_instants | SignalBenchmark::from_candles의 국내 fixture → from_index_candles; ETF 입력 유지 |
| crates/domain/src/signal_benchmark.rs | review_daily_close_same_day_is_zero_and_missing_end_is_unavailable | SignalBenchmark::from_candles의 국내 fixture → from_index_candles; ETF 입력 유지 |
| crates/domain/src/signal_return_scenarios.rs | tests::benchmark 지원 함수 | 국내 시나리오의 Candle 벤치마크 → 확정 지수 일봉 벤치마크 |

회귀 증거: 종목 봉 위장 거부 테스트는 수정 전 InvalidPeriod가 반환되어 실패했고 수정 후 InvalidBenchmark로 통과한다. 벤치마크·국내 신호 집중 테스트와 린트 결과는 작업 사본 target/D-39/review-d39-*에 보존한다.
