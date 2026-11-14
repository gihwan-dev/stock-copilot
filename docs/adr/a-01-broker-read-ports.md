# A-01 초안: 엔드포인트별 조회 사실과 닫힌 오류

- 상태: 적대적 리뷰 수정안. architecture 3.3 반영은 메인 리뷰 뒤 한다.
- 최초 구현: 9c1ba39d547e6484b68d8d988d54a95d23860d6c
- 최초 병합 기준: 1cd405c9f7ff11be0a26d1eb892766463aaf722a. 이번 회차 끝에 로컬 master f9f0c5f0b7dff04ad4a184aadf882f4af1fee376(D-22 포함)을 병합했다.
- 범위: application과 이 ADR. domain·tasks·progress는 수정하지 않는다.

## 현재 정의된 계약

출력은 모두 Result<출력, PortError>다. BrokerRead·MarketInfo는 Send + Sync이며 표준 RPITIT의 Send Future를 반환한다. 외부 런타임·의존성·박스 할당을 추가하지 않는다. dyn 호환성은 제공하지 않고 조립 루트가 제네릭으로 주입한다. 페이지·재시도·속도 제한은 어댑터 내부다. 거래 기간 분할과 시세 200개 배치 분할은 호출자 책임이다.

| 포트       | 메서드         | 입력                                                                  | 출력                    | 소비자                 | 공급자·GET 엔드포인트        | 상태·근거                                                                                                          |
| ---------- | -------------- | --------------------------------------------------------------------- | ----------------------- | ---------------------- | ---------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| BrokerRead | accounts       | 없음                                                                  | Vec<AccountObservation> | U-22, O-04, O-07       | T-05 · /api/v1/accounts      | 임시 최소 행: 계좌 키와 봉인된 AccountDisplayFact를 같은 호출에서 반환. 계좌 유형과 표시 접근자는 domain 정의 대기 |
| BrokerRead | holdings       | &AccountId                                                            | HoldingsObservation     | A-06, A-08, U-09       | T-05 · /api/v1/holdings      | Vec<Holding>과 ReportedPortfolio만 보존. 매수가능금액 조회나 Portfolio 조립을 하지 않음                            |
| BrokerRead | transactions   | &AccountId, SettlementWindow, &TransactionFilter                      | TransactionRead         | A-07, A-16, D-30       | T-06 · /api/v1/transactions  | Complete(CompleteTransactionResponse) 또는 Incomplete(IncompleteTransactions). 아래 보존·분할 계약 적용            |
| BrokerRead | quotes         | &[ResolvedOrderInstrument]                                            | Vec<QuoteObservation>   | A-14, O-01, O-04, U-13 | T-07 · /api/v1/prices        | 입력 임시: 종목 마스터가 주문 식별·시계열 ID를 함께 확정할 때 교체. 종목별 Result로 일부 실패 보존                 |
| BrokerRead | candles        | &ResolvedOrderInstrument, TimeInterval, CandlePeriod, PriceAdjustment | Vec<Candle>             | A-15, A-16, J-12       | T-07 · /api/v1/candles       | 입력 임시: 기존 타입 하나만 사용. 어긋날 수 있는 종목 쌍 튜플은 제거. 안정적 ID 연결은 T-09 사실이 필요            |
| BrokerRead | buying_power   | &AccountId, Currency                                                  | BuyingPower             | A-06, A-08, O-01, O-04 | T-11 · /api/v1/buying-power  | 통화별 독립 조회. observed_at과 원래 금액 오류 보존                                                                |
| MarketInfo | fx_rate        | FxPair, FxRateKind, Option<Timestamp>                                 | FxRate                  | A-08, A-16             | T-08 · /api/v1/exchange-rate | None은 최신, Some은 dateTime. 종류·출처·유효 기간 보존                                                             |
| MarketInfo | order_fx_rates | 없음                                                                  | OrderFxRates            | O-04, O-09             | T-08 · /api/v1/exchange-rate | 같은 응답의 rate·midRate를 함께 반환. 개별 두 호출을 같은 응답으로 가장하지 않음                                   |

