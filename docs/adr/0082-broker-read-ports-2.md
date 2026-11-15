# ADR 0082: A-20 읽기 포트 2

- 상태: 확정 (2026-11-16)

- 최초 구현 기준 SHA: `150b578babf37e2324e15165609799e0765e750e`, 브랜치 `a-20`.
- 최종 master 병합 기준: `8ba1f6abf3b1050017b649d1d1a5fd63104a6363`. 2차 독립 리뷰 대상: `203067774d5b37b024ce4c7b48d5054cd0418e44`.
- 메인 정정: 현재 병합된 domain 타입으로 정의 가능한 행만 구현한다. D-41 및 미정의 수급은 아래 정의 대기로 남긴다. domain 타입을 새로 만들지 않는다.

## 결정

ADR 0055의 RPITIT `Send` Future, `Send + Sync`, `Result<T, PortError>`와 제네릭 주입을 유지한다. 계좌·종목 읽기는 BrokerRead, 지수·랭킹·수급·섹터 구성은 MarketInfo에 둔다. 이번 표가 ADR 0055의 임시 계약·정의 대기 상태를 해당 행에 한해 갱신한다. 메서드 이름은 아래의 단순 명사형으로 정했다. 주문 목록을 상태 인자로 합치지 않고 OPEN과 CLOSED의 다른 완전성 계약을 서명에서 분리한다. 조건주문·최근 틱은 소비자가 없어 추가하지 않는다.

accounts는 중복 계좌를 거부하는 BrokerAccountRows, quotes/candles 입력과 QuoteObservation.instrument는 D-33의 QuoteInstrument다. 거래 정지·미지 유형 때문에 주문 식별을 만들 수 없는 종목도 알려진 시장이면 시세 식별로 조회할 수 있다. AccountObservation은 제거한다. 계산·주문 방향·가격·수량 결정은 추가하지 않는다.

RankingQuery는 application의 요청 값이며 모든 필드는 기존 domain 타입이다. 다섯 인자를 묶고 미지 종류/기간과 상승·하락 실시간 조합을 요청 전에 거부한다(ADR 0068의 지원 조합). 산술은 없다. capability는 기존 네 봉 기능에 새 조회별 변형을 더한다. 지원 목록은 어댑터가 공급하고 support_for로 확인한다. 포트 서명 자체가 목록 준수를 실행하지는 않는다. 미지원 호출은 Unsupported(해당 capability), 봉 조회는 주기와 수정 기준 capability를 함께 확인한다.

## 계약 표

모든 출력은 Result<출력, PortError>이며 아래는 성공 값 타입이다. 모든 행의 공통 오류 분류 E는 ADR 0063대로 AccessDenied→UserAction, Unavailable→Wait(원 RetryDelay), InvalidRequest/InvalidResponse/Unsupported→Bug, NotFound→NoData다. 계좌 없음은 AccessDenied(AccountNotFound), 주문 없음은 새 NotFound(Order)다. 본문·자격 증명·임의 문자열은 오류에 없다.

