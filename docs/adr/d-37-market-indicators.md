# D-37 시장 지표·랭킹·수급·섹터 구성

- 상태: 초안
- 시작 기준: `856cad43913e3275e835f9c2151d801c32d7cc29`, 브랜치 `d-37`

## 근거와 단위

로컬 `docs/toss-openapi-1.2.28.json`의 Market Indicators 카탈로그, MarketIndicatorPriceResponse·MarketIndicatorCandle, RankingResponse·RankingItem, InvestorTradingRecord·StockInvestorTradingRecord, SectorStocksResponse를 읽었다. 실제 API·키·계좌 자료는 사용하지 않았다.

| 스키마 심볼 | IndicatorCode | 단위 |
|---|---|---|
| KOSPI | Kospi | IndexPoints |
| KOSDAQ | Kosdaq | IndexPoints |
| KR_BOND_2Y | KrBond2Y | YieldPercent |
| KR_BOND_3Y | KrBond3Y | YieldPercent |
| KR_BOND_5Y | KrBond5Y | YieldPercent |
| KR_BOND_10Y | KrBond10Y | YieldPercent |
| KR_BOND_20Y | KrBond20Y | YieldPercent |
| KR_BOND_30Y | KrBond30Y | YieldPercent |
| 그 외 | Unknown, 원문 보존 | Unknown |

지수는 IndexPoints, 국채 금리는 IndicatorYieldPercent, 미지 단위는 UnclassifiedIndicatorValue다. 세 타입의 숫자 필드는 비공개이며 숫자 접근자·Money·Price 변환·산술·순서 비교를 제공하지 않는다. IndicatorValue의 Display는 제공하지 않는다. display()는 Points·YieldPercent·Unknown·VolumeShares의 닫힌 IndicatorDisplay를 반환하므로 표시값을 얻을 때 단위가 함께 남는다. IndicatorDisplayText는 비공개 생성의 표시 전용 문자열이며 수치 타입으로 암묵 변환되지 않는다. 공개 생성자는 NumericInput 등 응답 경계 입력만 받으며 이미 봉인된 값을 다른 대상에 넣는 생성자는 없다. 표시 문자열을 다시 원시 입력으로 보내는 어댑터의 거짓 진술까지 인증하지는 않는다.

현재가 응답에는 전일 대비·등락률이 없다. IndicatorChange::NotProvided로 보존하며 직접 계산하지 않는다. timestamp 없음은 ObservationAlignment::Unknown이다. 서버 시각·수집 시각·오프셋을 그대로 보존하며 D-11 assess_time의 2초 허용과 나노초 경계를 재사용한다. 원 시각을 수집 시각으로 메우지 않는다. 알려진 카탈로그는 국내 지표이며 미지 코드의 시장은 Unknown이다. 출처는 TossOpenApi다. 미국 ETF 근사·FRED는 이 응답의 출처라고 주장하지 않는다.

## 지표 봉과 D-39 인계

MarketIndicatorCandle은 OHLC를 IndicatorValue로 보존하고 Price·Money를 사용하지 않는다. 지표 봉 timestamp는 봉 시작이다. 일봉 예제 09:00을 종목 CandleTime의 자정으로 바꾸지 않는다. 분봉도 시작 시각을 보존한다. 국채 분봉·미지 심볼 봉은 거부한다. OHLC 범위·양수 지수·음수 거래량·미래 시각을 검증한다. IndicatorCandleStatus는 Finalized·InProgress·Unknown이다. 분봉은 스키마의 봉 시작 시각 + 60초를 끝으로 삼고, 끝이 수집 시각보다 엄격하게 이전일 때만 Finalized다. 끝과 수집 시각이 같거나 끝이 이후면 InProgress다. 표현 범위 밖의 끝은 Unknown이다. 일봉은 시작 시각만으로 장 종료를 증명할 수 없어 Unknown이다. 종목용 D-11 CandleStatus도 확정 판정 자체는 어댑터가 공급하므로, 그것을 무조건 복사하지 않는다. D-11의 시계 차이 검사는 그대로 재사용한다. 거래량은 IndicatorVolume으로 감싸 Quantity로 꺼내는 공개 경로를 제공하지 않는다.