accounts는 서버 순서를 유지한다. AccountObservation에는 기존 AccountId와 AccountDisplayFact만 있으며, 계좌 유형이 확인되었다고 주장하지 않는다. 현재 생성자는 두 사실을 받는다. 같은 행 결합은 T-05 책임이며, display의 계좌 키를 읽는 공개 domain 접근자가 없어 여기서 일치를 인증할 수 없다. 계좌 유형·전체 계좌 행 계약이 들어오기 전 U-22의 종합매매 계좌 선택을 완성한 것으로 보지 않는다. 별도 account_display 메서드는 제거했다. 전체 계좌번호는 포트에 없다.

holdings의 Holding 안에 HoldingObservation에서 온 현재가·토스 제공값·관측 시각이 유지된다. 토스 요약도 ReportedPortfolio로 보존하며 application은 계산하지 않는다. Portfolio 조립과 일부 실패 정책은 A-06 몫이다. 현재 Portfolio::new는 두 통화 BuyingPower를 필수로 받고, BuyingPower의 실패는 음수 금액 오류뿐이다. 네트워크 조회 실패를 표현하는 domain 입력은 없다. 0원·음수 가짜 값으로 대체하지 않는다. 아래 D-05 후속이 들어와야 조회 실패 현금을 포함한 Portfolio를 조립할 수 있다. 보유 성공과 현금 실패는 지금도 별도 포트 결과로 함께 유지할 수 있다.

quotes는 한 호출 1~200개다. 빈 목록·201개 이상은 HTTP 전에 InvalidRequest(BatchLimitExceeded)이며 자동 분할하지 않는다. 호출자가 200개씩 나눈다. 반환은 입력과 같은 순서·개수이며 중복 입력도 각 슬롯을 유지한다. 각 QuoteObservation은 요청 종목과 Result<Quote, PortError>를 가진다. 일부 종목의 데이터 없음·검증 실패를 그 종목 오류로 반환하고 성공 종목은 보존한다. 전송 실패나 응답 전체를 해석할 수 없는 경우만 바깥 Err다. 상한·슬롯 대응은 T-07 구현과 계약 테스트가 강제해야 하며 트레이트 선언 자체가 이를 실행하지 않는다.

candles는 요청 Symbol로 조회하고 T-09의 종목 마스터 사실로 InstrumentId를 확인해 domain Candle을 만든다. Symbol에서 안정적 ID를 추정하거나 새로 발명하지 않는다. 확인할 수 없으면 InvalidResponse(Validation)다. 마스터 결합 타입 이전의 임시 입력이라는 제약을 소비자가 알아야 한다. [start,end)의 timestamp 범위를 과거순으로 반환하고, 수정 기준·확정 여부·거래일 누락은 ADR 0035대로 T-07이 검증한다.

## 완전·불완전 거래 응답

TransactionRead::Complete만 ADR 0034의 CompleteTransactionResponse를 가진다. 마지막 페이지·필터 일관성·해당 종류의 전체 미결제·격리 없음이 모두 확인되어야 한다. Incomplete는 이 증거를 가지지 않으며 reconcile·LedgerEvidence에 직접 넣을 수 없다. D-30 증거 경계를 넓히지 않는다.

기존 D-08·D-30에는 불완전 응답과 미지 통화 원문의 보존 타입이 없어 application의 최소 결과를 둔다. IncompleteTransactions는 요청 계좌·결제 창·필터, 받은 유효 행의 Vec<TransactionBatch>, 격리 행, 닫힌 이유를 보존한다. 세 이유는 QuarantinedRows / RowLimitExceeded / PageInterrupted다. 여러 이유가 겹치면 RowLimitExceeded, PageInterrupted, QuarantinedRows 순으로 종료 원인을 정하되 격리 정보와 받은 행은 모두 유지한다. 아무 자료도 받지 못한 실패만 바깥 PortError로 반환한다.