| 포트 | 메서드 | 입력 | 출력 | 결합 의무·신선도·완전성 | 오류 분류 | 받는 어댑터 작업 |
|---|---|---|---|---|---|---|
| BrokerRead | accounts | 없음 | BrokerAccountRows | 같은 응답의 계좌 키·유형·표시 사실. 행별 observed_at, 서버 순서·중복 거부 | E | T-05 |
| BrokerRead | holdings | &AccountId | HoldingsObservation | 요청 계좌의 보유·토스 요약. 관측 시각 보존, 현금과 독립 | E | T-05 |
| BrokerRead | transactions | &AccountId, SettlementWindow, &TransactionFilter | TransactionRead | 요청 계좌·결제 창·필터. Complete 증거 또는 수신 행·격리·불완전 이유 | E | T-06 |
| BrokerRead | quotes | &[QuoteInstrument] | Vec<QuoteObservation> | 요청 마스터의 안정적 ID·시장·심볼, 입력 슬롯과 결과 결합. 종목별 Result·관측 시각 | E | T-07 |
| BrokerRead | candles | &QuoteInstrument, TimeInterval, CandlePeriod, PriceAdjustment | Vec<Candle> | 마스터 ID·시장·심볼 및 요청 기간·주기·수정 여부. 봉 시각·확정 여부·누락 보존 | E | T-07 |
| BrokerRead | buying_power | &AccountId, Currency | BuyingPower | 요청 계좌·통화. observed_at, 조회 실패를 0으로 대체하지 않음 | E | T-11 |
| MarketInfo | fx_rate | FxPair, FxRateKind, Option<Timestamp> | FxRate | 요청 통화쌍·종류·선택 시각. 출처·유효 기간 | E | T-08 |
| MarketInfo | order_fx_rates | 없음 | OrderFxRates | 한 응답의 매수·중간 환율 쌍. 유효 기간 보존 | E | T-08 |
| BrokerRead | sellable_quantity | account: &AccountId, instrument: &ResolvedOrderInstrument | SellableQuantityFact | 요청 계좌·주문 종목과 사실을 결합. observed_at | E | T-11 |
| BrokerRead | open_orders | account: &AccountId | BrokerOrderSet | 요청 계좌 전체 OPEN. 각 행의 계좌·마스터 주문 종목·관측 시각, coverage | E | T-15 |
| BrokerRead | closed_orders | account: &AccountId, evaluation_window: TimeInterval | ClosedOrderSet | 요청 계좌와 필요한 체결 평가 구간 evaluation_window. 각 행 계좌·종목, covered·observed_at·completeness | E | T-15 |
| BrokerRead | order | account: &AccountId, order_id: &BrokerOrderId | BrokerOrder | 요청 계좌·order_id와 응답 계좌·종목 결합. observed_at, 미지 상태 보존 | E | T-15 |
| BrokerRead | stock_warnings | instrument: &ResolvedOrderInstrument | StockWarnings | 계좌에 종속되지 않는 요청 종목 사실. 종목·observed_at·미지 경고 보존 | E | T-11 |
| BrokerRead | instruments | market: KnownMarket | MasterInstrumentFacts | 요청 시장의 전체 마스터. 응답의 안정적 ID·심볼·이름·유형·observed_at 결합 | E | T-09 |
| BrokerRead | orderbook | instrument: &QuoteInstrument | Orderbook | 요청 QuoteInstrument·응답 통화 결합. 서버 timestamp/수집 시각·Unknown 분리 | E | T-07 |
| BrokerRead | price_limits | instrument: &QuoteInstrument | PriceLimits | 요청 QuoteInstrument·응답 통화 결합. 상하한·Unlimited, 서버/수집 시각 | E | T-07 |
| BrokerRead | commission_rate | account: &AccountId, market: KnownMarket | CommissionRateFact | 요청 계좌의 해당 시장 요율. CommissionValidity 보존, 계좌 진위는 T-11 의무 | E | T-11 |
| MarketInfo | indicator_quotes | codes: &[IndicatorCode] | Vec<MarketIndicatorQuote> | 요청 지표 코드별 단위·출처·관측/수집 시각. 전일 대비를 계산하지 않음 | E | T-08 |
| MarketInfo | indicator_candles | code: IndicatorCode, window: TimeInterval, period: CandlePeriod | Vec<MarketIndicatorCandle> | 요청 지표·기간·주기 결합. 단위·봉 시작·확정 여부. 종목 Candle로 변환 금지 | E | T-08 |
| MarketInfo | rankings | query: &RankingQuery | StockRanking | 요청 kind·market·duration·limit·caution. rankedAt/수집 시각·TopN·순위 빈자리 | E | T-10 |
| MarketInfo | stock_investor_flow | instrument: &QuoteInstrument, from: CalendarDate, through: CalendarDate | Vec<InvestorFlow> | 요청 국내 종목·[from, through] 날짜. 종목별 일간 주식 수, updatedAt/수집 시각·Unknown 확정성 | E | T-10 |
| MarketInfo | indicator_investor_flow | code: IndicatorCode, from: CalendarDate, through: CalendarDate, interval: FlowInterval | Vec<InvestorFlow> | 요청 KOSPI/KOSDAQ·[from, through]·interval. 원화 수급, 계층·합계 차이·Unknown 확정성 | E | T-08/T-10 |
| MarketInfo | sector_composition | sector: &SectorId, market: KnownMarket | SectorComposition | 요청 sector·market. IncludingDescendants·서버 시각 Unknown·수집 시각 | E | T-09 |
| MarketInfo | calendar | market: KnownMarket | MarketCalendar | 요청 KnownMarket의 오늘·전후 영업일. 장 구간·휴장 보존. 서버의 오늘 기준 날짜 | E | T-08 |

