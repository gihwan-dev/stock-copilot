# D-44: 프로그램·공매도·신용·대차 조회 사실

- 상태: 초안

## 결정

기준은 dc8f3f950294e9d3d1947b39c256868076d7de15다. 저장소의 고정 스펙만 사용하며 실제 API를 호출하지 않는다. D-37·D-41의 검증 후 봉인 관례를 따른다. 원시 입력은 NumericInput으로 받아 단위별 비공개 필드를 가진 SupplyShares(0 이상 정수 주), SupplySignedShares(부호 있는 정수 주), SupplyKrw(0 이상 정수 원), SupplyRate(0~1 소수 비율)로 검증한다. 서로 대체할 수 없다. 빈 숫자는 오류이며 0은 실제 보고된 0이다. 프로그램 순수량은 매수−매도와 일치하는지만 검증하고 새로운 수급을 계산하지 않는다.

프로그램에는 updatedAt이 없다. ProgramContext는 기준일과 Clock 수집 시각만 보존하고 서버 시각은 None이다. 나머지는 SupplyContext로 기준일·updatedAt·수집 시각을 구분한다. 미래 기준일과 잘못된 국내 심볼, 허용 범위를 넘은 미래 서버 시각은 거부한다. 서로 다른 맥락 타입을 사용해 갱신 시각 없는 프로그램 맥락을 다른 응답에 넣지 못한다. 일간 사실의 확정성은 Unknown으로 취급한다(상수 접근자는 만들지 않는다). 스펙의 반영 시각 설명만으로 실제 확정을 인증하지 않는다.

봉인된 단위와 맥락을 받는 레코드 생성자는 추가적인 관계를 추정하지 않는다. 공매도 비율 null은 SupplyRatio::MissingDenominator, 보고된 0은 Reported(0)이다. 신용 한쪽 null은 CreditSide::NoData이며 0 잔고와 다르다. 신용융자와 개인 신용대주, 기관 대차를 구별한다. 잔고를 전일 증감으로 바꾸거나 대차와 공매도를 합산하지 않는다.

## 스펙 근거

검색 파일: docs/toss-openapi-1.2.28.json. 검색어: 프로그램, 공매도, 신용, 대차, program, short-selling, credit, lending. paths의 GET 200 응답에서 아래 components.schemas로 연결되는 것을 확인했다.

| 종류 | 스펙 | GET 경로 | 응답 스키마 | 단위·시각 |
|---|---|---|---|---|
| 프로그램 | 있음 | /api/v1/stocks/{symbol}/program-trades | ProgramTradesResponse → ProgramTradeRecord → ProgramTradingVolume | KRX만, NXT 제외, 차익·비차익 각각 주식 수. date만 있고 updatedAt 없음 |
| 공매도 | 있음 | /api/v1/stocks/{symbol}/short-selling | ShortSellingResponse → ShortSellingRecord | 주·KRW·소수 비율, date·updatedAt. 비율 분모는 정규장 외 포함 누적, 분모 부재 null·분모 0이면 0 |
| 신용 | 있음 | /api/v1/stocks/{symbol}/credit-trades | CreditTradesResponse → CreditTradeRecord → CreditTradeDetail | 주·소수 비율, date·updatedAt. 자기신용/유통금융 합산, T+1, 한쪽 null 가능 |
| 대차 | 있음 | /api/v1/stocks/{symbol}/securities-lending | SecuritiesLendingResponse → SecuritiesLendingRecord | 체결·상환·잔고 주 및 잔고 KRW, date·updatedAt |

## 인계

T-10은 요청 QuoteInstrument의 국내 시장·마스터 심볼과 응답 사실을 결합하고 from≤through 및 포함 날짜 창을 검증한다. count/until/nextUntil로 페이지를 수집하고 날짜순으로 반환하며 중복 날짜·검증 실패·페이지 누락을 빈 성공이나 완전 성공으로 숨기지 않는다. 빈 records만 정상 빈 목록이다. 미지원 시장/뒤집힌 기간은 InvalidRequest(UnsupportedQuery), 미지원 capability는 Unsupported(해당 기능), 잘못된 응답은 InvalidResponse(Validation)다. 오류 분류는 ADR 0063을 유지한다.

A-04는 후속 testkit 가짜에 새 메서드를 반영한다. A-09/G-07/M-04는 수집 시각과 Unknown·0/null 의미, 프로그램의 KRX 한정과 개인 신용대주/기관 대차 구별을 유지한다. A-15/E-08의 성적표·판정 계산 경로에는 이 조회를 연결하지 않는다. 실제 반영 시각·페이지 동작 실측은 T-10 소유다.

