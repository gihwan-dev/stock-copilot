# ADR 0085: D-39 벤치마크·섹터 경로와 조회 타입 (D-39·D-40·D-41)

- 상태: 확정 (2026-11-16)


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

실계좌·실제 API·새 외부 의존성 사용 없음. 직접 전체 검사는 반복하지 않고 정상 커밋 훅의 검사를 받는다. 초기 회차와 리뷰 수정 회차의 검증을 구분하며 자동 변이의 게이트 결과와 별도 실행 결과는 문서 끝에 기록한다. 프로젝트 설정과 훅의 세션 신뢰 적용 여부는 별도 런타임 증거가 없으므로 Git 훅의 실제 결과와 구분한다.

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

### 초기 회차 D-41 검증과 손 변형

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

## 리뷰 수정 회차 최종 손 변형

수정된 코드의 Git 추적 파일 복사본에서 작업별 기준 테스트를 먼저 통과시켰다. 무거운 실행은 순차 처리했고 속성 테스트는 H-46 기본 횟수를 사용했다. D-39 16개, D-40 14개, D-41 15개, 총 45개 모두 실제 컴파일 뒤 테스트 실패로 검출됐다. 생존·컴파일 실패·시간 제한은 각각 0개다. 메인 리뷰의 생존 축 여섯 개(가격일 통화, 결제일, 시장, 정규장 선택, 동일 심볼의 다른 시장, 상세 자기 부모)를 포함하며 null 사이 역전도 별도로 검증했다.