지수 봉을 D-11 Candle로 감싸면 포인트가 가격·통화로 둔갑하므로 금지한다. 기존 benchmark.rs의 BenchmarkSeries 필드는 비공개이고 공개 생성 경로는 from_candles뿐이다. 기존 impl을 바꾸지 말라는 범위를 지켜 이번에는 연결을 추가하지 않는다. **D-39**가 봉인된 지수 봉 열을 받아 동일 코드·일봉·확정 거래일·시각 엄격 증가를 검증한 뒤, 내부에서 인접 종가 비율 `close[t]/close[t-1]-1`만 BenchmarkSeries에 넣는 생성 경로가 필요하다. 분자·분모의 포인트 단위가 소거된다. 원 통화 성과가 아닌 원화 환산 성과의 환율 처리·스냅샷 날짜 정렬은 기존 D-09 계약을 유지해야 한다. 임의 Ratio 열을 받는 공개 생성자를 열지 않는다. D-39는 종목용 Candle 경로를 ETF 벤치마크에만 남기고 지수 위장 입력을 닫는다. 이 연결과 일봉 종료 근거의 공급 계약은 D-39의 인계 사항이다.

## 랭킹

| 스키마 type | RankingKind | RankingBasis |
|---|---|---|
| MARKET_TRADING_AMOUNT | MarketTradingAmount | Amount(Money), 시장 통화 |
| MARKET_TRADING_VOLUME | MarketTradingVolume | Shares(Quantity) |
| TOSS_SECURITIES_TRADING_AMOUNT | TossTradingAmount | Amount(Money), 토스 체결 기준 |
| TOSS_SECURITIES_TRADING_VOLUME | TossTradingVolume | Shares(Quantity), 토스 체결 기준 |
| TOP_GAINERS | TopGainers | ChangeRate(Ratio), 기간 등락률 |
| TOP_LOSERS | TopLosers | ChangeRate(Ratio), 기간 등락률 |
| 그 외 | Unknown | 기준 단위를 알 수 없어 사실 생성 거부 |

기간은 realtime·1d·1w·1mo·3mo·6mo·1y와 Unknown이다. TOP 두 종류의 realtime은 거부한다. 응답의 Symbol·시장·통화만 보존하며 InstrumentId를 심볼에서 발명하지 않는다. D-33 QuoteInstrument는 마스터 조회 후 같은 시장·심볼인지 대조해서 연결할 대상이다. 순위로 정렬 후 양수의 엄격한 증가·동순위 금지·종목 중복 금지·기준 값 역순을 검사한다. 1부터 연속이면 RankingContinuity::Continuous, 시작 또는 중간에 빈자리가 있으면 Gaps이며 빈자리 자체로 응답을 거부하지 않는다. 기준 값의 동률은 허용하며 TOP_LOSERS만 값 오름차순이다. change_rate 입력은 nullable이고 RankingChange::Unavailable/Available로 보존한다. TOP 두 종류에서만 미제공을 거부한다. 제공된 등락률은 종류와 무관하게 -1 이상이어야 하며 소수 비율 그대로 보존한다. excludeInvestmentCaution=false는 RankingCautionFilter::Included, true는 Excluded로 요청과 사실에 보존한다. 반환 수가 요청 수보다 작을 수 있으므로 TopN은 전체 목록 완전성 주장이 아니다. 상한은 스키마의 100이며 rankedAt null은 Unknown이다.

## 투자자 수급

| 스키마 필드 | InvestorClass |
|---|---|
| individual | Individual |
| foreigner | Foreigner |
| institution | Institution |
| otherCorporation | OtherCorporation |
| financialInvestment | FinancialInvestment |
| insurance | Insurance |
| trust | Trust |
| privateEquityFund | PrivateEquityFund |
| bank | Bank |
| otherFinancialInstitution | OtherFinancialInstitution |
| pensionFund | PensionFund |
| 그 외 | Unknown, 원문 보존 |

