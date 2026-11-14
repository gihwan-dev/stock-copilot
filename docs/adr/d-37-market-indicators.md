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

지수는 IndexPoints, 국채 금리는 IndicatorYieldPercent, 미지 단위는 UnclassifiedIndicatorValue다. 세 타입의 숫자 필드는 비공개이며 숫자 접근자·Money·Price 변환·산술·순서 비교를 제공하지 않는다. IndicatorValue의 닫힌 변형으로 표시한다. 공개 생성자는 NumericInput 등 응답 경계 입력만 받으며 이미 봉인된 값을 다른 대상에 넣는 생성자는 없다. 표시 문자열을 다시 원시 입력으로 보내는 어댑터의 거짓 진술까지 인증하지는 않는다.

현재가 응답에는 전일 대비·등락률이 없다. IndicatorChange::NotProvided로 보존하며 직접 계산하지 않는다. timestamp 없음은 ObservationAlignment::Unknown이다. 서버 시각·수집 시각·오프셋을 그대로 보존하며 D-11 assess_time의 2초 허용과 나노초 경계를 재사용한다. 원 시각을 수집 시각으로 메우지 않는다. 알려진 카탈로그는 국내 지표이며 미지 코드의 시장은 Unknown이다. 출처는 TossOpenApi다. 미국 ETF 근사·FRED는 이 응답의 출처라고 주장하지 않는다.

## 지표 봉과 D-09 인계

MarketIndicatorCandle은 OHLC를 IndicatorValue로 보존하고 Price·Money를 사용하지 않는다. 지표 봉 timestamp는 봉 시작이다. 일봉 예제 09:00을 종목 CandleTime의 자정으로 바꾸지 않는다. 분봉도 시작 시각을 보존한다. 국채 분봉·미지 심볼 봉은 거부한다. OHLC 범위·양수 지수·음수 거래량·미래 시각을 검증한다. 서버에 확정 여부 필드가 없으므로 이 타입은 봉 확정을 증명하지 않는다.

지수 봉을 D-11 Candle로 감싸면 포인트가 가격·통화로 둔갑하므로 금지한다. 기존 benchmark.rs의 BenchmarkSeries 필드는 비공개이고 공개 생성 경로는 from_candles뿐이다. 기존 impl을 바꾸지 말라는 범위를 지켜 이번에는 연결을 추가하지 않는다. D-09 후속에서 봉인된 지수 봉 열을 받아 동일 코드·일봉·확정 거래일·시각 엄격 증가를 검증한 뒤, 내부에서 인접 종가 비율 `close[t]/close[t-1]-1`만 BenchmarkSeries에 넣는 생성 경로가 필요하다. 분자·분모의 포인트 단위가 소거된다. 원 통화 성과가 아닌 원화 환산 성과의 환율 처리·스냅샷 날짜 정렬은 기존 D-09 계약을 유지해야 한다. 임의 Ratio 열을 받는 공개 생성자를 열지 않는다. 이 연결은 구현 완료로 주장하지 않는다.

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

기간은 realtime·1d·1w·1mo·3mo·6mo·1y와 Unknown이다. TOP 두 종류의 realtime은 거부한다. 응답의 Symbol·시장·통화만 보존하며 InstrumentId를 심볼에서 발명하지 않는다. D-33 QuoteInstrument는 마스터 조회 후 같은 시장·심볼인지 대조해서 연결할 대상이다. 순위로 정렬 후 1부터 연속인지, 종목 중복·기준 값 역순인지 검사한다. 동률을 허용하며 TOP_LOSERS만 값 오름차순이다. 반환 수가 요청 수보다 작을 수 있으므로 TopN은 전체 목록 완전성 주장이 아니다. 상한은 스키마의 100이며 rankedAt null은 Unknown이다.

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

종목별 순매수는 정수 주식 수이며 응답 netBuyVolume과 buyVolume−sellVolume을 대조한다. 시장 수급은 정수 KRW 매수·매도만 제공하므로 순매수 차를 domain의 정확 Amount 뺄셈으로 얻는다. 금액을 수량으로 바꾸지 않는다. 종목 외국인은 등록외국인만, 시장 외국인은 등록·미등록 합계다. 집계 대표 일자와 1d·1w·1mo·1y 간격을 보존하며 해당 간격의 실제 영업일 시작·끝을 발명하지 않는다. 서버 updatedAt과 수집 시각은 별도로 보존한다. 확정 여부 필드가 없고 updatedAt만으로 휴장·NXT·T+1 반영을 인증할 수 없어 FlowFinality::Unknown으로 둔다. 누락된 nullable 개인·기관 세부·기타법인은 행을 만들지 않으며 0으로 채우지 않는다.

## 섹터 구성과 기존 경로

/stocks 응답은 섹터→시장별 Symbol 목록이며 하위 섹터 종목을 포함한다. SectorCompositionScope::IncludingDescendants를 보존한다. 목록 순서는 서버 순서를 유지하고 중복을 거부한다. 서버 시각은 없으므로 Unknown과 수집 시각만 보존한다. 마스터에 섹터가 있다고 가정하지 않는다. identified_member는 D-33 MasterInstrument의 QuoteInstrument와 시장·심볼을 대조한다.

D-06 SectorAssignment::new는 현재 공개다. 이것을 닫으려면 기존 impl과 소비자·테스트 이전이 필요하다. 이번에는 수정하지 않는다. **A-20·T-10이 옮긴 뒤 닫는다.** 하위 섹터를 포함하는 구성만으로 종목의 유일한 최하위 섹터를 추정해서도 안 된다. 이 조회 결과에서 임의 SectorAssignment를 자동 생성하지 않는다.

## 미해결과 범위

- D-09 지수 봉 소비 경로·거래일 확정 증거는 위 인계대로 별도 변경이 필요하다.
- 수급 기관 7개 세부 합계, 시장 전체 매수·매도 합계의 응답 전체 대조는 T-08·T-10 계약에서 완전 응답을 확보한 뒤 해야 한다. 이 타입은 응답 행 단위의 수치·단위·집계 사실을 검증한다.
- 투자자 순매수·순매도 랭킹과 섹터 랭킹의 별도 응답, 섹터 목록·상세·관련 ETF, 공매도·프로그램·신용·대차는 이번 기본 네 사실 타입으로 구현되었다고 주장하지 않는다. A-20 계약 전체를 완료한 것도 아니다.
- 실제 서버 응답의 진위·최신성·Symbol과 안정적 ID의 대응은 공급 어댑터의 책임이다.
- 프로젝트 파일 hooks=true는 확인했으나 사용자 프로젝트 신뢰·훅 정의 신뢰의 실제 적용은 확인하지 못했다. 홈 설정을 읽거나 변경하지 않았고 실제 just check·Git 훅 결과를 별도로 보고한다.

## 검증 기록

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