## 검증

시작 pnpm frozen install 및 lefthook install 성공. 프로젝트 신뢰와 세션 훅 신뢰 적용은 확인할 수 없다. 홈 설정이나 인증 파일은 읽거나 수정하지 않았다. Git 커밋 훅과 실제 실행한 검사로 검증한다. 새 외부 의존성과 검사 끄기는 없다.
## 손 변형 결과

리뷰 수정 제품 코드를 mktemp -d 복사본에서 한 번에 하나씩 변경하고 cargo test --locked -p domain --lib supply_tests로 실제 실행했다. 기준·복구 대조군 모두 통과. 27개 모두 컴파일 후 테스트 실패로 검출됐고 생존 0·컴파일 불가 0이다. 24번은 리뷰어가 발견한 updated→collected 생존을 잡는 보강이다. 이후 회귀 테스트의 중첩·숫자 타입만 정리했으며 제품 부분의 바이트 일치를 확인하고 최종 수급 테스트 7개를 다시 통과시켰다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 1 | buy 접근자 → sell | supply_values::supply_tests::supply_program_validates_reported_net_and_keeps_two_trading_groups | 검출 |
| 2 | sell 접근자 → buy | supply_values::supply_tests::supply_program_validates_reported_net_and_keeps_two_trading_groups | 검출 |
| 3 | arbitrage 접근자 → non_arbitrage | supply_values::supply_tests::supply_program_validates_reported_net_and_keeps_two_trading_groups | 검출 |
| 4 | non_arbitrage 접근자 → arbitrage | supply_values::supply_tests::supply_program_validates_reported_net_and_keeps_two_trading_groups | 검출 |
| 5 | new_quantity 접근자 → return_quantity | supply_values::supply_tests::supply_short_credit_and_lending_preserve_every_distinct_field | 검출 |
| 6 | return_quantity 접근자 → balance_quantity | supply_values::supply_tests::supply_short_credit_and_lending_preserve_every_distinct_field | 검출 |
| 7 | balance_quantity 접근자 → new_quantity | supply_values::supply_tests::supply_short_credit_and_lending_preserve_every_distinct_field | 검출 |
| 8 | balance_rate 접근자 → trading_rate | supply_values::supply_tests::supply_short_credit_and_lending_preserve_every_distinct_field | 검출 |
| 9 | trading_rate 접근자 → balance_rate | supply_values::supply_tests::supply_short_credit_and_lending_preserve_every_distinct_field | 검출 |
| 10 | margin_loan 접근자 → stock_loan | supply_values::supply_tests::supply_short_credit_and_lending_preserve_every_distinct_field | 검출 |
| 11 | stock_loan 접근자 → margin_loan | supply_values::supply_tests::supply_short_credit_and_lending_preserve_every_distinct_field | 검출 |
| 12 | execution_quantity 접근자 → repayment_quantity | supply_values::supply_tests::supply_short_credit_and_lending_preserve_every_distinct_field | 검출 |
| 13 | repayment_quantity 접근자 → balance_quantity | supply_values::supply_tests::supply_short_credit_and_lending_preserve_every_distinct_field | 검출 |
| 14 | balance_quantity 접근자 → execution_quantity | supply_values::supply_tests::supply_short_credit_and_lending_preserve_every_distinct_field | 검출 |
| 15 | volume_rate 접근자 → amount_rate | supply_values::supply_tests::supply_short_credit_and_lending_preserve_every_distinct_field | 검출 |
| 16 | amount_rate 접근자 → volume_rate | supply_values::supply_tests::supply_short_credit_and_lending_preserve_every_distinct_field | 검출 |
| 17 | 주식 수 0 거부 | supply_values::supply_tests::supply_integer_display_normalizes_scale_without_changing_values, supply_values::supply_tests::supply_program_validates_reported_net_and_keeps_two_trading_groups, supply_values::supply_tests::supply_numeric_units_boundaries_and_missing_values | 검출 |
| 18 | 주식 정수 검사 반전 | supply_values::supply_tests::supply_program_validates_reported_net_and_keeps_two_trading_groups, supply_values::supply_tests::supply_numeric_units_boundaries_and_missing_values, supply_values::supply_tests::supply_integer_display_normalizes_scale_without_changing_values, supply_values::supply_tests::supply_short_credit_and_lending_preserve_every_distinct_field | 검출 |
| 19 | 비율 1 거부 | supply_values::supply_tests::supply_numeric_units_boundaries_and_missing_values | 검출 |
| 20 | 비율 상한 제거 | supply_values::supply_tests::supply_numeric_units_boundaries_and_missing_values | 검출 |
| 21 | KST 기준일 검사 반전 | supply_values::supply_tests::supply_date_uses_updated_day_and_rejects_offset_disguised_future, supply_values::supply_tests::supply_context_preserves_dates_and_distinct_observation_clocks | 검출 |
| 22 | KST를 UTC 날짜로 오판 | supply_values::supply_tests::supply_dates_use_korean_market_day_independent_of_timestamp_offset | 검출 |
| 23 | 프로그램 순수량 일치 반전 | supply_values::supply_tests::supply_program_validates_reported_net_and_keeps_two_trading_groups | 검출 |
| 24 | 갱신일 대신 수집일 사용 | supply_values::supply_tests::supply_date_uses_updated_day_and_rejects_offset_disguised_future | 검출 |
| 25 | SupplyShares 정수 표시 정규화 제거 | supply_values::supply_tests::supply_integer_display_normalizes_scale_without_changing_values | 검출 |
| 26 | SupplySignedShares 정수 표시 정규화 제거 | supply_values::supply_tests::supply_integer_display_normalizes_scale_without_changing_values | 검출 |
| 27 | SupplyKrw 정수 표시 정규화 제거 | supply_values::supply_tests::supply_integer_display_normalizes_scale_without_changing_values | 검출 |