종목별 순매수는 정수 주식 수이며 응답 netBuyVolume과 buyVolume−sellVolume을 대조한다. 시장 수급은 정수 KRW 매수·매도만 제공하므로 순매수 차를 domain의 정확 Amount 뺄셈으로 얻는다. 금액을 수량으로 바꾸지 않는다. 종목 외국인은 등록외국인만, 시장 외국인은 등록·미등록 합계다. 집계 대표 일자와 1d·1w·1mo·1y 간격을 보존하며 해당 간격의 실제 영업일 시작·끝을 발명하지 않는다. 서버 updatedAt과 수집 시각은 별도로 보존한다. 확정 여부 필드가 없고 updatedAt만으로 휴장·NXT·T+1 반영을 인증할 수 없어 FlowFinality::Unknown으로 둔다. 누락된 nullable 개인·기관 세부·기타법인은 행을 만들지 않으며 0으로 채우지 않는다. 빈 행 목록은 거부한다. StockFlowRow/StockFlowRecordInput과 MarketFlowRow/MarketFlowRecordInput은 서로 다른 입력 타입이므로 주식 수 행을 원화 생성자에 넣을 수 없다. 집계일이 updatedAt의 보존 오프셋 날짜보다 뒤면 거부한다.

FlowLevel은 TopLevel·InstitutionDetail·Unknown이다. top_level(), institution_details(), unknown_items() 접근자를 나누고 섞인 items() 경로는 제거했다. 시장 수급에는 상위 네 분류가 모두 필요하다. 기관 세부가 있으면 7개 모두와 상위 기관 합계가 있어야 하고 매수·매도·순매수 합계가 일치해야 한다. 종목 수급에도 같은 기관 검증을 적용한다. 합계 넘침은 닫힌 오류다. 시장 네 분류의 매수 합계와 매도 합계는 비교하여 FlowBalance::Matched/Mismatched로 보존하며 불일치만으로 거부하지 않는다. 종목 수급에는 NotApplicable을 둔다. 미지 분류는 별도로 보존하며 상위 네 분류나 기관 세부의 합계에 끼워 넣지 않는다.

## 섹터 구성과 기존 경로

/stocks 응답은 섹터→시장별 Symbol 목록이며 하위 섹터 종목을 포함한다. SectorCompositionScope::IncludingDescendants를 보존한다. 목록 순서는 서버 순서를 유지하고 중복을 거부한다. 서버 시각은 없으므로 Unknown과 수집 시각만 보존한다. 마스터에 섹터가 있다고 가정하지 않는다. identified_member는 D-33 MasterInstrument의 QuoteInstrument와 시장·심볼을 대조한다. SectorStockRow에서 시가총액·거래대금을 받아 SectorMember에 SectorAmount::Available/Unavailable로 보존한다. US 종목도 스키마대로 KRW 환산 금액이며 USD 입력·음수·잘못된 수치는 거부한다. 0과 미제공을 혼동하지 않는다. G-07이 시가총액 구간을 선택할 수 있도록 금액 사실 접근자를 제공한다.

D-06 SectorAssignment::new는 현재 공개다. 이것을 닫으려면 기존 impl과 소비자·테스트 이전이 필요하다. 이번에는 수정하지 않는다. **D-40**이 조회 결과에서만 SectorAssignment를 만들게 하고 기존 공개 생성자를 닫는다. A-20·T-10 소비자 이전도 D-40에서 함께 다룬다. D-40은 다중 섹터 소속의 선택 규칙을 정해 D-12의 섹터 상한이 집중도를 덜 잡지 않도록 한다. 하위 섹터를 포함하는 구성만으로 종목의 유일한 최하위 섹터를 추정해서도 안 된다. 이 조회 결과에서 임의 SectorAssignment를 자동 생성하지 않는다.

## 인계와 실측 항목