## 어댑터 의무와 소비자

- T-05/A-06/A-08: holdings 및 현금은 요청 계좌와 응답을 실제로 대조한다. 이 응답 타입은 계좌 자체를 담지 않으므로 포트 서명만으로 응답 진위를 인증하지 않는다. 계좌 목록은 유형·표시 사실이 같은 행에서 나온다.
- T-07/A-14/O-01: quotes는 1~200개, 빈 입력·201개 이상은 BatchLimitExceeded. 입력 순서·개수·중복 슬롯을 그대로 유지하고 종목별 실패는 안쪽 Result로 보존한다. 배치 전체 실패만 바깥 Err다. 호출자가 분할한다. candles는 [start,end), 시간순·주기·수정 여부·안정적 ID를 대조하며 서버 페이지를 받아 요청 범위를 채운다. 실패한 페이지를 버리고 완전한 열로 선언하지 않는다.
- T-15/O-01/O-04/O-11/O-19/O-25: open_orders는 종목·기간 필터 없는 계좌 전체 OPEN이다. 행의 실제 요청 계좌와 마스터 종목을 대조한다. 식별 불가·검증 실패·페이지 누락이면 Incomplete이며 버리고 Complete로 만들지 않는다. 실패를 빈 성공으로 바꾸지 않는다. 단건 order는 요청 ID와 응답 ID도 대조한다. 주문 조회는 읽기이며 재전송을 결정하지 않는다.
- T-15/D-38/A-16: closed_orders의 evaluation_window는 ADR 0075의 소비자가 필요한 체결 평가 구간 (start,end]이며 HTTP orderedAt 필터가 아니다. 시작 시각과 같은 체결은 제외하고 끝 시각과 같은 체결은 포함하는 domain 판정 의미를 그대로 따른다. T-15가 이 평가 구간에 기여할 수 있는 주문을 모두 포함하는 HTTP 주문 시각 조회 범위와 페이지 수집을 책임진다. 페이지 끝·누락/격리 없음·평가에 필요한 오래된 주문의 늦은 체결 포함을 입증할 때만 Complete다. 임의의 하루/최대 주문 수명을 추정해 조회 시작을 자르지 않는다. API의 역사 조회 한계·주문 수명·CLOSED 전환 지연 때문에 포괄을 입증하지 못하면 Complete를 선언하지 않는다. 주문 시각 창을 체결 포괄 범위로 복사하지 않는다. 입증하지 못하면 페이지/기간 포괄 근거 부족은 MissingPages, 식별/검증 실패는 RejectedRows, 요청 실패는 RequestFailed로 보존한다. covered는 어댑터가 확보한 체결 포괄 사실이고 evaluation_window와 같을 수 있지만 입력을 복사하는 것만으로 생성하지 않는다. 실제 관측 시각은 Clock 사실에서 받는다. CLOSED의 PartialFilled는 현재 domain이 거부하므로 버리고 Complete로 만들지 않는다. 아무 유효 자료도 없으면 바깥 PortError, 유효 종료 행을 받았다면 가능한 범위와 불완전 원인을 보존한다.
- T-11/O-01/O-04: 매도가능수량은 요청 계좌·종목과 사실의 계좌·종목을 대조한다. 경고 응답에는 계좌가 없으며 요청 종목을 반드시 결합한다. 다른 계좌에 묶인 경고로 가장하지 않는다. 승인 직전 재조회·신선도 허용 간격은 O 작업 책임이다. 수수료는 계좌 헤더로 받은 응답에서 시장에 맞는 사실을 반환한다. CommissionRateFact 자체에는 계좌 키·관측 시각이 없으므로 어댑터가 요청 결합을 책임지고 유효 기간을 보존한다.
- T-09/U-29/O-01: instruments는 /stocks/all의 해당 시장 전체를 status/securityType/commonShare 필터 없이 받는다. Symbol/ISIN으로 안정적 ID를 발명하지 않는다. 구성원 하나라도 검증할 수 없으면 전체 InvalidResponse로 돌려주며 완전 목록처럼 누락하지 않는다. 단건 검색과 필터링은 소비자가 이 마스터에서 한다.
- T-08/A-09: indicator_quotes는 코드 입력 순서·개수·중복을 유지한다. 알려진 지표만 요청하며 Unknown은 UnsupportedQuery. 누락/단위/식별 불일치는 InvalidResponse, 자료 없음은 해당 조회 오류다. 미국 지수·금리는 이 조회의 공식 값이라고 주장하지 않고 기존 ETF 시세/외부 MacroSeries로 구분한다. indicator_candles는 국채 분봉·Unknown을 요청하지 않으며 domain의 Finalized/InProgress/Unknown을 보존한다. 실패 페이지는 바깥 Err이고 빈 완전 성공으로 숨기지 않는다.
- T-10/G-07/M-04: rankings는 전체 시장 완전성 대신 TopN 및 순위 빈자리를 보존한다. stock_investor_flow는 국내만, 일별 주식 수다. indicator_investor_flow는 KOSPI/KOSDAQ만, 알려진 FlowInterval만 허용한다. from≤through를 요청 전에 검증하며 뒤집힌 날짜·지원 밖 조합은 UnsupportedQuery다. 어댑터는 count/until 페이지로 포함 날짜 창을 채우고 날짜순 반환한다. 페이지 실패는 바깥 Err, 누락된 nullable 값은 0으로 채우지 않는다.
- T-09/G-07/M-04/D-12: sector_composition은 /sectors/{sectorId}/stocks 결과의 시장별 구성이다. 섹터 소속 선택·SectorAssignment 생성은 하지 않는다(D-40). 시장·심볼을 마스터와 대조하는 책임을 유지한다.
- T-08/J-05/A-14/O-01: calendar는 KnownMarket만 입력받고 현재 서버 기준 3개 날짜를 반환한다. 날짜별 세션·오프셋·휴장·경매 구간을 보존한다. 임의 과거 날짜 조회는 현재 소비자가 없어 만들지 않는다.