| 번호 | 바꾼 것 | 검출 테스트(대표) | 결과 |
|---|---|---|---|
| R2-D39-01 | 종목 봉의 ETF 종류 제한 제거: 지수 종류를 USD 종목 봉으로 위장 | domestic_kinds_cannot_relabel_adjusted_usd_candles | 컴파일 성공·검출 |
| R2-D39-02 | 리뷰 생존: 국내 지수 가격일 통화 KRW → USD | holiday_carry_uses_krw_price_dates_and_preserves_settlement_dates | 컴파일 성공·검출 |
| R2-D39-03 | 리뷰 생존: 결제일 → 국내 가격일 | holiday_carry_uses_krw_price_dates_and_preserves_settlement_dates | 컴파일 성공·검출 |
| R2-D39-04 | 리뷰 생존: 미국 세션의 시장 검사 무력화 | regular_close_controls_finality_and_same_day_labels_are_accepted | 컴파일 성공·검출 |
| R2-D39-05 | 리뷰 생존: 장전까지 정규장 선택 후보로 허용 | regular_close_controls_finality_and_same_day_labels_are_accepted | 컴파일 성공·검출 |
| R2-D39-06 | 수집 시각과 정규장 종료가 같은 봉을 확정 | regular_close_controls_finality_and_same_day_labels_are_accepted | 컴파일 성공·검출 |
| R2-D39-07 | 진행 중·미확정 입력의 확정 상태 검사 제거 | index_path_rejects_invalid_sources | 컴파일 성공·검출 |
| R2-D39-08 | 벤치마크 입력의 일봉 검사 제거 | index_path_rejects_invalid_sources | 컴파일 성공·검출 |
| R2-D39-09 | 동일 지수 검사 반전 | holiday_carry_uses_krw_price_dates_and_preserves_settlement_dates | 컴파일 성공·검출 |
| R2-D39-10 | 공유 수익률 계산의 분모·분자 역전 | holiday_carry_uses_krw_price_dates_and_preserves_settlement_dates | 컴파일 성공·검출 |
| R2-D39-11 | 지수 봉 4097개 상한을 한 개 완화 | series_rejects_misalignment_duplicates_and_input_limits | 컴파일 성공·검출 |
| R2-D39-12 | 스냅샷 4097개 상한을 한 개 완화 | series_rejects_misalignment_duplicates_and_input_limits | 컴파일 성공·검출 |
| R2-D39-13 | 신호 zip 길이 검사 반전 | holiday_carry_uses_krw_price_dates_and_preserves_settlement_dates | 컴파일 성공·검출 |
| R2-D39-14 | 지수 벤치마크 통화 제한 반전 | holiday_carry_uses_krw_price_dates_and_preserves_settlement_dates | 컴파일 성공·검출 |
| R2-D39-15 | KOSPI를 KOSDAQ로 재표지 | missing_stock_history_cannot_be_used_for_mixed_weights | 컴파일 성공·검출 |
| R2-D39-16 | 국내 신호의 종가 달력을 미국 시장으로 변경 | holiday_carry_uses_krw_price_dates_and_preserves_settlement_dates | 컴파일 성공·검출 |
| R2-D40-01 | 대분류 필터 반전: 하위 섹터 선택 | asset_sector_cap_combines_samsung_and_hynix_instead_of_splitting_subsectors | 컴파일 성공·검출 |
| R2-D40-02 | 전체 시가총액이 작은 대분류를 선택 | larger_major_total_wins_even_when_member_weight_is_smaller | 컴파일 성공·검출 |
| R2-D40-03 | 전체 시총 동률과 미제공의 식별자 순 역전 | equal_totals_and_missing_totals_use_sector_id_order | 컴파일 성공·검출 |
| R2-D40-04 | 일부 시가총액 미제공에서 전체 식별자 fallback 누락 | equal_totals_and_missing_totals_use_sector_id_order | 컴파일 성공·검출 |
| R2-D40-05 | 리뷰 생존: 같은 심볼의 다른 시장 구성을 허용 | identical_symbols_from_other_markets_are_not_memberships | 컴파일 성공·검출 |
| R2-D40-06 | 소속 심볼 검사를 무력화 | missing_major_and_unknown_type_remain_explicitly_unclassified | 컴파일 성공·검출 |
| R2-D40-07 | 마스터의 ETF·펀드를 미분류로 잘못 분류 | etf_and_fund_classification_comes_from_master | 컴파일 성공·검출 |
| R2-D40-08 | 대분류 소속 없는 일반 종목을 ETF로 잘못 분류 | identical_symbols_from_other_markets_are_not_memberships | 컴파일 성공·검출 |
| R2-D40-09 | 단일 대분류를 여러 대분류로 표지 | samsung_and_hynix_stay_in_the_same_major_sector | 컴파일 성공·검출 |
| R2-D40-10 | 시가총액 미제공 표지와 fallback 제거 | equal_totals_and_missing_totals_use_sector_id_order | 컴파일 성공·검출 |
| R2-D40-11 | 대분류 조회 중복 검사 반전 | asset_sector_cap_combines_samsung_and_hynix_instead_of_splitting_subsectors | 컴파일 성공·검출 |
| R2-D40-12 | 대분류 전체 시총 합산을 차감으로 변경 | larger_major_total_wins_even_when_member_weight_is_smaller | 컴파일 성공·검출 |
| R2-D40-13 | 분류 결과의 소속·이유 메타데이터 유실 | equal_totals_and_missing_totals_use_sector_id_order | 컴파일 성공·검출 |
| R2-D40-14 | 알려지지 않은 마스터 유형을 ETF로 추측 | missing_major_and_unknown_type_remain_explicitly_unclassified | 컴파일 성공·검출 |
| R2-D41-01 | 리뷰 생존: 중·소분류 상세의 자기 부모 검사 제거 | review_detail_rejects_self_parent_for_non_major_depths | 컴파일 성공·검출 |
| R2-D41-02 | null 행에서 이전 비교값을 지워 null 너머 역전을 허용 | review_rank_order_compares_available_values_across_nulls | 컴파일 성공·검출 |
| R2-D41-03 | 공통 순위 검사에서 0과 중복 순위 허용 | investor_invalid_values_order_counts_currency_and_identity_are_closed | 컴파일 성공·검출 |
| R2-D41-04 | 공통 순위 빈자리 판정 반전 | investor_groups_keep_independent_times_directions_units_and_gaps | 컴파일 성공·검출 |
| R2-D41-05 | 순매도 랭킹을 순매수 방향으로 표지 | investor_groups_keep_independent_times_directions_units_and_gaps | 컴파일 성공·검출 |
| R2-D41-06 | 투자자 그룹별 서버 집계 시각을 수집 시각으로 덮기 | investor_empty_groups_preserve_unavailable_time | 컴파일 성공·검출 |
| R2-D41-07 | 투자자 랭킹의 0원 금액을 허용 | investor_invalid_values_order_counts_currency_and_identity_are_closed | 컴파일 성공·검출 |
| R2-D41-08 | 투자자 랭킹의 소수 원화 금액을 허용 | investor_invalid_values_order_counts_currency_and_identity_are_closed | 컴파일 성공·검출 |
| R2-D41-09 | 섹터 랭킹 최대 100개 상한을 한 개 완화 | sector_rankings_preserve_nulls_and_reject_invalid_inputs | 컴파일 성공·검출 |
| R2-D41-10 | 일곱 번째 기간 수익률을 조용히 무시 | review_sector_detail_rejects_a_seventh_rate | 컴파일 성공·검출 |
| R2-D41-11 | 공통 섹터 금액 파서의 KRW 단위를 USD로 변경 | review_shared_amount_and_change_types_preserve_zero_and_unavailable | 컴파일 성공·검출 |
| R2-D41-12 | 공통 등락률 파서에서 -1 경계를 잘못 거부 | sector_metadata_and_rates_reject_invalid_hierarchy_and_periods | 컴파일 성공·검출 |
| R2-D41-13 | 서로 다른 거래대금·시가총액 필드를 바꾸어 봉인 | review_rank_order_compares_available_values_across_nulls | 컴파일 성공·검출 |
| R2-D41-14 | Debug가 금액 외 섹터 이름까지 가림 | debug_redacts_only_money_and_keeps_identifiers_and_summary | 컴파일 성공·검출 |
| R2-D41-15 | 공통 SectorAmount Debug에서 금액 원문 노출 | debug_redacts_only_money_and_keeps_identifiers_and_summary | 컴파일 성공·검출 |