- **D-39**: 지수 봉 열(MarketIndicatorCandle)에서 BenchmarkSeries를 만드는 봉인된 경로. 종목용 Candle 경로는 ETF 벤치마크에만 쓰고 지수 위장 경로를 닫는다. 일봉 종료 근거와 거래일 확정 증거, 스냅샷 날짜 정렬도 이 경로에서 확인한다.
- **D-40**: 섹터 소속(SectorAssignment)을 섹터 구성 조회 결과에서만 만들게 하고 기존 공개 생성자를 닫는다. 다중 섹터 선택 규칙과 A-20·T-10 소비자 이전을 담당한다.
- **D-41**: 투자자 순매수·순매도 랭킹, 섹터 목록·상세·섹터 랭킹, ETF 구성의 타입. ADR 0055의 나머지 D 작업 목록과 종목별 프로그램·공매도·신용·대차 수급 타입도 이 인계 목록에서 추적한다. A-20 계약 전체의 완료를 주장하지 않는다.
- **T-10 실측**: 시세 조회 실패로 제외된 종목의 서버 rank 재부여 여부, 같은 순위 둘의 존재 여부. 스키마에 동순위가 없어 현재는 거부한다. 빈자리는 사실로 보존한다.
- **T-10 실측**: 장중 잠정치에서 상위 네 분류의 매수·매도 합계가 일시적으로 어긋나는지. 현재는 불일치 표지를 보존한다.
- **T-08 실측**: 지표 timestamp가 없고 lastPrice가 0인 실제 응답의 의미. 기존 양수 지수 검증은 그대로 두며 데이터 없음이라고 추정해 치환하지 않는다. 지수·국채 일봉의 종료 근거와 최종 갱신 시점도 확인한다.
- 실제 서버 응답의 진위·최신성·Symbol과 안정적 ID의 대응은 공급 어댑터의 책임이다.
- 프로젝트 파일 hooks=true는 확인했으나 사용자 프로젝트 신뢰·훅 정의 신뢰의 실제 적용은 확인하지 못했다. 홈 설정을 읽거나 변경하지 않았고 실제 just check·Git 훅 결과를 별도로 보고한다.

## 최초 검증 기록 (f168680)

- 시작 frozen install·lefthook install 뒤 `just base=856cad43913e3275e835f9c2151d801c32d7cc29 check`: 종료 0. 기준 전체 테스트 1,058개 통과, 커버리지 위반 없음.
- 최종 `cargo clippy --locked -p domain --all-targets -- -D warnings`: 종료 0. 검사 끄기·새 의존성·기존 테스트 변경 없음.
- 최종 `cargo nextest run --locked -p domain -E 'test(market_index) | test(market_ranking) | test(investor_flow) | test(sector_composition)'`: 19 passed. 랭킹 순위 연속성과 임의 입력 순서 정규화 속성은 Config::with_cases(4096)으로 검증했다.
- 최종 `just coverage-check`: 741 passed, `coverage-check: 위반 없음`. 새 테스트를 모듈 내부에 두어 측정 예외를 추가하지 않았다. 수급의 정확 뺄셈은 원시 숫자 파싱 뒤 의미 검증 전에 수행해 실제 넘침 오류 경로를 검증하며, 의미 검증을 통과하지 못한 값은 사실로 발행하지 않는다.
- 별도 외부 crate의 rustc 컴파일: 허용 대조군 1개 성공, 금지 실험 13개 모두 거부. Money·Price·Decimal 추출 E0599, 지수·시장 재봉인과 단위 혼합 E0308, IndexPoints→Ratio E0277, 비공개 생성 E0423, 지수 포인트와 금리 합산 E0369. 수급 주식 수와 원화 타입의 비교도 E0308이다.
- 자동 cargo-mutants를 실행하지 않았다. 최종 네 모듈을 임시 복사본에 복사해 아래 손 변형 30개를 각각 실행했다. 30개 모두 컴파일됐고 테스트 실패로 검출됐다. 생존 0, 컴파일 실패를 검출로 센 경우 0. 실행 뒤 임시 복사본의 네 모듈이 원본과 바이트 단위로 같은지 확인했다.