QuarantinedTransaction::UnknownCurrency는 Vec<domain::UntrustedText>에 원래 거래 행의 UTF-8 원문을 순서대로 보존한다. 이 domain 타입은 변환·정규화 없이 최대 65,536바이트를 보존하고 Debug에는 길이만 보인다. 긴 행은 어댑터가 UTF-8 경계에서 조각내며 이어 붙이면 원문과 정확히 같아야 한다. 잘라 버리거나 Currency를 KRW/USD로 치환하지 않는다. 본문은 정상 조회 결과의 격리 값에만 있고 PortError에는 들어가지 않는다. 전체 응답·헤더·키를 복사하지 않는다. A-07은 격리 행을 보존하고 사용자에게 알리되 합계·사라짐 대조에는 쓰지 않는다.

완전 응답은 D-08의 4,096행 상한을 지킨다. 상한을 넘는 기간은 호출자 A-07/A-16이 결제일 창으로 나눈다. 어댑터는 몰래 기간을 나누어 완전 응답을 합성하지 않는다. 4,097번째를 발견하면 받은 행을 4,096개 이하의 TransactionBatch 여러 개로 보존하고 RowLimitExceeded를 반환한다. 아직 읽지 않은 페이지가 있음을 숨기지 않는다. 한 결제일·전체 미결제 자체가 상한을 넘으면 더 나누어 완전성을 만들었다고 주장하지 않으며 불완전 상태로 보류한다. 저장된 이력의 100만 행 상한과 조회 응답 4,096행 상한은 별개다.

## 정의 대기와 메인이 등록할 선행 작업

계좌·주문·종목의 읽기는 BrokerRead, 시장 전체 사실은 MarketInfo에 둔다. architecture 3.1의 broker-toss가 두 포트를 구현한다는 근거를 따른다. 아래는 메서드 구현이 아니며 domain 타입 이름도 초안이다. 메인이 선행 D 작업 → 후속 포트 작업 → T 계약 테스트 순으로 tasks에 등록한다.