변형별 전체 실패 테스트와 로그는 target/D-39/review-hand-mutants/results.json, 소스 해시는 source-hashes.json에 있다. 임시 소스 복사본은 실행 종료 시 제거했고 cleanup.json으로 확인했다. 단순 컴파일 실패를 검출로 세지 않았다.

## 자체 Opus 리뷰의 평가

0d46eb8..66ede36을 새 Claude Opus 5.5가 읽기 전용으로 독립 리뷰했다. 구현 보고서·ADR 초안·실행 로그는 전달하지 않았다. 원본 집계는 D-39 차단 0·중요 0·사소 1, D-40 차단 0·중요 1·사소 2, D-41 전부 0이다. 실행 검사는 하지 않았다.

D-40 중요 지적은 구성원 한 개라도 시총 null이면 ID 순이 되는 점이다. 사용자가 확정한 “구성원 시가총액 미제공이면 섹터 식별자 순으로만 정하고 표지”와 이 ADR의 기존 규칙에 해당하므로 기각한다. 제공된 값만 합산해 전체 시총으로 잘못 부르거나 별도 응답 의존성을 추가하지 않는다. 단일 소속은 Single과 실제 섹터를 보존하면서 CapitalizationUnavailable 이유도 유지한다. 이 이유는 선택 실패가 아니라 구성 시총의 미제공을 나타낸다.

SectorId 동률 순은 기존 SectorId::Ord인 문자열 순이다. 예를 들어 같은 시총의 "9"와 "10"은 "10"을 고른다. OpenAPI의 섹터 목록 응답 자체가 숫자 순이라는 계약과 선택의 결정적 동률 순을 구분한다. 이 변경에서 전역 SectorId 의미나 기존 소비자의 정렬을 바꾸지 않는다.