| 번호 | 손 변형 |
|---|---|
| 1 | 지표 코드 Kospi — 컴파일 성공, 테스트 실패 |
| 2 | 지표 코드 Kosdaq — 컴파일 성공, 테스트 실패 |
| 3 | 지표 코드 KrBond2Y — 컴파일 성공, 테스트 실패 |
| 4 | 지표 코드 KrBond3Y — 컴파일 성공, 테스트 실패 |
| 5 | 지표 코드 KrBond5Y — 컴파일 성공, 테스트 실패 |
| 6 | 지표 코드 KrBond10Y — 컴파일 성공, 테스트 실패 |
| 7 | 지표 코드 KrBond20Y — 컴파일 성공, 테스트 실패 |
| 8 | 지표 코드 KrBond30Y — 컴파일 성공, 테스트 실패 |
| 9 | 시각 부재를 정렬로 오인 — 컴파일 성공, 테스트 실패 |
| 10 | 정렬 표지 — 컴파일 성공, 테스트 실패 |
| 11 | 지수 영점 경계 — 컴파일 성공, 테스트 실패 |
| 12 | 미지 봉 코드 — 컴파일 성공, 테스트 실패 |
| 13 | 국채 분봉 경계 — 컴파일 성공, 테스트 실패 |
| 14 | 고저 범위 — 컴파일 성공, 테스트 실패 |
| 15 | 하한 경계 — 컴파일 성공, 테스트 실패 |
| 16 | 상한 경계 — 컴파일 성공, 테스트 실패 |
| 17 | 요청 수 상한 포함 — 컴파일 성공, 테스트 실패 |
| 18 | 연속 순위 — 컴파일 성공, 테스트 실패 |
| 19 | 중복 종목 — 컴파일 성공, 테스트 실패 |
| 20 | 하락 랭킹 방향 — 컴파일 성공, 테스트 실패 |
| 21 | 일반 랭킹 방향 — 컴파일 성공, 테스트 실패 |
| 22 | 통화 일치 — 컴파일 성공, 테스트 실패 |
| 23 | 국내 심볼 길이 — 컴파일 성공, 테스트 실패 |
| 24 | 소문자 거부 — 컴파일 성공, 테스트 실패 |
| 25 | 음수 거래량 — 컴파일 성공, 테스트 실패 |
| 26 | 정수 단위 — 컴파일 성공, 테스트 실패 |
| 27 | 외국인 등록 범위 — 컴파일 성공, 테스트 실패 |
| 28 | 구성 중복 — 컴파일 성공, 테스트 실패 |
| 29 | 구성 시장 일치 — 컴파일 성공, 테스트 실패 |
| 30 | 구성 종목 일치 — 컴파일 성공, 테스트 실패 |


## 적대적 리뷰 수정 검증

- 로컬 master `257bd4c94999034745f75fb7df2c1fb545138919`를 `git merge --no-ff --no-edit master`로 받았다. lib.rs 양쪽 선언을 보존하고 생성물은 기존 스크립트로 다시 만들었다. 병합 커밋 `7c1c80b3046f57cf6fc487d37924bfc10e2fef7e`의 정상 훅에서 전체 1,099개·커버리지 763개 통과, test-diff 위반 없음. LEFTHOOK=0 예외는 사용하지 않았다.
- 수급의 새 회귀 테스트를 먼저 작성했을 때 새 API 미구현으로 컴파일 실패(종료 101)를 확인한 뒤 구현했다. 기존 master 테스트는 변경하지 않았다. docs/tasks.md는 master 병합 내용 외에 직접 편집하지 않았다.
- 최종 clippy `cargo clippy --locked -p domain --all-targets -- -D warnings` 통과. source-check·raw-types 위반 없음. 수정 후 커버리지는 775개 테스트 통과와 위반 없음까지 확인했고, 이후 추가한 정렬 회귀를 포함한 최종 전체 검사는 커밋 훅에서 다시 실행한다.
- 외부 crate: 허용 대조 1개 성공, 금지 14개 모두 컴파일 거부. IndicatorValue의 Display(E0599), 지표 거래량→Quantity(E0308) 및 수치 추출(E0599), 닫힌 표시 값→NumericInput(E0308), 주식 수 기록·행→원화 입력(E0308), 평탄한 수급 items 접근(E0599), 지수 Money·Price 추출(E0599), 재봉인·단위 혼합(E0308), 봉 확정 표지 직접 변경(E0616), 불린 필터(E0308)를 포함한다.
- 랭킹 단위 기대값은 종류별 상수로 단언한다. 소수 비율 0.0125, 유의종목 필터, nullable 등락률, -1 경계, 같은 순위·서로 다른 종목, 미국 심볼 첫 글자만 잘못된 반례를 검증한다. Config::with_cases(4096)으로 연속 입력과 임의의 빈자리 입력 각각의 순서 정규화를 확인한다.
- 수급은 계층별 정렬·기관 합계·상위 합계 불일치 표지·입력 단위·집계일·넘침을 검증한다. 구성 시장 검사는 심볼이 같고 시장만 다른 반례로 고정한다. 지표 봉의 종료 동등 경계와 표현 범위 밖의 끝, 닫힌 표시 단위, 시가총액·거래대금·미제공 보존도 검사한다.
- 손 변형 최초 실행에서 필터 변형 1개는 컴파일 실패여서 집계하지 않았다. 수급 정렬을 단순 역순으로 바꾼 변형이 생존해 상위·기관 세부·미지 분류 각각의 여러 입력 순서에 대한 독립 단언을 추가했다. 필터 변형도 입력을 소비하는 컴파일 가능한 형태로 수정했다.
- 최종 소스 복사본의 집중 테스트 32개가 통과한 뒤 아래 32개 손 변형을 전부 재실행했다. 32개 모두 컴파일되고 테스트 실패로 검출됐다. 생존 0, 최종 컴파일 실패 0, 자동 변이 검사 실행 없음. 종료 뒤 다섯 소스 파일이 원본과 바이트 단위로 일치함을 확인했다.