| 포트               | 정의 대기 읽기·domain 타입 이름 초안                                        | 엔드포인트                                                                                                                                    | 소비자                                 | 공급자·선행 책임                                                                                                                                                      |
| ------------------ | --------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| BrokerRead         | 매도가능수량 · SellableQuantityFact                                         | /api/v1/sellable-quantity                                                                                                                     | O-01, D-26, O-04                       | T-11; 계좌·종목·수량·관측 시각을 봉인하는 D 작업                                                                                                                      |
| BrokerRead         | 미체결·주문 목록/상세 · OrderQuery, BrokerOrder, BrokerOrderSet             | /api/v1/orders, /api/v1/orders/{orderId}                                                                                                      | O-01, O-04, O-11, O-19, O-25           | T-15; 상태·주문 키·목록 완전성 사실 D 작업                                                                                                                            |
| BrokerRead         | 매수 유의사항 · StockWarnings                                               | /api/v1/stocks/{symbol}/warnings                                                                                                              | O-01, O-04                             | T-11; 닫힌 알려진 경고와 미지 경고 원문·시각 D 작업                                                                                                                   |
| BrokerRead         | 종목 마스터 · MasterInstrument, MasterInstrumentFacts                       | /api/v1/stocks, /api/v1/stocks/all                                                                                                            | O-01, D-12, D-26, U-29, quotes·candles | T-09; ResolvedOrderInstrument·InstrumentId·종목명·유형을 같은 조회 사실에 결합하는 D 작업. PolicySecurityFacts 공개 생성 경계도 연결                                  |
| BrokerRead         | 호가 · Orderbook, OrderbookLevel                                            | /api/v1/orderbook                                                                                                                             | D-26, O-04, A-14                       | T-07/D-26 연결; 가격·잔량·시장·시각 D 작업                                                                                                                            |
| BrokerRead         | 상하한가 · PriceLimits                                                      | /api/v1/price-limits                                                                                                                          | D-26, O-01, O-04                       | T-07/D-26 연결; 호가 단위 규칙과 상하한가 사실을 혼동하지 않는 D 작업                                                                                                 |
| BrokerRead         | 수수료율 · CommissionRateFact                                               | /api/v1/commissions                                                                                                                           | D-22, O-04, A-14                       | T-11; 병합한 D-22에 CommissionRateFact·CommissionValidity와 estimate_order_fee 공개 생산 경로가 이미 있다. 이 행은 추가 D 타입이 아니라 후속 포트·T-11 계약 정의 대기 |
| MarketInfo         | 지수 현재가·봉 · MarketIndicatorQuote, IndicatorUnit, MarketIndicatorCandle | /api/v1/market-indicators/prices, /api/v1/market-indicators/{symbol}/candles                                                                  | A-09, D-09, U-13, M-04                 | T-08; 포인트 단위·지표 식별·09:00 봉 시각·출처·관측 시각 D 작업. Money/Quote로 위장하지 않음                                                                          |
| BrokerRead         | 종목 랭킹 · RankingQuery, StockRanking                                      | /api/v1/rankings, /api/v1/rankings/investor-trading                                                                                           | G-07, M-04                             | T-10; 순위·기간·rankedAt과 종목 사실 D 작업                                                                                                                           |
| BrokerRead         | 종목별 수급 · InvestorTradingSeries, StockFlowSeries                        | /api/v1/stocks/{symbol}/investor-trading, /program-trades, /short-selling, /credit-trades, /securities-lending (앞 경로와 같은 symbol 접두사) | G-07, M-04                             | T-10; 수급 종류·기간·단위·관측 시각 D 작업                                                                                                                            |
| MarketInfo         | 지표 수급 · IndicatorInvestorFlow                                           | /api/v1/market-indicators/{symbol}/investor-trading                                                                                           | M-04, A-09                             | T-08·T-10 연결; 지표 단위·집계 사실 D 작업                                                                                                                            |
| MarketInfo         | 섹터 목록·구성·랭킹 · SectorCatalog, SectorComposition, SectorRanking       | /api/v1/sectors, /api/v1/sectors/rankings, /api/v1/sectors/{sectorId}, /api/v1/sectors/{sectorId}/stocks, /api/v1/sectors/{sectorId}/etfs     | G-07, M-04, D-12                       | T-09·T-10; 분류 출처·구성·기준 시각 D 작업                                                                                                                            |
| BrokerRead         | 계좌 행 완성 · BrokerAccountRow, BrokerAccountKind                          | /api/v1/accounts                                                                                                                              | U-22, O-04, O-07                       | T-05; 키·닫힌 유형·표시 사실 결합과 읽기 접근자 D 작업. 임시 AccountObservation 대체                                                                                  |
| MarketInfo         | calendar · KnownMarket(Kr/Us), CalendarQuery                                | /api/v1/market-calendar/KR, /api/v1/market-calendar/US                                                                                        | J-05, A-14, O-01                       | T-08; 미지 시장을 만들 수 없는 domain 입력 D 작업. 기존 Market::Unknown을 받던 메서드는 제거하고 정의 대기                                                            |
| A-06의 domain 입력 | 조회 실패 현금 · CashObservation, CashObservationFailure                    | buying-power의 실패를 소비, 새 HTTP 없음                                                                                                      | A-06, A-08                             | D-05 후속: 네트워크 실패·미조회와 음수 금액 오류 구별. Portfolio 조립 입력을 확장                                                                                     |

최근 틱 체결 /api/v1/trades는 이번 소비자 계약에서 필요성을 확정하지 않았다. 조건주문 GET 두 개(/api/v1/conditional-orders, /api/v1/conditional-orders/{conditionalOrderId})도 현재 O 작업이 조건주문을 소비하지 않으므로 호출 메서드를 만들지 않는다. OpenAPI에 있다는 이유만으로 새 소비자를 발명하지 않는다. 이들은 조회 누락을 숨기기 위한 역할 제외가 아니라 현재 소비자 미확정 항목이다.

## PortError와 A-05 인계

