# A-09 시장 개요 조회

- 상태: 초안

## 결정

기준은 904c78794d4e3d10d62777fe0b3ffe3112b9f20d다. GetMarketOverview는 명시적인 조회 목록을 순서대로 한 번씩 읽는다. 국내 지수·국채는 MarketInfo.indicator_quotes, ETF 근사는 BrokerRead.quotes, 일별 값은 MacroSeries.daily_values, 환율과 장 운영 정보는 기존 MarketInfo 포트를 쓴다. 새 포트와 가짜 메서드·의존성은 추가하지 않는다. 조회별 Result와 원 오류를 그대로 보존하여 일부 실패 뒤에도 다음 조회를 수행한다. NotConfigured·NoData·빈 성공 목록은 각각 유지한다. 저장·이벤트·재시도·주문 전송은 없다.

ETF 가격은 지수 포인트나 금리 퍼센트로 바꾸지 않고 EtfApproximation 변형에 담는다. MacroObservation의 MacroSource(UsTreasury/Fred/TossEtf)·단위·기준일·계열 식별자는 그대로 보존한다. 어떤 ETF가 어떤 지수·금리를 대신하는지의 카탈로그와 선택은 호출자의 공급 계약이다. 유스케이스는 추종 관계를 인증하지 않는다.

ACC-4의 5분은 domain Timestamp.elapsed_until과 TimeSpan 비교로 판정한다. 정확히 300초는 WithinFiveMinutes, 초과는 OlderThanFiveMinutes이며 장 운영 상태와 독립된 사실이다. 화면은 장중 여부와 함께 표시한다. 서버 시각이 없거나 미래 시각이면 Unknown이며 수집 시각으로 원 시각을 메우지 않는다. 원 서버/수집 시각은 값에 보존한다. MacroSeries와 calendar에는 원 수집 시각 필드가 없으므로 주입 Clock의 응답 수신 시각을 별도 관측 시각으로 붙인다. 환율도 원 수집 시각 대신 응답 수신 시각을 보존하며 as_of는 원 값 안에 있다.

## 정의 대기

| 기준 부분 | 이유 | 현재 결과 |
|---|---|---|
| 일별 지표의 오래됨 판정 | 거래일·발표 주기별 신선도 규칙/도메인 함수가 없음 | DailyPolicyPending, 원 기준일 보존 |
| calendar 원 수집 시각 및 오래됨 | 기존 타입에 원 수집 시각이 없음 | Unknown, 응답 수신 시각 보존 |
| ETF 추종 대상 기본 카탈로그 | 봉인된 매핑 타입이 없음 | 호출자가 명시한 QuoteInstrument만 조회 |

## 인계

- D-37/N-16: 일별 발표 주기와 휴장일을 반영한 신선도 도메인 계약, MacroObservation의 원 수집 시각.
- T-08/M-04: ETF 추종 카탈로그 검증, 포트 응답과 요청 식별자 결합, 원 시각 보존. ETF 가격을 지수·금리로 오해하지 않게 표시.
- U-02/G-07: OverviewFreshness 및 출처를 화면 DTO로 옮긴다. DailyPolicyPending은 내부 이름을 화면에 노출하지 않고 기준일을 표시한다. 장중 시세의 5분 초과 표시.

- H-41: Opus 리뷰 재현 파일 쓰기의 cwd-unknown 훅 거부를 실행 환경에서 확인한다. 보호 규칙을 약하게 만들지 않는다.
- H-47: 기존 Git 훅 회귀 하위 명령의 signal 9 종료는 원인/종료 주체 미확인이다. 단독 재실행과 전체1730개 재실행은 통과했으나 이를 원인 해소로 해석하지 않는다.

## 검증 경계

프로젝트 설정 hooks=true를 확인했으나 프로젝트 신뢰와 훅 신뢰 저장 상태 및 런타임 적용은 확인 불가다. 정상 Git 훅 설치를 확인했다. 실제 키·실제 계좌·실제 API는 사용하지 않았다. 기존 테스트 변경, 검사 끄기, 의존성 추가 없음.

## 리뷰 평가와 표시 계약

WithinFiveMinutes는 원 자료 시각에서 300초 이내라는 사실이며, 환율이 현재 환전에 유효하다는 뜻이 아니다. FxRate의 as_of·valid_until·validity_at은 반환 값에 그대로 있다. 유효기간 [800,900), 조회 시각 1000이면 경과 시간 200초와 OutsideInterval이 함께 성립한다. U-02/G-07은 5분 상태만으로 환율을 사용 가능하다고 표시하지 않는다. 유효성까지 단일 신선도에 합치는 계약은 정의 대기이며 D-10/T-08/G-07에 인계한다.

2초까지의 시계 차이를 허용해 원 자료를 받는 것과 미래 시각의 신선도를 확정하는 것은 별개다. 1ns 앞선 시각도 Unknown으로 보존한다. 300초 상수는 ACC-4에 이미 정해진 표시 기준이고 시간 계산은 기존 domain 연산만 사용한다. 현재 Timestamp는 1~9999년의 검증된 값이고 고정된 300초·0ns의 TimeSpan 생성은 유효하다. 구조상 도달하지 않은 시간 오류의 추가 분류는 D-10/G-07에 인계하며, 새로운 domain 정책은 여기서 정하지 않는다.