D-39의 사소 지적은 기존 Sp500Etf 이름표만으로 실제 S&P500 추종 ETF인지 인증하지 못한다는 점이다. 허용 심볼을 임의로 발명하지 않고 A-09·A-15·E-08의 ETF 벤치마크 선택/출처 계약에 인계한다. 이번에 닫은 국내 지수의 종목 봉 위장 경로와는 구분한다.

리뷰 기록은 docs/reviews/D-39.md·D-40.md·D-41.md에 원본 등급과 평가를 함께 보존한다. 추가 차단·중요 수용 지적은 남지 않았다. 리뷰 이후 추가한 자동 변이 경계 테스트는 구현자 검증이며 Opus 재리뷰를 주장하지 않는다.

## 리뷰 수정 회차 자동 변이

기준은 master 580a19874ee4ad32c21c4c3f83a0a4142b3a4858이다. `just mutants-diff 580a19874ee4ad32c21c4c3f83a0a4142b3a4858`를 실제 한 번 실행했으나 선행 test-diff가 승인된 생성 경로 이관 13건을 지적해 변이 실행 전에 종료됐다. 이 명령의 통과를 주장하지 않는다. 하네스·기준·허용 목록을 고치지 않았다.

그 게이트 결과와 별개로 동일한 변경분 패치·대상 Rust 파일·H-46 프로필(opt-level 1, debug 0, overflow/debug assertions 유지, nextest, jobs 2, 테스트 스레드 4)로 cargo-mutants 진단을 실행했다. 기준 테스트를 통과한 첫 실행은 198개 중 검출 106·미검출 8·컴파일 불가 84·시간 초과 0이다. 실제 실행 시간은 약 16분이다. 미검출별 처리는 다음과 같다.

| 미검출 | 처리 | 재검증 |
|---|---|---|
| SignalBenchmark 지수 달력 >4097 → >=4097 | 정확히 4097개에서 입력 원천 검증까지 도달하는 반례 추가 | 검출 |
| 지수 봉 >4097 → >=4097 | 정확히 4097개에서 중복 봉 오류를 구분하는 반례 추가 | 검출 |
| 지수 스냅샷 >4097 → >=4097 | 정확히 4097개에서 잘못된 기간 오류를 구분하는 반례 추가 | 검출 |
| 대분류 동률 ID < → <= | 후보 ID 중복을 먼저 거부해 관찰 가능한 차이가 없음. cmp == Less로 순서 비교를 명시하여 중복 경계 연산 제거 | 해당 변형 소멸, 변경 비교의 변이 검증 |
| SectorEtfs::etfs가 빈 목록 반환 | zip만으로는 빈 목록이 통과하므로 공개 반환 목록의 전체 심볼을 정확히 대조하는 테스트 추가 | 검출 |
| 섹터 랭킹 >100 → >=100 | 서로 다른 100개 섹터가 전부 반환되는 정상 경계 추가 | 검출 |
| 투자자 랭킹 금액 < → <= | 요청 개수만큼의 순매수·순매도 동률이 세 투자자 그룹 모두에서 보존되는 반례 추가 | 검출 |
| NetTradingItem Debug가 빈 출력 | 금액은 가리고 심볼·순위·가림 표지가 남는지 명시적으로 검사 | 검출 |

추가 테스트의 기준 실행은 benchmark 48개·sector_selection 11개·market_query_facts 25개가 통과했다. 최종 소스는 196개를 열거했다. cargo-mutants의 iterate로 이전에 판정한 항목을 재사용하고 신규/미판정 11개를 실제 다시 실행하여 검출 10·컴파일 불가 1·미검출 0·시간 초과 0을 얻었다(기준 테스트 다시 통과, 약 3분). 이전 판정에는 컴파일 불가도 포함되므로 이를 검출로 부풀리지 않는다.

최종 열거된 196개 각각을 이전 또는 재실행의 동일 변형 이름과 대조했고 빠진 항목은 0개다. 최종 소스의 집계는 검출 112개·컴파일 불가 84개·미검출 0개·시간 초과 0개다. 기록한 11개 소스 파일의 SHA-256도 현재 파일과 일치한다. 컴파일 불가 84개는 실행 테스트에 잡힌 변형으로 세지 않는다. 증거는 target/D-39/review-mutants-diff.log, review-auto/round1, review-auto/mutants.out, review-auto/final-aggregate.json과 receipt.json이다.