PortError는 비공개 ReadEndpoint·HttpStatus·PortErrorKind를 가진다. ReadEndpoint는 현재 여덟 포트 메서드 단위이며 토큰 발급 중 오류도 그 조회를 시작한 메서드에 귀속한다. HttpStatus는 NotReceived, 1xx/2xx/3xx, 주요 4xx 개별 상태와 OtherClientError, 5xx ServerError의 닫힌 분류다. 원시 숫자나 임의 문자열은 받지 않는다. RetryDelay는 Unknown / Known(domain::TimeSpan)이다. 정확한 HTTP 숫자가 필요한 후속 계약은 별도 검토한다.

| 닫힌 종류       | 하위 종류                                                                                                                       | A-05 세 분류 초안                  | 소비자 근거                                                                                                           |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| AccessDenied    | InvalidCredentials, IpNotAllowed, TokenRevokedRepeated, AccountNotFound, Forbidden, LoginUserNotFound                           | 사용자 조치                        | design 2.6의 키·IP·중복 키 문장을 구별. A-19가 원인별 문장을 고름                                                     |
| Unavailable     | RateLimited, Maintenance, ServerError, Network, ExpiredToken + RetryDelay                                                       | 대기                               | 네트워크·점검·한도·토큰 갱신을 사용자 키 오류로 바꾸지 않음                                                           |
| NotFound        | Stock, ExchangeRate, Sector                                                                                                     | 사용자 조치(대상·기간의 자료 없음) | 조회 대상 결과의 부재이며 연결 자체 실패로 승격하지 않음. A-19 상태 띠 정책과 구분                                    |
| InvalidResponse | Schema, Validation                                                                                                              | 버그                               | 응답 계약·domain 생성 실패. 원문은 오류 값에 없음                                                                     |
| Unsupported     | BrokerCapability                                                                                                                | 버그                               | 선언된 지원 집합을 확인하지 않고 지원 밖 조회를 요청한 호출 계약 위반. 지원 없음은 별도 CapabilitySupport로 정상 처리 |
| InvalidRequest  | MalformedRequest, MissingAccountHeader, InvalidToken, EdgeBlocked, UnsupportedQuery, UnsupportedContentType, BatchLimitExceeded | 버그                               | 잘못 만든 요청·상한 위반·토큰 갱신 후에도 남는 요청 문제                                                              |

분류 함수는 A-05가 만든다. 위 표는 그 함수의 입력 구분을 유지하기 위한 인계이며 UI 문구나 자동 재시도 구현이 아니다. 오류의 HTTP 상태와 종류가 모순되게 매핑되지 않도록 T-04가 계약 테스트한다. 생성자는 닫힌 필드만 허용하지만 서버 사실의 진위를 인증하지 않는다.

### R-01 Q7 전체 오류표 대응

아래는 로컬 R-01 Q7 전체 행이다. 주문 생성·정정·취소 전용 오류는 조회 PortError로 억지 변환하지 않고 BrokerTrade/O 작업에 인계한다. 그 행에도 A-05 분류를 적어 누락과 상태 처리를 구별한다. 주문의 Unknown은 오류가 아닌 상태라는 architecture 3.8을 유지한다.

