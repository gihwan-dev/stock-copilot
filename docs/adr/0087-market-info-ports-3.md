# ADR 0087: A-21 읽기 포트 3: 섹터·투자자별 랭킹 조회

- 상태: 확정 (2026-11-16)

- 기준 SHA: 735ea38189edbf15d34238dd14ac49bb02c9e94f

## 결정

A-01·A-20의 Send + Sync, RPITIT Send Future, Result<T, PortError> 및 제네릭 주입을 따른다. domain의 D-37·D-41 타입을 그대로 반환하며 새 domain 타입·계산은 없다. SectorRankingQuery는 A-20의 RankingQuery와 같은 application 요청 값이다. 기존 domain 열거형으로 두 종류(TopGainers·MarketTradingAmount)와 여섯 기간을 요청 전에 검증하고 나머지는 InvalidRequest(UnsupportedQuery)로 거부한다. 타입에 없는 서버 기간·시각을 추정하지 않는다.

DataAvailability는 선택 키가 있는 외부 정보 포트의 NotConfigured·NoData 표현이다. 토스 MarketInfo는 A-01·A-20대로 봉인된 성공 값과 PortError를 유지한다. 빈 성공 목록과 NotFound(Sector)를 구별하며 실패를 빈 목록/0으로 바꾸지 않는다. 중복 DataAvailability 래퍼는 추가하지 않는다.

BrokerCapabilities에 각 새 메서드와 일대일인 기능 다섯 개를 추가한다. 미지원 호출은 해당 Unsupported(capability)이며 어댑터가 HTTP 전에 확인한다. trait 선언이나 로컬 가짜 테스트가 실제 공급자의 준수를 보증하지 않는다.

## 계약 표

모든 출력은 Result<출력, PortError>의 성공 값이다. 공통 오류 E는 ADR 0063대로 AccessDenied→UserAction, Unavailable→Wait(원 RetryDelay), InvalidRequest/InvalidResponse/Unsupported→Bug, NotFound→NoData다. 아래는 A-20의 24개, A-21의 다섯 개와 D-44의 네 개를 합친 전체 표다.

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
| MarketInfo | investor_trading_rankings | limit: RankingLimit | InvestorTradingRankings | 외국인·기관·개인별 순매수/순매도, 그룹별 rankedAt·수집 시각·TopN·순위 빈자리 보존 | E | T-10 |
| MarketInfo | sectors | 없음 | SectorCatalog | 모든 계층의 부모 관계·서버 순서, 서버 시각 Unknown·수집 시각. 대분류 선택은 소비자 | E | T-09 |
| MarketInfo | sector | sector: &SectorId, market: KnownMarket | SectorDetail | 요청 ID·시장, 여섯 기간·nullable 등락률·요약·서버 시각 Unknown·수집 시각 | E | T-09 |
| MarketInfo | sector_rankings | query: &SectorRankingQuery | SectorRanking | 요청 종류·시장·기간, 대분류/중분류·TopN·순위 빈자리·nullable 금액/등락률·수집 시각 | E | T-10 |
| MarketInfo | sector_etfs | sector: &SectorId, market: KnownMarket | SectorEtfs | 요청 ID·시장, 관련 ETF 서버 순서, 하위 섹터 미포함·서버 시각 Unknown·수집 시각 | E | T-09 |

| MarketInfo | program_trades | instrument: &QuoteInstrument, from: CalendarDate, through: CalendarDate | Vec<ProgramTrade> | 국내 종목·포함 날짜 창·일간 단위·기준일·수집 시각 결합. 서버 시각 Unknown, KRX만 | E | T-10 |
| MarketInfo | short_selling | instrument: &QuoteInstrument, from: CalendarDate, through: CalendarDate | Vec<ShortSelling> | 국내 종목·포함 날짜 창·일간 단위·기준일·수집 시각 결합. updatedAt 보존 | E | T-10 |
| MarketInfo | credit_trades | instrument: &QuoteInstrument, from: CalendarDate, through: CalendarDate | Vec<CreditTrade> | 국내 종목·포함 날짜 창·일간 단위·기준일·수집 시각 결합. updatedAt 보존 | E | T-10 |
| MarketInfo | securities_lending | instrument: &QuoteInstrument, from: CalendarDate, through: CalendarDate | Vec<SecuritiesLending> | 국내 종목·포함 날짜 창·일간 단위·기준일·수집 시각 결합. updatedAt 보존 | E | T-10 |

## 어댑터 의무

기존 24개 메서드는 ADR 0082의 결합·페이지·오류 의무를 유지한다. 새 다섯 메서드는 다음을 추가한다.

