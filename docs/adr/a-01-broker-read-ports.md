# A-01 초안: 증권사 조회 포트 계약

- 상태: 구현·리뷰 대상 초안. 메인이 리뷰 뒤 architecture 3.3에 반영한다.
- 최초 기준: ac1232699397ba824a201a9994dec7f44f3f0230
- 병합한 로컬 master: 1cd405c9f7ff11be0a26d1eb892766463aaf722a
- 범위: application. domain은 읽기 전용. BrokerTrade는 order-execution 소유다.

## 계약 유도

architecture 3.3은 역할만 정한다. 사용자 후속 지시에 따라 tasks의 소비자와 T 작업·로컬 OpenAPI 공급자를 함께 연결해 아래 계약을 유도한다. 원래 계약이 문서에 있었다고 주장하지 않는다. 모든 HTTP 경로는 GET이며 R-01 6장의 엔드포인트 표와 OpenAPI 1.2.28의 해당 paths를 대조했다. 외부 서버나 실제 키·계좌는 사용하지 않았다.

아래 출력은 모두 Result<출력, PortError>다. 입력에서 &는 차용이고, candle의 종목 쌍은 (&ResolvedOrderInstrument, &InstrumentId)다. Vec는 봉인된 domain 값의 목록일 뿐 원시 JSON·페이지가 아니다. 한 응답 타입이 없는 목록에는 기존 타입의 Vec를 사용한다. PortError는 AccessDenied / Unavailable / InvalidResponse의 데이터 없는 닫힌 열거형이다. 낮음 표지는 소비자가 한 작업뿐이거나 계약 조합·엔드포인트 대응에 검토가 더 필요한 행이다.

| 포트       | 메서드          | 입력                                                 | 출력(domain 타입)           | 오류      | 소비자 작업      | 공급자 작업·엔드포인트                                         | 근거 문장·확신                                                                                                                                   |
| ---------- | --------------- | ---------------------------------------------------- | --------------------------- | --------- | ---------------- | -------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| BrokerRead | accounts        | 없음                                                 | Vec<AccountId>              | PortError | U-22, A-06       | T-05 · /api/v1/accounts                                        | U-22는 첫 종합매매 계좌 자동 선택과 계좌 변경을 요구한다. API accountSeq를 도메인 키로 변환한다. 낮음: 직접 소비자 U-22, A-06의 계좌 공급은 유도 |
| BrokerRead | account_display | &AccountId                                           | AccountDisplayFact          | PortError | O-04, O-07       | T-05 · /api/v1/accounts                                        | 주문 요약은 계좌 뒤 네 자리다. ADR 0045대로 같은 계좌 응답 행의 키와 마스킹 번호를 결합한다                                                      |
| BrokerRead | portfolio       | &AccountId                                           | Portfolio                   | PortError | A-06, A-08, U-09 | T-05·T-11 · /api/v1/holdings + /api/v1/buying-power (KRW, USD) | A-06은 보유 동기화, A-08은 통화별 매수가능금액을 현금으로 사용한다. 낮음: 기존 Portfolio 생성에 두 통화 BuyingPower가 필요해 세 조회를 조합      |
| BrokerRead | transactions    | &AccountId, SettlementWindow, &TransactionFilter     | CompleteTransactionResponse | PortError | A-07, A-16       | T-06 · /api/v1/transactions                                    | 최근 14일 결제일 창·하루 전체 검사. ADR 0034의 마지막 페이지·필터·전체 미결제·격리 없음 확인을 증거 타입으로 반환                                |
| BrokerRead | quote           | &ResolvedOrderInstrument                             | Quote                       | PortError | A-14, O-01, O-04 | T-07 · /api/v1/prices                                          | 최신가로 제안하고 최신 시세로 재검증한다. 서버 nullable timestamp와 수집 시각을 유지                                                             |
| BrokerRead | candles         | 종목 쌍, TimeInterval, CandlePeriod, PriceAdjustment | Vec<Candle>                 | PortError | A-15, A-16, J-12 | T-07 · /api/v1/candles                                         | 판정은 수정 일봉·1분봉, 빠진 날은 일봉으로 재계산한다. 낮음: 안정적 시계열 ID와 요청 Symbol의 연결은 T-09 사실을 어댑터가 검증해야 함            |
| BrokerRead | buying_power    | &AccountId, Currency                                 | BuyingPower                 | PortError | O-01, O-04, A-08 | T-11 · /api/v1/buying-power                                    | 통화별 매수가능금액으로 주문을 재검증한다. 현금·예수금으로 재명명하지 않고 observed_at을 유지                                                    |
| MarketInfo | calendar        | &Market, CalendarDate                                | MarketCalendar              | PortError | J-05, A-14, O-01 | T-08 · /api/v1/market-calendar/KR 또는 /US                     | 개장 시각은 토스에서 읽는다. ADR 0028의 세 행 캘린더를 보존하고 누락을 휴장으로 바꾸지 않음                                                      |
| MarketInfo | fx_rate         | FxPair, FxRateKind, Option<Timestamp>                | FxRate                      | PortError | A-08, A-16       | T-08 · /api/v1/exchange-rate                                   | 원화 합산에 Mid와 출처·유효 시각이 필요하다. 과거는 dateTime, None은 최신 응답                                                                   |
| MarketInfo | order_fx_rates  | 없음                                                 | OrderFxRates                | PortError | O-04, O-09       | T-08 · /api/v1/exchange-rate                                   | ADR 0045는 같은 응답의 rate·midRate를 고액 판정에 함께 요구한다. 낮음: 두 번의 fx_rate 호출로 동일 응답을 증명할 수 없어 별도 메서드로 유도      |