| HTTP·R-01 코드                                                                             | 조회 PortError 대응 또는 후속 주문 책임                     | A-05 분류·비고                                                                                 |
| ------------------------------------------------------------------------------------------ | ----------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| 400 invalid-request                                                                        | InvalidRequest(MalformedRequest)                            | 버그. 커서 오류 재시작은 T-06 내부. 부분 수신이면 Incomplete(PageInterrupted)로 자료 유지      |
| 400 confirm-high-value-required                                                            | O-09/BrokerTrade의 확인 거절                                | 사용자 조치. 조회 오류 변형을 추가하지 않음                                                    |
| 400 account-header-required                                                                | InvalidRequest(MissingAccountHeader)                        | 버그                                                                                           |
| 400 unsupported-ranking-duration, unsupported-symbol, unsupported-market, unsupported-date | InvalidRequest(UnsupportedQuery)                            | 버그. domain 쿼리/기능 미지원은 요청 전 Unsupported로 구분                                     |
| 400 us-modify-quantity-not-supported                                                       | 후속 주문 요청 오류                                         | 버그. 정정은 현재 제품 범위 밖                                                                 |
| 401 invalid-token                                                                          | InvalidRequest(InvalidToken), T-02가 먼저 재발급 1회 처리   | 버그 가능성. 반복 token-revoked와 합치지 않음                                                  |
| 401 expired-token                                                                          | Unavailable(ExpiredToken, RetryDelay)                       | 대기. T-02 갱신 후에도 반환할 때 구분                                                          |
| 401 token-revoked                                                                          | T-02 재발급 1회, 반복 때 AccessDenied(TokenRevokedRepeated) | 사용자 조치. 최초 재발급 시도와 반복 중단 구별                                                 |
| 401 login-user-not-found                                                                   | AccessDenied(LoginUserNotFound)                             | 사용자 조치                                                                                    |
| 401 edge-blocked                                                                           | InvalidRequest(EdgeBlocked) + Unauthorized                  | 버그                                                                                           |
| 401 토큰 invalid_client                                                                    | AccessDenied(InvalidCredentials)                            | 사용자 조치, design 2.6 순서 1                                                                 |
| 400 토큰 invalid_request 등 OAuth 요청 오류                                                | InvalidRequest(MalformedRequest)                            | 버그. error_description은 보관하지 않음                                                        |
| 403 ip-not-allowed / 토큰 access_denied                                                    | AccessDenied(IpNotAllowed)                                  | 사용자 조치, 순서 2                                                                            |
| 403 forbidden                                                                              | AccessDenied(Forbidden)                                     | 사용자 조치. 원인 자유문장을 저장하지 않음                                                     |
| 403 edge-blocked                                                                           | InvalidRequest(EdgeBlocked) + Forbidden                     | 버그                                                                                           |
| 404 edge-blocked                                                                           | InvalidRequest(EdgeBlocked) + NotFound                      | 버그. 실제 데이터 없음과 구별                                                                  |
| 404 stock-not-found, exchange-rate-not-found, sector-not-found                             | NotFound(Stock / ExchangeRate / Sector)                     | 사용자 조치 범주의 자료 없음. 연결 점검 실패로 오인하지 않음                                   |
| 404 account-not-found                                                                      | AccessDenied(AccountNotFound)                               | 사용자 조치. 계좌 목록 재조회                                                                  |
| 404 order-not-found, conditional-order-not-found                                           | 정의 대기 주문 조회의 데이터 없음/상태 대조                 | 사용자 조치 또는 상태 변화. 조건주문 이미 반영됨은 상태로 인계, 현재 오류 함수로 분류하지 않음 |
| 409 request-in-progress                                                                    | 주문 키 처리 중 상태                                        | 대기. 조회 PortError로 주문 재전송을 결정하지 않음                                             |
| 409 opposite-pending-order-exists                                                          | O-01/O-19 주문 게이트 거절                                  | 사용자 조치                                                                                    |
| 409 already-filled, already-canceled, already-modified, already-rejected                   | 주문 대조 후 상태 변화                                      | 상태이므로 오류 세 분류로 강제하지 않음                                                        |
| 409 already-processing                                                                     | 후속 정정·취소 처리 중 상태                                 | 대기, data.retryAfterSeconds는 후속 주문 계약의 닫힌 대기 시간으로                             |
| 414 edge-blocked                                                                           | InvalidRequest(EdgeBlocked) + UriTooLong                    | 버그                                                                                           |
| 415 unsupported-content-type                                                               | InvalidRequest(UnsupportedContentType)                      | 버그                                                                                           |
| 422 insufficient-buying-power                                                              | 주문 게이트 거절                                            | 사용자 조치. 달러 환전은 design 2.6 순서 8                                                     |
| 422 order-hours-closed                                                                     | 주문 제안 만료/장 닫힘 상태                                 | 대기 분류지만 자동 주문 재전송은 안 함                                                         |
| 422 stock-restricted                                                                       | 주문 게이트 거절                                            | 사용자 조치                                                                                    |
| 422 price-out-of-range                                                                     | 주문 가격 거절                                              | 사용자 조치, D-26 가격 한계 재검증                                                             |
| 422 order-type-not-allowed                                                                 | 주문 유형 거절                                              | 사용자 조치                                                                                    |
| 422 prerequisite-required                                                                  | 주문 사전 요건 거절                                         | 사용자 조치, design 2.6 순서 8                                                                 |
| 422 market-not-supported-for-stock                                                         | 종목 마스터·주문 시장 불일치                                | 버그 또는 데이터 문제, O-01/T-09가 사실을 대조                                                 |
| 422 investor-exchange-not-integrated                                                       | 주문 사전 요건 거절                                         | 사용자 조치, design 2.6 순서 8                                                                 |
| 422 amount-order-outside-regular-hours, fractional-quantity-outside-regular-hours          | O-01 시간 정책 거절                                         | 사용자 조치. capability가 아님                                                                 |
| 422 modify-restricted, cancel-restricted                                                   | 후속 정정·취소 거절                                         | 사용자 조치, 현재 제품 범위 밖                                                                 |
| 422 insufficient-sellable-quantity                                                         | 주문 수량 거절                                              | 사용자 조치, D-26/O-01 재검증                                                                  |
| 422 order-limit-exceeded                                                                   | 주문 한도 거절                                              | 사용자 조치                                                                                    |
| 422 duplicate-conditional-order, condition-already-met                                     | 후속 조건주문 거절                                          | 사용자 조치, 현재 제품 범위 밖                                                                 |
| 422 idempotency-key-conflict                                                               | 주문 키 계약 위반                                           | 버그                                                                                           |
| 422 account-restricted                                                                     | 주문 계좌 제한                                              | 사용자 조치. 계좌 없음과 합치지 않는 후속 주문 종류 필요                                       |
| 422 max-order-amount-exceeded                                                              | 주문 금액 제한                                              | 사용자 조치                                                                                    |
| 429 rate-limit-exceeded, edge-rate-limit-exceeded                                          | Unavailable(RateLimited, RetryDelay) + RateLimited          | 대기. Retry-After가 없으면 Unknown이며 0초를 발명하지 않음                                     |
| 500 internal-error                                                                         | 조회: Unavailable(ServerError, RetryDelay)                  | 대기. 주문은 Unknown 상태·O-19 대조                                                            |
| 500 maintenance                                                                            | Unavailable(Maintenance, RetryDelay)                        | 대기. 주문 재전송 판단은 O-19 소유                                                             |
| HTTP 없음: 네트워크 단절·시간 초과                                                         | Unavailable(Network, RetryDelay) + NotReceived              | 대기. 요청·응답 본문 없음                                                                      |
| 스키마·domain 검증 실패 (추가 경계)                                                        | InvalidResponse(Schema / Validation)                        | 버그                                                                                           |
| capability 밖 요청 (추가 경계)                                                             | Unsupported(읽기 capability)                                | 버그인 호출 계약 위반. 정상 지원 판정은 CapabilitySupport                                      |