- T-10: investor_trading_rankings는 국내 투자자 랭킹의 count만 받는다. 임의 시장·기간 인자를 발명하지 않는다. 세 투자자 그룹과 각 매수/매도 목록, 그룹별 독립 rankedAt을 같은 응답에서 보존한다. count보다 짧은 목록·빈 목록·순위 공백을 완전한 전체 시장으로 부르지 않는다. 금액은 KRW이며 0으로 누락을 보충하지 않는다.
- T-09: sectors는 전체 계층 목록이다. 누락/검증 실패 행을 버리고 완전한 목록으로 만들지 않는다. 사용자에게 보이는 섹터는 ADR 0085의 Major이며 필터링·소속 결정은 D-40 소비자가 한다. sector는 응답 ID와 요청 ID·시장을 결합하고 여섯 기간 및 nullable 값을 보존한다. 없는 섹터는 NotFound(Sector), 식별 불일치는 InvalidResponse(Validation)다.
- T-10: sector_rankings는 대분류/중분류만 대상이다. depth가 없는 랭킹 응답에 임의 계층을 붙이지 말고 T-09 목록과 대조한다. 타입과 서버 응답의 종류·시장·기간을 맞추며 null을 0으로 바꾸지 않는다. 금액은 KRW(미국도 환산값)이며 순위/금액 비교는 domain이 한다.
- T-09: sector_etfs는 관련 ETF 목록이며 ETF 내부 보유 자산 분해가 아니다. 요청 섹터·시장, KRX/NXT 포함, 하위 섹터 미포함을 유지한다. 직전 분기 시총 순서라는 서버 의미를 보존하고 분기 날짜·시총을 발명하지 않는다. 심볼과 마스터 연결은 어댑터 책임이다.
- T-09: sector·sector_etfs 및 기존 sector_composition의 SectorId는 domain에서 출력 가능한 ASCII 최대 64바이트를 허용하므로 HTTP 경계의 더 좁은 제약을 대신하지 않는다. 요청 전에 `^[A-Za-z0-9_-]{1,40}$`를 검증하고, 불일치하면 해당 ReadEndpoint의 InvalidRequest(MalformedRequest)로 거부하여 HTTP를 보내지 않는다. `../accounts`, 쿼리/fragment 문자, 41~64바이트 입력도 거부한다. 검증된 ID만 URL 경로 세그먼트 API로 넣고 원문 문자열로 경로·쿼리를 조립하지 않는다. 이 작업은 기존 domain 식별자의 의미나 공개 생성자를 바꾸지 않는다.
- T-10: sector_rankings의 각 ID를 유효한 SectorCatalog의 id/depth와 대조한다. 목록에 없는 ID 또는 Minor 응답은 SectorRankings의 InvalidResponse(Validation)로 전체 실패하며 행을 버리거나 Major로 추정하지 않는다. 대조용 목록 조회가 실패하면 원 status/kind·RetryDelay를 보존하여 SectorRankings에 귀속한 바깥 Err를 반환하고 검증되지 않은 성공 랭킹을 내지 않는다. Major/Middle의 원 랭킹은 그대로 반환하며 새 depth 래퍼를 만들지 않는다. A-09/G-07/M-04는 기존 ID 접근자와 SectorCatalog를 결합해 Major만 사용자 섹터로 표시한다. 소비자의 목록 조회가 실패하면 섹터 표시를 보류하며 Middle을 Major로 표시하거나 순위를 재부여하지 않는다.
- T-10/A-09/G-07/M-04: 스키마 설명은 자료 없는 섹터의 changeRate를 서버가 0으로 줄 수 있다고 한다. 받은 0은 그대로 보존하되 실제 보합이나 자료 존재의 증거로 단정하지 않는다. null과 서버 0을 서로 바꾸거나, 추가 근거 없이 0을 NoData로 복원하지 않는다.
- 모든 새 응답의 수집 시각은 Clock에서 받는다. 서버 시각 없는 섹터 API는 Unknown을 유지한다. 허용 신선도와 오래됨 표시는 A-09/G-07/M-04 소비자가 결정한다. 실패한 페이지/행을 누락시키거나 완전 성공으로 가장하지 않는다.

## 정의 대기 해소 (D-44)

| 행 | 필요한 봉인 타입 | 받는 작업 |
|---|---|---|
| 프로그램 수급 | ProgramTrade로 정의 완료, program_trades 계약 참조 | D-44 → T-10 |
| 공매도 수급 | ShortSelling로 정의 완료, short_selling 계약 참조 | D-44 → T-10 |
| 신용 수급 | CreditTrade로 정의 완료, credit_trades 계약 참조 | D-44 → T-10 |
| 대차 수급 | SecuritiesLending로 정의 완료, securities_lending 계약 참조 | D-44 → T-10 |