새 Opus 리뷰의 6개 추정 지적을 새 Codex가 독립 비판했다. 제품 결함 확정 0건, 기각 4건, 불확실 2건이다. ETF 적격성과 추종 관계는 기존 QuoteInstrument가 인증하지 않으며 정의 대기로 남긴다. 요청/응답 결합 검증은 ADR 0082에 따라 T-07/T-08이 소유한다. 원 수집 시각이 없는 응답의 수신 시각은 원 수집 시각이 확인되었다는 주장이 아니다. U-02/G-07은 이 구분을 유지한다. 상세 실행 증거와 평가 및 리뷰어 훅 제한은 docs/reviews/A-09.md에 기록한다.

## 손 변형 실제 실행

mktemp 복사본에서 최종 제품 소스와 통합 테스트로 기준 통과, 각 변형 실행, 원복 통과를 확인했다. 컴파일 성공 21개 모두 검출, 생존 0개다. 컴파일 불가 5개와 최종 소스에 적용되지 않은 2개는 검출 수에 넣지 않았다. 개별 로그와 집계는 target/A-09/hand-*.log 및 hand-results.json이다.

| 번호 | 바꾼 것 | 잡은 테스트 또는 상태 |
|---|---|---|
| 1 | 5분 포함 경계 | test tests::fx_calendar_and_etf_failures_retain_original_facts ... FAILED, test tests::fx_freshness_uses_original_as_of_instead_of_receive_time ... FAILED, test tests::indicators_preserve_values_times_and_five_minute_boundary ... FAILED, test result: FAILED. 3 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 2 | 오래됨 반전 | test tests::fx_freshness_uses_original_as_of_instead_of_receive_time ... FAILED, test tests::mixed_results_preserve_sources_absence_errors_and_request_order ... FAILED, test tests::indicators_preserve_values_times_and_five_minute_boundary ... FAILED, test result: FAILED. 3 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 3 | 5분을299초 | test tests::fx_freshness_uses_original_as_of_instead_of_receive_time ... FAILED, test tests::indicators_preserve_values_times_and_five_minute_boundary ... FAILED, test tests::fx_calendar_and_etf_failures_retain_original_facts ... FAILED, test result: FAILED. 3 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 4 | 5분을301초 | test tests::fx_freshness_uses_original_as_of_instead_of_receive_time ... FAILED, test tests::mixed_results_preserve_sources_absence_errors_and_request_order ... FAILED, test tests::indicators_preserve_values_times_and_five_minute_boundary ... FAILED, test result: FAILED. 3 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 5 | 오래됨을최신 | test tests::indicators_preserve_values_times_and_five_minute_boundary ... FAILED, test tests::mixed_results_preserve_sources_absence_errors_and_request_order ... FAILED, test tests::fx_freshness_uses_original_as_of_instead_of_receive_time ... FAILED, test result: FAILED. 3 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 6 | 최신을오래됨 | test tests::fx_freshness_uses_original_as_of_instead_of_receive_time ... FAILED, test tests::fx_calendar_and_etf_failures_retain_original_facts ... FAILED, test tests::indicators_preserve_values_times_and_five_minute_boundary ... FAILED, test result: FAILED. 3 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 7 | 미래시각을최신 | test tests::indicators_preserve_values_times_and_five_minute_boundary ... FAILED, test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 8 | 시각없음을최신 | test tests::indicators_preserve_values_times_and_five_minute_boundary ... FAILED, test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 9 | 결과역순 | 최종 소스 미적용(집계 제외) |
| 10 | 요청누락 | test tests::fx_calendar_and_etf_failures_retain_original_facts ... FAILED, test tests::indicators_preserve_values_times_and_five_minute_boundary ... FAILED, test tests::macro_sources_and_every_availability_are_preserved ... FAILED, test tests::mixed_results_preserve_sources_absence_errors_and_request_order ... FAILED, test result: FAILED. 2 passed; 4 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 11 | 모든결과누락 | 최종 소스 미적용(집계 제외) |
| 12 | 지수요청교환 | 컴파일 불가(집계 제외) |
| 13 | 지수시각수집으로 | test tests::indicators_preserve_values_times_and_five_minute_boundary ... FAILED, test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 14 | 지수수집시각교체 | test tests::indicators_preserve_values_times_and_five_minute_boundary ... FAILED, test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 15 | 지수반환누락 | test tests::indicators_preserve_values_times_and_five_minute_boundary ... FAILED, test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 16 | ETF요청누락 | 컴파일 불가(집계 제외) |
| 17 | ETF시각교체 | test tests::mixed_results_preserve_sources_absence_errors_and_request_order ... FAILED, test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 18 | ETF수집시각교체 | test tests::mixed_results_preserve_sources_absence_errors_and_request_order ... FAILED, test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 19 | ETF오류최신 | test tests::fx_calendar_and_etf_failures_retain_original_facts ... FAILED, test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 20 | 계열날짜교환 | test tests::mixed_results_preserve_sources_absence_errors_and_request_order ... FAILED, test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 21 | 미설정데이터없음 | test tests::macro_absence_and_errors_are_distinct ... FAILED, test tests::mixed_results_preserve_sources_absence_errors_and_request_order ... FAILED, test result: FAILED. 4 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 22 | 데이터없음미설정 | test tests::macro_absence_and_errors_are_distinct ... FAILED, test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 23 | 일별최신표시 | test tests::macro_sources_and_every_availability_are_preserved ... FAILED, test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 24 | 환율종류교체 | 컴파일 불가(집계 제외) |
| 25 | 환율통화쌍교체 | 컴파일 불가(집계 제외) |
| 26 | 환율시각교체 | test tests::fx_freshness_uses_original_as_of_instead_of_receive_time ... FAILED, test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
| 27 | 캘린더시장교체 | 컴파일 불가(집계 제외) |
| 28 | 캘린더최신표시 | test tests::fx_calendar_and_etf_failures_retain_original_facts ... FAILED, test result: FAILED. 5 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s |