## 읽기 capability와 생성 경계

BrokerCapabilities는 DailyCandles / MinuteCandles / AdjustedHistory / RawHistory의 닫힌 지원 집합이다. support_for는 Supported / Unsupported를 반환한다. 테스트는 지원 두 개·미지원 두 개·빈 집합을 실제 판정한다. 주문 여섯 변형과 소수 매도 항목은 제거했다. 주문 지원 여부는 order-execution의 BrokerTrade 계약, 정규장 시간 제한은 O-01 정책이다.

| 현재 읽기 항목  | 공급자              | 소비자                |
| --------------- | ------------------- | --------------------- |
| DailyCandles    | T-07 interval=1d    | A-15, A-16            |
| MinuteCandles   | T-07 interval=1m    | A-15, J-12            |
| AdjustedHistory | T-07 adjusted=true  | A-15, A-16            |
| RawHistory      | T-07 adjusted=false | A-07/D-30 입출고 평가 |

정의 대기 읽기의 capability 후보는 종목 마스터·호가·가격 제한·수수료율·매도가능수량·주문 조회·매수 경고·랭킹·지표·섹터·장 캘린더다. domain 사실과 소비자 계약을 확정한 후 추가하며, 아직 호출할 메서드가 없는 지원 플래그를 먼저 배포하지 않는다.