이 표의 의미 의무는 포트 선언이 실행하지 않는다. 타입·Send·오류의 테스트는 로컬 가짜 계약 증거이고 실제 요청/응답/페이지의 검증은 각 T 작업이 소유한다. 실제 API·계좌·키는 사용하지 않는다.

## 정의 대기

| 행 | 필요한 타입 | 받는 작업 ID |
|---|---|---|
| 투자자별 순매수·순매도 랭킹 | 투자자 랭킹의 단위·기간·집계 시각을 봉인한 타입(D-41 확정 이름 사용) | D-41 → A-21 읽기 포트 3 → T-10 |
| 섹터 목록·상세·랭킹 | SectorCatalog·SectorDetail·SectorRanking에 해당하는 D-41 확정 타입 | D-41 → A-21 읽기 포트 3 → T-09/T-10 |
| ETF 구성 | ETF 구성의 D-41 확정 타입 | D-41 → A-21 읽기 포트 3 → T-09 |
| 프로그램 수급 | 프로그램 매매 단위·집계 기간·관측 시각의 봉인 타입 | domain 선행 작업 필요(메인이 ID 등록) → A-21 → T-10 |
| 공매도 수급 | 공매도 수량·금액·기간·관측 시각의 봉인 타입 | domain 선행 작업 필요(메인이 ID 등록) → A-21 → T-10 |
| 신용 수급 | 신용 잔고·증감·기간·관측 시각의 봉인 타입 | domain 선행 작업 필요(메인이 ID 등록) → A-21 → T-10 |
| 대차 수급 | 대차 수량·잔고·기간·관측 시각의 봉인 타입 | domain 선행 작업 필요(메인이 ID 등록) → A-21 → T-10 |