portfolio는 평가액을 계산하거나 최신 Quote로 보유 현재가를 덮지 않는다. 어댑터는 기존 Holding·ReportedHolding·ReportedPortfolio를 생성하고 두 통화 BuyingPower와 Portfolio를 조립한다. 평가 계산은 domain, 조회 시각 정합과 부분 실패 처리 정책은 A-06·A-08이다. 조합 중 필수 조회가 실패하면 오류이며 0원·빈 성공으로 바꾸지 않는다. 완전한 빈 보유 응답과 조회 실패는 구별한다. 계좌 목록의 서버 순서를 유지해 U-22의 첫 계좌 선택을 바꾸지 않는다. account_display는 같은 목록에서 요청 계좌와 일치하는 행만 사용한다.

candles는 TimeInterval의 [start,end) 범위에 있는 봉 timestamp를 반환한다. 어댑터가 count·before 페이징과 중복 처리를 끝내 과거순으로 반환한다. 거래일 공백·거래 정지·확정 여부는 ADR 0035의 T-07 책임을 유지하며 빈 목록으로 적격 표본을 가장하지 않는다. 종목 쌍의 두 ID가 같은 종목인지 확인할 수 없으면 InvalidResponse다. 종목 쌍은 Clippy의 인자 수 제한을 지키면서 domain 타입만 전달하는 튜플이다. application 전용 요청 DTO는 만들지 않는다. 수정 기준은 요청 맥락으로 유지하며 개별 수정 계수를 발명하지 않는다.

## 정의하지 않는 행

소비자·공급자가 있어도 현재 domain 계약이 없거나 3.3 범위 밖이면 정의하지 않는다. 다음은 개정 필요이며 완료된 조회 기능으로 세지 않는다.