new는 crate 밖 어댑터가 지원 집합을 만들어야 하므로 공개다. Rust 가시성으로 특정 외부 crate만 허용할 수 없다. 실제 운영 호출 위치를 broker 어댑터로 제한하는 하네스 작업을 인계한다. AccountObservation·HoldingsObservation 같은 임시 사실 봉투의 운영 생성자도 동일한 신뢰 경계 검토 대상이다. 공개 생성자는 IO 인증이나 주문 승인 권한이 아니다.

## 검증·하네스

새 의존성·허용 목록·crate 등록·기존 master 테스트·검사 끄기 변경은 없다. application 일반 조회는 파일별 수치 커버리지 대상이 아니며 order_gate의 100% 정책은 유지한다. 자동 변이는 사용자 지시로 실행하지 않는다. 잠금 파일 변경 시 전체 domain으로 넓어지는 하네스 동작은 우회하지 않는다.

가짜는 실제 Holding·ReportedPortfolio·BuyingPower·Quote·FxRate·완전 거래 응답을 만들며 보유 성공 후 독립 현금 실패, 완전/불완전 세 이유, 원문 조각 보존, 시세 배치의 성공·실패 혼합, 200/201/빈 입력, 모든 메서드의 바깥 오류, 읽기 지원 판정, 닫힌 오류와 대기 시간 보존을 확인한다. 이는 포트 컴파일 계약과 가짜의 대본 검증이다. 실제 어댑터의 배치 상한·원문 보존·반환 순서는 T 계약 테스트가 별도로 검증해야 한다.

테스트의 cfg(test) mod 감싸기를 제거한 실험에서 보호된 Cargo.toml:130의 forbid(clippy::tests_outside_test_module)와 하네스 3.3이 거부했다. mod만 남겨도 같은 오류와 테스트 도우미의 expect_used 오류가 났다. 규칙을 끄지 않고 필수 감싸기는 유지한다. 판정 의미 없는 오류 변형 간 assert_ne는 제거했다. 이번 브랜치에서 만든 테스트만 수정했으며 master의 테스트는 변경하지 않는다.

프로젝트 hooks=true는 확인했지만 세션 신뢰 적용은 추측하지 않는다. 직접 검사와 커밋 훅 전체 결과를 증거로 사용한다.

## architecture 3.3 문장 초안

현재 정의된 BrokerRead 메서드는 accounts, holdings, transactions, quotes, candles, buying_power이고 MarketInfo는 fx_rate, order_fx_rates다. holdings는 보유 관측·토스 요약만 반환하고 현금 조회·Portfolio 조립은 A-06에서 분리한다. 거래는 완전 증거 또는 자료를 보존한 불완전 결과, 시세는 최대 200개의 종목별 결과를 반환한다. 계좌 행과 quotes·candles의 종목 입력은 domain 선행 작업 전의 임시 계약이다. 오류는 닫힌 원인·엔드포인트·HTTP 상태·대기 시간만 담는다. 두 포트는 Send + Sync, 메서드는 Send Future이며 dyn을 지원하지 않는다. 조립 루트는 제네릭으로 주입한다. BrokerCapabilities는 읽기 기능만 나타내며 주문 지원·승인을 대신하지 않는다.

매도가능수량, 주문 조회, 매수 유의사항, 종목 마스터, 호가, 가격 제한, 수수료율, 지수, 랭킹·수급·섹터와 KnownMarket 기반 calendar는 정의 대기다. 선행 domain 사실 타입 뒤 BrokerRead(계좌·주문·종목) 또는 MarketInfo(시장 전체)에 메서드를 추가한다. 이 문장은 해당 읽기가 이미 구현되었거나 조회 포트가 완성되었다는 뜻이 아니다.