최종 자동 변이 보강과 ID 비교 단순화 뒤 손 변형 45개를 같은 방식으로 다시 실행했다. D-39 16개·D-40 14개·D-41 15개 모두 컴파일 성공 후 검출됐으며 새 소스 해시가 현재 파일과 일치한다. 실행용 임시 소스 복사본은 제거했다. 최종 결과는 review-hand-final-run.log와 review-hand-mutants/results.json이다.

## D-42 병합 호환성 (회차 3)

기존 최종 커밋 61297bae438c97c0855ecc01a00886d477e16f02에서 최신 master ecb6e5a73b3fe5a394d55c0803bb203ad984cdf6(D-42·DS-06 포함)을 받았다. 소스는 자동 병합됐고 생성물 두 파일의 충돌은 기존 config → sandbox fragment → hashes 스크립트로 재생성했다. 호환성 코드 병합 커밋은 67ddafb70df6f9fb7e41c6ea52723f24ef121d29다.

첫 cargo test -p domain은 단위 테스트 950개 통과·2개 실패였다. 실패 원인은 D-42의 아래 두 테스트가 KOSPI를 종목 봉 생성자로 넣은 것이며, D-39에서 의도대로 닫힌 경로가 BenchmarkMismatch를 반환했다. 생성자 호출 두 줄만 기존 test_benchmark 도우미로 옮겼다. 이 도우미는 KOSPI/KOSDAQ fixture를 확정 지수 봉으로 변환해 SignalBenchmark::from_index_candles에 넘긴다. 두 함수는 이 호출명 변경을 제외하면 master 원문과 바이트 단위로 같다.

새 master 대비 승인된 test-diff는 정확히 15건(테스트 14개·지원 함수 1개)이다. 이전 13건에 늘어난 항목은 다음 두 개뿐이며 단언과 기대 수치는 보존했다.

| 파일 | 테스트 | 전 → 후 | 유지한 검증 |
|---|---|---|---|
| crates/domain/src/signal_benchmark.rs | d42_calendar_limit_accepts_4097_consistent_facts | SignalBenchmark::from_candles(Kospi, Candle) → test_benchmark의 지수 분기 → SignalBenchmark::from_index_candles | 달력 4097개 허용, 수익률 0.250000 |
| crates/domain/src/signal_benchmark.rs | d42_distinct_settlement_days_can_share_the_previous_market_close | SignalBenchmark::from_candles(Kospi, Candle) → test_benchmark의 지수 분기 → SignalBenchmark::from_index_candles | 결제일 상이·가격일 동일, 수익률 0.000000 |

수정 뒤 cargo test -p domain은 단위 952개·통합 16개 통과, doctest 0개다. D-37과 공유하는 market_ranking·sector_composition의 영 금액/거래량 예제와 virtual_exit·signal_return_review의 D-42 예제는 수정 없이 통과했다. 정상 병합 커밋 훅은 워크스페이스 1392개·커버리지 대상 1043개와 커버리지 게이트까지 통과했고, test-diff의 승인된 15건만 남았다. 후속 disable-report·새 master 이후 새 끄기 0건·hooks-installed도 통과한 뒤 그 커밋에만 LEFTHOOK=0을 적용하고 사유를 메시지에 남겼다. 병합 뒤 master가 실제 조상인 상태의 별도 test-diff도 15건으로 일치했다.

원본 실패·수정 후 통과·정상 훅·이관 증거는 작업 사본 target/D-39/d42-domain-red.log, d42-domain-green.log, d42-merge-normal-hook.log, d42-new-master-test-diff.log와 d42-compatibility.json이다. 위 손 변형·자동 변이 표는 회차 2의 실행 기록이며 이번 회차의 호환성 실행 검증과 구분한다.