| 영역                                        | 소비자           | 공급자·엔드포인트                                                                | 필요한 개정과 이유                                                                                                                                                                                                                                                                              |
| ------------------------------------------- | ---------------- | -------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 종목 마스터·이름·유형·섹터                  | O-01, D-12, U-29 | T-09 · /api/v1/stocks, /stocks/all, /sectors                                     | architecture의 broker-toss 배치 외에 BrokerRead 귀속 근거가 없다. 별도 포트 또는 3.3 개정 필요. ResolvedOrderInstrument·InstrumentNameFact는 공개 생성 경로가 있지만 PolicySecurityFacts::from_master_lookup_results는 pub(crate)다. ADR 0048대로 T-09가 생성 경계를 연결한다. A-01은 열지 않음 |
| 랭킹·수급                                   | G-07             | T-10 · /api/v1/rankings, /rankings/investor-trading                              | 역할 범위이나 현재 domain에 요청 종류·기간·순위·rankedAt의 봉인 타입이 없다. Vec<Symbol>로 순위·집계 시각을 버리지 않는다. domain 계약 추가 후 포트 개정 필요. 낮음: 직접 소비자 G-07 하나                                                                                                      |
| 국내 지수                                   | A-09             | T-08 · /api/v1/market-indicators/prices                                          | 지수 포인트는 Money·거래 통화 가격이 아니다. domain에 전용 지표 타입이 없고 Quote로 위장하지 않는다. domain 계약 추가 후 포트 개정 필요. 낮음: 직접 소비자 A-09 하나                                                                                                                            |
| 매도가능수량·수수료·매수 유의사항·주문 목록 | O-01, O-11, O-19 | T-11·T-15 · /sellable-quantity, /commissions, /stocks/{symbol}/warnings, /orders | 3.3의 이번 역할 목록 밖이다. 별도 포트 또는 역할 개정 필요. ConfirmedOrderFee는 ADR 0045대로 D-22가 공개 생성 경로를 정하며 이번에 봉인을 열지 않음                                                                                                                                             |
| 사용자 이관 확인·시작 잔고                  | A-07, D-30       | 후속 A·S, broker 조회가 아님                                                     | ADR 0047의 사용자 확인 증거 발급과 연속 기간 저장·복원 책임이다. 임의 네트워크 메서드나 0원 시작 잔고를 추가하지 않음                                                                                                                                                                           |

## 도메인 사실의 인계

HoldingObservation·ReportedHolding·ReportedPortfolio·BuyingPower는 portfolio 반환 안에 보존되며 BuyingPower는 단독 재조회도 가능하다. 계좌 표시 사실은 account_display, Quote·Candle은 quote·candles, 캘린더는 calendar, 환율은 fx_rate·order_fx_rates의 반환이다. 모두 기존 공개 생성 경로만 사용한다. LedgerEvidence::from_complete_response는 transactions의 반환을 소비하는 A-07/D-30 연결이며 별도 IO가 아니다. 종목명·계좌 표시·환율 생성자의 호출 위치 제한은 ADR 0045의 후속 하네스 책임이다. 포트 시그니처 자체가 네트워크 사실의 진위를 인증하지 않는다.

## BrokerCapabilities

닫힌 지원 집합 BTreeSet<BrokerCapability>를 BrokerCapabilities 비공개 필드에 담는다. 빈 집합은 지원 선언 없음이며 전체 지원으로 해석하지 않는다. 중복은 집합이 제거한다. 어댑터가 구성 시 제공하는 정적 메타데이터로, capability 조회 HTTP가 없으므로 IO 트레이트에 새 메서드를 만들지 않는다. 조립 지점에서 조회 포트와 함께 유스케이스에 주입한다. 지원 선언은 주문 허가가 아니며 O-01의 장·잔고·원칙 검사와 사람 승인을 대체하지 않는다.

| 항목                   | 닫힌 도메인 차이·공급자                                | 소비자                                                   |
| ---------------------- | ------------------------------------------------------ | -------------------------------------------------------- |
| KrLimit                | OrderTerms::KrLimit, T-16 /orders                      | A-14, O-01                                               |
| KrMarket               | OrderTerms::KrMarket, T-16 /orders                     | A-14, O-01                                               |
| UsLimit                | OrderTerms::UsLimit, T-16 /orders                      | A-14, O-01                                               |
| UsMarketBuy            | OrderTerms::UsMarketBuy, T-16 /orders                  | A-14, O-01                                               |
| UsMarketSell           | OrderTerms::UsMarketSell, T-16 /orders                 | A-14, O-01                                               |
| UsMarketAmountBuy      | OrderTerms::UsMarketAmountBuy, T-16 /orders            | A-14, O-01                                               |
| UsFractionalMarketSell | SellOrderQuantity의 소수 가능, T-16 /orders            | O-01 (정규장 종료 1시간 전 제한). 낮음: 직접 소비자 하나 |
| DailyCandles           | CandlePeriod::Daily, T-07 /candles interval=1d         | A-15, A-16                                               |
| MinuteCandles          | CandlePeriod::Minute, T-07 /candles interval=1m        | A-15, J-12                                               |
| AdjustedCandles        | PriceAdjustment::Adjusted, T-07 /candles adjusted=true | A-15, A-16                                               |
| RawCandles             | PriceAdjustment::Raw, T-07 /candles adjusted=false     | A-07 (ADR 0047 입출고 평가). 낮음: 직접 소비자 하나      |