## 인계

A-21은 위 정의 대기 행만 이어 받는다. tasks.md 행과 미정의 수급 domain 작업 ID는 메인이 등록한다. D-39의 지수 벤치마크 연결, D-40의 섹터 소속 생성은 본 포트가 대체하지 않는다. T-04/T-15는 실제 order-not-found를 새 MissingData::Order로 정규화해야 한다(현재 주문 전용 오류를 거부하는 매핑은 이번에 바꾸지 않음). T-15는 CLOSED 완전성 입증·부분 체결·외부 주문 포함 및 filledAmount 수수료/세금 포함을 실측한다. 하네스 후속은 원시 사실 생성자의 호출 위치를 허용 어댑터/테스트로 제한한다. 공개 생성자는 네트워크 진위를 인증하지 않는다.

## 기존 테스트 변경과 검증

ports.rs의 계좌 헬퍼는 AccountObservation의 별개 키/표시 생성 대신 BrokerAccountRow를 만들며 accounts의 타입은 Vec<AccountObservation> → BrokerAccountRows, first → rows().first(), display → display_fact()다. quotes/candles 호출은 ResolvedOrderInstrument → QuoteInstrument이며 보유·주문 식별은 기존 타입을 유지한다. 새 필수 trait 메서드의 가짜 impl을 추가한다. 기존 단언은 삭제·완화하지 않는다. error_class.rs의 독립 기대 match에는 새 MissingData::Order → NoData(Order) 분기만 추가한다. test-diff가 이 승인된 변경만 거부하면 나머지 검사 통과 후 지정한 커밋에 한해 LEFTHOOK=0을 사용한다.

새 의존성·주석·검사 끄기·domain 변경은 없다. 프로젝트 설정 hooks=true는 확인했으나 현재 세션 프로젝트/훅 신뢰 적용은 확인되지 않아 실제 Git 훅/검사 결과를 증거로 삼는다. 시작 frozen install 및 lefthook install 성공. 시작 전체 검사는 지시대로 생략했다. RankingQuery 테스트를 먼저 실행하여 미정의 타입·엔드포인트 컴파일 실패를 확인한 뒤 구현했다.

## 손 변형 결과

최초 구현 보고의 27개 전부 검출 주장은 잘못된 집계였다. 당시 기록은 25개 검출·2개 컴파일 불가였다. 컴파일 불가는 검출로 세지 않는다. 수정 후 limit 변형은 원 입력을 소비하는 생성자 필드 고정으로 보완하고 재실행했다.

mktemp -d 복사본에 최종 수정 소스·테스트를 복사해 한 번에 하나씩 변형했다. 요청 검증/필드 보존은 cargo test --locked -p application --test read_queries, capability/Order 오류는 --test ports a20_capabilities로 검사했다. 29개 모두 실제 컴파일 후 테스트 실패로 검출됐고 생존 0·컴파일 불가 0이다. 기준과 복구 검사도 통과했다. 순수 trait 선언에는 실행 분기가 없으며 실제 HTTP 계약의 변이는 각 T 작업에 남는다. 제품 자동 변이 검사는 실행하지 않았다.