## 인계

- A-21 채택 담당 메인: 초안 번호 부여와 함께 architecture 3.3의 포트 역할/메서드 및 ADR 0082의 정의 대기 투자자·섹터·ETF 행을 갱신한다. 구현자는 기존 확정 ADR을 수정하지 않는다.
- A-04: testkit 가짜에는 이번 다섯 메서드와 capability를 반영한다. 이 작업은 testkit을 수정하지 않는다.
- T-09/T-10: 위 계약의 실제 HTTP·응답 식별·완전성·capability 검사를 구현한다. 로컬 계약 검증과 실제 API 실측을 구분한다.
- A-09/G-07/M-04: freshness·TopN·Unknown 및 섹터 대분류 의미를 표시/후보 선택에 사용한다.
- A-15/E-08: 지수 벤치마크는 ADR 0085의 봉인 경로를 유지한다. 관련 ETF를 S&P500 추종 인증이나 ETF 보유 자산으로 해석하지 않는다.

## 검증

프로젝트 파일 hooks=true 및 Git 훅 설치 확인. 프로젝트 신뢰와 세션 훅 런타임 적용은 확인할 수 없으며 통과를 주장하지 않는다. 홈 설정/자격 증명을 읽거나 수정하지 않았다. 기존 tests/ports.rs의 FakeBroker에 다섯 필수 impl을 추가했으며 기존 단언·테스트 함수는 변경하지 않았다. 외부 의존성·검사 끄기 추가 없음.

## 리뷰 반영

독립 Opus 리뷰와 Codex 비판을 거쳐 HTTP 경계의 식별자 검증, 계층 대조 실패/소비자 표시, 서버 0의 의미를 위 의무에 명시했다. 서로 다른 섹터 ID 두 개와 두 시장의 상세/관련 ETF 반환 식별을 테스트에 더했다. ADR 직접 읽기 및 무번호 초안 관련 지적은 implementer 규칙과 구체 승인에 따라 기각했다. 테스트의 기대 서명은 문서 자동 파서가 아니며, 29개 계약 행은 별도 대조로 확인한다.

## 손 변형 실제 실행

최종 소스의 분기·허용 경계·반환 필드·오류를 임시 복사본에서 바꿔 27개를 각각 실제 컴파일·실행했다. 별도 빌드 디렉터리를 사용했고 매 로그에 application 재컴파일을 확인했다. 27개 모두 컴파일 성공 후 검출됐다. 원본으로 application 산출물을 지운 뒤 같은 테스트를 재컴파일하여 기준 통과도 확인했다. 임시 복사본 제거 완료. 앞선 matches! 구현과 공유 빌드 디렉터리 실험은 최종 집계에서 제외한다. 개별 로그는 target/A-21/final-hand-NN.log, 집계는 final-hand-results.json, 기준은 isolated-baseline.log다.

| 번호 | 바꾼 것 | 잡은 테스트 |
|---|---|---|
| 1 | TopGainers / MarketTradingVolume 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 2 | TopGainers / TossTradingAmount 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 3 | TopGainers / TossTradingVolume 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 4 | TopGainers / TopLosers 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 5 | TopGainers / Unknown 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 6 | MarketTradingAmount / MarketTradingVolume 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 7 | MarketTradingAmount / TossTradingAmount 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 8 | MarketTradingAmount / TossTradingVolume 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 9 | MarketTradingAmount / TopLosers 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 10 | MarketTradingAmount / Unknown 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 11 | Day / Realtime 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 12 | Day / Unknown 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 13 | Week / Realtime 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 14 | Week / Unknown 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 15 | Month / Realtime 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 16 | Month / Unknown 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 17 | ThreeMonths / Realtime 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 18 | ThreeMonths / Unknown 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 19 | SixMonths / Realtime 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 20 | SixMonths / Unknown 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 21 | Year / Realtime 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 22 | Year / Unknown 허용/거절 교환 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 23 | 종류 반환 고정 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 24 | 기간 반환 고정 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 25 | 오류 endpoint 바꿈 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 26 | 오류 status 바꿈 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |
| 27 | 거절 원인 바꿈 | sector_query_accepts_exactly_two_kinds_and_six_periods_and_keeps_identity |

추가 가짜 검증은 원본 제품 소스의 위 27개 손 변형과 별도다. 새 두 시장·두 ID 테스트는 상세/관련 ETF 가짜의 시장 KR 고정과 ID 고정 4개를 실제 컴파일 후 모두 검출했고 복구 대조군은 통과했다. target/A-21/fake-probe-results.json과 개별 로그에 기록했다.