시간외 지원은 이번 OrderTerms의 별도 주문 변형이 아니므로 임의 항목으로 만들지 않는다. 실제 장 시간은 calendar가 제공한다. 소수 매수·금액 매도·정정·취소도 새 지원 항목에 넣지 않는다.

## 오류·비동기·검증

PortError의 세 변형은 각각 접근 거부, 일시 이용 불가, 계약에 맞지 않는 응답이다. 자유 문자열·비밀·본문·원시 상태 코드를 보관할 필드가 없다. A-05가 사용자 조치·대기·버그로 분류하고 T-04가 토스 코드를 변환한다. Unknown 주문 결과 같은 업무 상태를 이 오류로 대체하지 않는다. 이 세 종류 외의 실제 오류를 만났을 때 조용히 성공으로 바꾸지 말고 후속 계약을 검토한다.

architecture·harness는 조회 트레이트의 async 방식과 객체 안전성을 지정하지 않는다. 표준 Future를 반환하는 RPITIT와 Send, 트레이트의 Send + Sync만 사용한다. 외부 의존성·런타임·박스 할당을 추가하지 않으며 제네릭으로 조립한다. dyn 호환성은 제공하지 않는다. 동적 주입이 필요한 소비자가 생기면 그때 계약을 검토한다.

application은 기존 멤버·린트 상속·arch-check·capability-check·커버리지 등록을 그대로 사용하고 domain 경로 의존 하나만 추가한다. 외부 패키지·기능·라이선스·허용 목록 추가는 없다. 커버리지 packages에는 application이 이미 포함되어 있고 파일별 100%는 order_gate 경로에 적용된다. 일반 조회 포트는 변이 대상 밖이다. 기존 테스트·검사 규칙·예외는 변경하지 않는다.

테스트 전용 최소 가짜가 각 메서드를 명시적 domain 시그니처로 구현하고 제네릭 포트를 통해 전부 호출한다. 각 결과 변수에도 독립적으로 domain 반환 타입을 적어 타입을 검사하며 Future의 Send를 요구한다. 모든 메서드의 정확한 오류 반환과 지원 집합의 빈 값·포함·비포함을 확인한다. 이는 컴파일 계약 검사이며 임의 소스 변경을 탐지하는 AST 검사나 T 어댑터의 성공 응답 계약 테스트를 대신하지 않는다. 런타임 없는 단일 poll은 즉시 완료되는 가짜에만 사용한다. 실제 IO Future 실행기를 제공하지 않는다.

프로젝트 설정 hooks=true는 확인했지만 실제 프로젝트 신뢰·훅 정의 신뢰 적용은 증명하지 못했다. 직접 전체 검사와 Git 커밋 훅 결과를 증거로 사용한다.

## architecture 3.3 반영 문장 초안

BrokerRead는 계좌 키 목록, 계좌 표시 사실, 두 통화 매수가능금액을 포함한 포트폴리오, 완전 거래 응답, 현재가, 캔들, 통화별 매수가능금액을 조회한다. MarketInfo는 세 행 장 캘린더, 현재·과거 참고 환율, 한 응답의 주문용 환율 쌍을 조회한다. 입출력은 domain 타입이며 오류는 본문을 담지 않는 PortError다. 포트는 Send + Sync이고 메서드는 Send Future를 반환하며 제네릭으로 주입한다. 페이지·재시도·속도 제한은 어댑터 내부다. BrokerCapabilities는 구성 시 주입하는 닫힌 지원 집합이며 주문 승인 권한이 아니다. 랭킹·국내 지수는 전용 domain 계약을 추가한 뒤 정의하고, 종목 마스터는 별도 포트 또는 역할 개정으로 귀속을 정한다.