| 번호 | 바꾼 것 | 결과 | 잡은 테스트 |
|---|---|---|---|
| 1 | kind 고정 MarketTradingAmount | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 2 | kind 고정 MarketTradingVolume | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 3 | kind 고정 TossTradingAmount | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 4 | kind 고정 TossTradingVolume | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 5 | kind 고정 TopGainers | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 6 | kind 고정 TopLosers | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 7 | duration 고정 Realtime | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 8 | duration 고정 Day | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 9 | duration 고정 Week | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 10 | duration 고정 Month | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 11 | duration 고정 ThreeMonths | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 12 | duration 고정 SixMonths | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 13 | duration 고정 Year | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 14 | market 고정 Kr | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 15 | market 고정 Us | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 16 | caution 고정 Included | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 17 | caution 고정 Excluded | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 18 | limit 고정 7 | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 19 | limit 고정 93 | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 20 | 미지 종류 허용 | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 21 | TOP 실시간/미지 기간 허용 | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 22 | 일반 랭킹 미지 기간 허용 | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 23 | TOP 정상 기간 거부 | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 24 | 일반 정상 기간 거부 | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 25 | 오류 엔드포인트 교환 | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 26 | 오류 상태 교환 | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 27 | 오류 원인 교환 | 검출 (exit 101) | ranking_request_matches_domain_supported_combinations_and_preserves_every_field |
| 28 | 지원 capability를 미지원으로 | 검출 (exit 101) | a20_capabilities_and_new_endpoint_errors_preserve_closed_classification |
| 29 | Order 자료 없음을 Stock으로 오분류 | 검출 (exit 101) | a20_capabilities_and_new_endpoint_errors_preserve_closed_classification |

## 외부 컴파일과 승인된 테스트 차이

외부 소비자 crate에서 허용 제네릭 호출 1건은 컴파일 성공했다. 주문 식별을 시세 입력으로 전달, 미지 시장을 calendar에 전달, ClosedOrderSet 대신 BrokerOrderSet 수신, BrokerRead에서 rankings 호출, RankingQuery의 비공개 필드 조립, 제거한 AccountObservation 가져오기 6건은 각각 컴파일에서 거부됐다. 실험 파일은 target/A-20 아래이며 제품 소스가 아니다.

application 좁은 검사 45개 및 clippy 통과. test-diff는 승인된 기존 호출부/헬퍼/impl/가져오기 변경과 새 오류 기대 분기에서 15건을 보고했다. 세 기존 테스트의 단언 변경은 accounts의 rows()/display_fact() 및 quotes/candles의 QuoteInstrument 호출 이관뿐이다. expected_no_data는 Order→NoData(Order) 분기만 더했다. 기존 단언은 지우거나 약하게 만들지 않았다.

생성물 보호 파일은 .codex/config.toml(새 ADR 읽기 전용 등록), harness/claude-hooks/hashes.json(해당 설정 해시)만 공식 스크립트로 갱신했다. 허용 범위를 넓히거나 규칙을 삭제하지 않았다.

| 파일 | 기존 테스트 또는 헬퍼 | 전 → 후 |
|---|---|---|
| crates/application/tests/ports.rs | successful_facts_survive_a_separate_cash_failure | Vec<AccountObservation>·first/display → BrokerAccountRows·rows().first()/display_fact(), 동일 계좌 키·표시 사실 단언 유지 |
| crates/application/tests/ports.rs | quote_batch_has_individual_results_and_caller_splits_above_two_hundred | 주문 식별 입력 → 시세 식별 입력, 개별 성공/실패·200/201/빈 목록 단언 유지 |
| crates/application/tests/ports.rs | failures_before_any_data_remain_typed_errors | quotes/candles의 주문 식별 입력 → 시세 식별 입력, 각 오류 단언 유지 |
| crates/application/tests/ports.rs | call_market_and_history 및 account/instrument/master_instrument/quoted | 확정 타입의 입력·생성자·접근자로 기계적 이전, 거래·보유·환율 기대값 유지 |
| crates/application/tests/ports.rs | FakeBroker의 BrokerRead/MarketInfo impl | 필수 신규 메서드 및 확정 서명 추가, 기존 성공/불완전/실패 시나리오 유지 |
| crates/application/tests/error_class.rs | expected_no_data | 기존 세 분기 유지 + 새 Order→NoData(Order) 소진 분기 |

최초 정상 커밋 훅은 전체 워크스페이스 1,346/1,346, 커버리지 대상 945/945 및 coverage-check 위반 없음까지 통과한 뒤 승인된 test-diff 15건만 거부했다. test-diff 뒤의 disable-report·disable-report-since·hooks-installed도 별도 실행하여 확인한다. 지시문의 사전 승인에 따라 구현 커밋 한 건만 LEFTHOOK=0으로 만들며 커밋 메시지에 사유를 남긴다. 최종 결과는 정상 훅 실행, 개별 실패 재검증, 승인된 test-diff 예외를 구분해 아래에 기록한다.