| 번호 | 수정 분기 손 변형 | 최종 결과 |
|---|---|---|
| 1 | 빈자리 표지 | 컴파일 성공·테스트 검출 |
| 2 | 동순위 거부 | 컴파일 성공·테스트 검출 |
| 3 | 순서 무관 정규화 | 컴파일 성공·테스트 검출 |
| 4 | 등락률 미제공 보존 | 컴파일 성공·테스트 검출 |
| 5 | 등락률 랭킹의 미제공 거부 | 컴파일 성공·테스트 검출 |
| 6 | 등락률 -1 경계 | 컴파일 성공·테스트 검출 |
| 7 | 등락률 하한 거부 | 컴파일 성공·테스트 검출 |
| 8 | 토스 거래대금 단위 상수 | 컴파일 성공·테스트 검출 |
| 9 | 등락률 100배 변형 | 컴파일 성공·테스트 검출 |
| 10 | 유의종목 제외 필터 | 컴파일 성공·테스트 검출 |
| 11 | 미국 심볼 첫 글자 독립 반례 | 컴파일 성공·테스트 검출 |
| 12 | 기관 세부 계층 | 컴파일 성공·테스트 검출 |
| 13 | 시장 상위 네 분류 필수 | 컴파일 성공·테스트 검출 |
| 14 | 빈 행 거부 | 컴파일 성공·테스트 검출 |
| 15 | 기관 세부 일곱 분류 필수 | 컴파일 성공·테스트 검출 |
| 16 | 기관 매수 합계 | 컴파일 성공·테스트 검출 |
| 17 | 기관 매도 합계 | 컴파일 성공·테스트 검출 |
| 18 | 순매수 보존 | 컴파일 성공·테스트 검출 |
| 19 | 상위 매수 합계 계산 | 컴파일 성공·테스트 검출 |
| 20 | 상위 매도 합계 계산 | 컴파일 성공·테스트 검출 |
| 21 | 시장 합계 차이 표지 | 컴파일 성공·테스트 검출 |
| 22 | 수급 정렬 독립 반례 | 컴파일 성공·테스트 검출 |
| 23 | 집계일 동일 날짜 경계 | 컴파일 성공·테스트 검출 |
| 24 | 미래 집계일 거부 | 컴파일 성공·테스트 검출 |
| 25 | 봉 종료와 수집 시각 동등 경계 | 컴파일 성공·테스트 검출 |
| 26 | 분봉 구간 길이 | 컴파일 성공·테스트 검출 |
| 27 | 일봉 종료 근거 미제공 | 컴파일 성공·테스트 검출 |
| 28 | 지표 거래량 표시 단위 | 컴파일 성공·테스트 검출 |
| 29 | 지수 표시 단위 | 컴파일 성공·테스트 검출 |
| 30 | 구성 시장 독립 반례 | 컴파일 성공·테스트 검출 |
| 31 | 시가총액 보존 | 컴파일 성공·테스트 검출 |
| 32 | 미국 섹터 구성 원화 보존 | 컴파일 성공·테스트 검출 |