## 포트와 경계 검증

MarketInfo에 program_trades, short_selling, credit_trades, securities_lending을 추가했다. 모두 (&QuoteInstrument, CalendarDate from, CalendarDate through) → Result<Vec<종류별 일간 사실>, PortError>의 Send Future다. capability와 ReadEndpoint도 메서드별 네 개를 추가했다. ADR 0087의 계약 표는 29→33행, MarketInfo 서명은 14→18개이며 네 정의 대기 행을 해소했다. 실제 어댑터의 입력·페이지 검증을 trait 자체가 실행한다고 주장하지 않는다.

외부 rustc 대조군은 정상 컴파일·실행했다. SupplyKrw→SupplyShares 대체, ProgramContext→SupplyContext 대체, 비공개 수량 생성과 비공개 기준일 변경은 모두 컴파일이 거부됐다. 원본 소스에 주석·검사 끄기·새 의존성은 없다. 보호 변경은 계약 갱신에 필요한 기존 ADR 0087 및 새 ADR 등록으로 생성된 .codex/config.toml과 harness/claude-hooks/hashes.json뿐이다.

기존 ports.rs의 FakeBroker에 필수 네 메서드를 추가하고 새 테스트를 추가했다. 기존 ports_a21_contract.rs의 a21_contract_table_matches_market_trait_signatures 기대 목록만 정확히 14→18개로 늘렸다. 기존 단언의 삭제·완화는 없다. domain 테스트는 초기에 구현과 함께 작성했으며 선작성 red 증거는 없다. 포트 서명 대조는 구현 전 실패를 실제 확인했다.

## 리뷰 수정 결정

ADR 0068의 보존 오프셋 날짜 비교 관례는 같은 순간의 UTC·KST 표현에 따라 국내 기준일 수락이 달라진다. D-44에서는 비교 날짜만 고정 KST(+09:00)로 구한다. 원래 updatedAt·Clock 시각과 오프셋은 바꾸지 않는다. 기존 InvestorFlow와 ADR 0068의 변경은 D-44 범위 밖이며 T-10과 메인에게 같은 의미 검토를 인계한다. 날짜 표현 범위를 넘으면 InvalidFlow다.

정수 값인 5.0은 스펙의 decimal에 맞으므로 거부하지 않는다. 주·순수량·원 표시만 정규화하며 값은 보존한다. 공통 SupplyRate는 0~1의 정확한 값을 보존하고 필드마다 다른 소수 자릿수를 하나의 전역 제한으로 만들거나 반올림하지 않는다. T-10은 공매도 거래량 비중 5자리·금액 비중 4자리라는 서버 계약을 필드별 응답 테스트로 확인한다.

T-10/A-09/G-07/M-04는 당일 프로그램 값이 장중에 바뀔 수 있음을 보존하고 날짜가 지났다는 이유만으로 확정으로 승격하지 않는다. 확정성은 Unknown이며 이를 상수 필드로 중복하지 않는다.

자동 just mutants-diff는 실제 실행했으나 사전 승인된 기존 impl·기대 목록 변경 두 건을 test-diff가 거부하여 변이 실행 전에 차단됐다. 자동 검출·생존 0의 증거가 아니다. 하네스는 바꾸거나 우회하지 않았고, 직접 실행한 손 변형과 구별한다.