## 리뷰 수정 결정

1차 리뷰와 새 Codex 비판을 거쳐 BrokerRead 표 9행을 정정했다. 종료 조회 입력은 HTTP 주문 시각 창 대신 필요한 체결 평가 구간으로 확정한다. HTTP 주문 시각 조회 범위는 T-15 내부 책임이며, domain 생성자가 누락 주문의 존재를 인증한다고 주장하지 않는다. 늦은 체결 누락을 Complete라고 거짓 선언하면 잘못된 정상 계산이 가능하다는 비판 재현을 수용한다. 실제 어댑터 장애 또는 안전한 구현 자체가 불가능하다는 뜻은 아니다.

가짜 종료 조회의 응답 covered=(0,60), observed=71은 평가 요청 (11,23)과 독립된 고정 자료로 두었다. MissingPages는 순매수 계산에서 Query(MissingPages)로 남으며 정상 0으로 승격되지 않는다. 가짜 랭킹의 종류·기간 고정값을 제거하고 서로 다른 세 요청을 검증한다. 회귀 테스트는 수정 전 MarketTradingVolume 요청에 TopLosers가 나와 실패했고 수정 후 통과한다.

수수료 사실의 계좌·관측 시각 부재는 기존 domain 제한이다. T-11은 서로 다른 계좌 응답·캐시가 섞이지 않는 수용 테스트를 둔다. 소비 단계 계좌 결합을 타입으로 강제할지는 domain 후속 검토로 남기되 A-21 범위를 임의로 넓히지 않는다. 지표/수급의 지원 밖 입력과 역전 날짜의 HTTP 전 거부는 T-08/T-10의 명시적 수용 항목이다. 포트 선언·가짜 테스트가 이를 실행했다고 주장하지 않는다.

첫 보고의 손 변형 집계를 정정한다. 최초 기록은 25개 컴파일 후 검출·2개 컴파일 불가이며 컴파일 불가를 검출 수에 세지 않는다. 위 표는 수정 후 실제 재실행 결과로 교체했다.

## H-46 병합

최종화 중 master 8ba1f6abf3b1050017b649d1d1a5fd63104a6363(Merge h-46)을 --no-ff로 받았다. lib.rs 충돌은 read_queries와 self_check 양쪽 선언을 모두 보존하고 .codex/config.toml·hashes.json은 공식 생성 스크립트로 재생성했다. 리뷰 수정분은 이 병합 트리에 함께 담는다. H-46이 fast를 선택하더라도 docs/설정 변경은 보수적으로 전체 검사로 승격하는 정책이므로 그 정책을 따른다. 손으로 전체 검사를 따로 반복하지 않는다. 이 작업이 추가한 테스트에는 속성 횟수 숫자 리터럴이 없다. master에서 병합된 domain/하네스 테스트 변경은 A-20 구현자가 수정한 것이 아니다.

## 2차 사소 지적 평가와 인계

2차 새 Opus 리뷰는 차단·중요 0건, 사소 5건(추정 1건)이다. application 53개와 clippy를 독립 실행했다. 손 변형 재현은 exporters/dynamic-target/cwd-unknown 훅에 막혀 미검증이고 우회하지 않았다. 이 독립 실행 범위를 작성자의 최종 손 변형 결과와 구분한다.

- 요청 검증은 종류·기간을 와일드카드 없이 소진하는 match로 보완했다. application은 ADR 0068의 요청 지원 조합을 검증하고 domain은 응답 사실을 검증한다. 기존 domain impl은 바꾸지 않으며 새 enum 변형은 제품·테스트 양쪽의 소진 검사를 깨뜨려 검토를 요구한다.
- 지표 가짜가 입력을 무시하던 예시를 수정했다. KRBond3Y/KOSDAQ/KOSPI/중복KRBond3Y의 요청 순서·단위·개수를 독립 기대값으로 검사하고 Unknown은 UnsupportedQuery다. KR/US 두 시장의 마스터·수수료 사실 동등성·캘린더 시장도 검증한다. 국내 수급 예시는 국내 QuoteInstrument를 사용한다. 실제 HTTP 전 지원 검사까지 구현한 증거로 주장하지 않는다.
- 초기 기준 SHA는 최초 구현 기준이고 현재 수정 기준과 구분해 적었다. 초안 상태는 원 지시문의 명시 요구여서 유지한다. 메인은 이 초안을 채택·번호 부여할 때 architecture 3.3의 BrokerRead/MarketInfo 역할과 ADR0055의 임시·정의 대기 현황을 갱신한다. 이 초안은 해당 행의 현재 구현 계약이며 기존 확정 ADR 내용은 구현자가 고치지 않는다.
- 병합+리뷰 수정이 함께 든 2030677은 독립 리뷰 대상 SHA로 고정한다. 이력은 다시 쓰지 않고 이번 사소 보완은 후속 커밋으로 분리한다.
- T-15 수용 항목: CLOSED 역사 조회 범위·주문 유효 기간·CLOSED 전환 지연 및 평가 시작 OPEN 사실 같은 포괄 근거를 실측/계약으로 확정하고, 그 근거를 가진 정상 Complete 사례와 오래된 늦은 체결/근거 부족 Incomplete 사례를 모두 검사한다. 근거를 확보하지 못하면 보수적 Incomplete/NotCompared를 유지한다. 일간 대조가 항상 가능하다고 약속하거나 임의 주문 수명 상수를 넣지 않는다. 실제 키 실측은 사용자 몫이다.

수정자는 사소 지적을 직접 보완하고 좁은 회귀/소진 조합/손 변형으로 검증했다. 추가 독립 에이전트는 실행하지 않았으며 최종 사소 보완분의 검증은 작성자 검증이다. 실제 어댑터 계약·기존 확정 문서 채택은 위 인계에 남는다.

## 최종 검사와 커버리지 변동 인계

최종 정상 커밋 훅은 master8ba1f6a 기준 check-fast를 실행했고 문서·설정 변경으로 전체 검사에 승격했다. 워크스페이스1303/1303은 통과했다. 커버리지 대상955/955 테스트는 통과했지만, A-20에서 변경하지 않은 domain/src/tick_size.rs:273의 속성 테스트 본체에서 remainder==0 분기가 미도달하여 최초 coverage-check는1건 실패했다. 이 실패를 승인 예외로 무시하지 않았다.

소스·seed·기준을 바꾸지 않고 같은 STOCK_COPILOT_PROPTEST_CASES=4096으로 커버리지만 재검사했다. 누적 프로파일을 배제하려고 cargo-llvm-cov의 profraw 정리 후 새 실행을 추가 확인했다. 최종 새 실행은955/955·coverage-check 위반0이며, LCOV270행4096회/273행6회/275행4090회다. 기존 테스트나 커버리지 기준, 속성 횟수를 수정하지 않았다. tick_size.rs는 master와 차이가 없다. 난수 조건에 따라 테스트 본체의 줄 커버리지가 달라질 수 있는 기존 재현성 문제는 메인/하네스 후속으로 넘긴다. 임의 seed를 고르거나 실패 분기를 제외하지 않았다.

정상 훅은 최초 커버리지 실패로 중단됐으므로, 실패한 커버리지의 새 프로파일 재검증과 그 뒤 test-diff·disable-report·disable-report-since·hooks-installed를 각각 실행했다. 마지막 test-diff는 승인된 호출부·헬퍼·필수 impl·Order 소진 분기의15건만 거부했고, 그 외 검사는 모두 통과했다. 최종 커밋은 이15건에 대한 원 지시문의 사전 승인에 따라 LEFTHOOK=0으로 만들며, 최초 실패와 재검증을 숨기지 않고 커밋 메시지에 남긴다. 전체 check-fast 명령 자체가 종료0이었다고 주장하지 않는다.
