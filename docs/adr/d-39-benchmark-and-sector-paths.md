# D-39·D-40·D-41 벤치마크와 섹터 조회 경로

- 상태: 초안

## 결정

### D-39 지수 일봉

D-37의 응답 생성자는 일봉을 Unknown으로 둔다. 캔들의 필수 시작 시각은 원 응답의 Timestamp로 보존하고, 지표 현재가에도 쓰이는 nullable 관측 시각과 구별한다. finalize_index_day는 같은 KST 날짜의 국내 정규장 세션이 있고 종료 시각이 수집 시각보다 엄격히 앞설 때만 KOSPI/KOSDAQ 일봉을 Finalized로 봉인한다. 세션 종료와 수집이 같은 시각이면 거부한다. 장 종료의 실제 공급 근거는 T-08 책임이며, 단순히 시작 시각이 과거라는 이유로 확정하지 않는다.

index_benchmark_series는 확정 지수 일봉만 받아 D-09의 checked 포인트 비율 수익률을 재사용한다. 지수 포인트를 Money·Price로 바꾸지 않는다. 동일 지수·유일한 KST 날짜·전체 스냅샷 가격일 대응을 요구하며 혼합 지수, 진행 중/미확정, 분봉, 금리, 누락과 날짜 중복을 거부한다. 반환 날짜는 전체 결제일을 유지한다. 입력 4097개 상한과 최소 두 스냅샷은 D-09와 같다. Native USD는 거부하고 Native KRW/ConvertedKrw만 허용한다.

SignalBenchmark::from_candles는 길이 검증 결과에 기존 zip·봉인 블록을 map으로 연결한다. 검증이 성공한 경우에만 기존 계산을 실행하며 다른 계산·입력 정책은 바꾸지 않는다. 관측 수+1과 지점 수가 다르면 BenchmarkMismatch이며 양방향 불일치와 빈 지점을 회귀로 검사한다. 기존 캔들 생성자와 그 테스트는 보존한다. 레거시 Candle 기반 국내 지수 입력의 식별 인증은 기존 ADR 0062의 어댑터 책임이 남으며 새 조회 소비자는 지수 전용 경로를 사용한다.

### D-40 섹터 선택

구성 조회(SectorComposition)와 종목 마스터의 시장·심볼이 일치한 행만 후보가 된다. design의 섹터 상한·쏠림 절과 ADR 0049에는 다중 소속 선택 규칙이 없어, 지시문의 기본값인 LargestCompositionWeightThenSectorId를 채택한다. API가 별도 구성 비중을 주지 않으므로 같은 구성 조회의 해당 종목 시가총액 / 구성 전체 시가총액 합을 구성 비중으로 정의한다. 거래대금을 분모로 섞지 않는다. nullable 시가총액이 하나라도 없거나 합계가 0이면 MissingWeight다. 합을 정확히 표현할 수 없으면 Arithmetic이며 반올림한 값을 선택에 쓰지 않는다.

후보 간 비중은 정확한 정수 계수의 교차 곱으로 비교한다. 기존 exact_arithmetic의 384비트 Magnitude를 재사용하는 자유 함수만 추가한다. 양의 96비트 계수 두 개의 곱(192비트 미만)에 최대 소수 56자리 정렬(187비트 미만)을 더해도 384비트 안이다. 이 범위에서 최상위 캐리는 항상 0이므로 전용 정수 자리 이동은 실패하지 않는 비교 연산이다. 범용 fallible 이동의 불가능한 오류를 반복 전파하지 않는다. 분자·분모의 스케일이 서로 다른 예와 MAX 계수의 캐리를 독립된 동률 예로 검사한다. 근사 나눗셈으로 동률을 만들지 않는다. 동률은 SectorId의 기존 Ord(대소문자를 구분한 식별자 순)로 작은 값을 택한다. 입력 순서와 무관하다.

SectorAssignment::new는 crate 내부로 닫고 조회 식별과 기존 주문 식별을 내부 AssignmentIdentity 규약으로 받는다. 주문 전송 식별을 새로 만들거나 노출하지 않는다. 공개 생성 경로는 sector_assignment_from_compositions다. 반환값의 Single/Multiple 표지를 SectorAssignments로 수집한 뒤에도 별도 대응표로 보존한다. Unspecified는 내부 레거시 생성 결과의 표지이며 새 공개 경로는 반환하지 않는다. 기존 테스트 본문·단언은 바꾸지 않는다. 생성자가 crate 밖에서 쓰인 테스트는 현재 없다. 기존 impl의 최소 변경은 생성자 폐쇄·메타데이터 초기화와 목록의 메타데이터 수집뿐이다.


### D-41 추가 조회 사실

구현 예정.

## 검증

실계좌·실제 API·새 의존성·자동 변이 사용 없음. 직접 전체 검사는 반복하지 않고 정상 커밋 훅의 검사를 받는다. 프로젝트 설정과 훅의 세션 신뢰 적용 여부는 별도 런타임 증거가 없으므로 Git 훅의 실제 결과와 구분한다.

## 인계

- T-08: 실제 지수 일봉의 날짜와 장 종료 증거를 조회 세션에 연결한다.
- A-09·A-15·E-08: 국내 지수 벤치마크에는 index_benchmark_series를 사용한다. 종목 거래소에 맞춘 KOSPI/KOSDAQ 선택은 ADR 0072의 후속 계약이다.

## 손 변형

D-39의 최종 손 변형 결과는 아래 표에 기록한다. D-40·D-41은 해당 커밋에서 덧붙인다.

### D-39 손 변형 결과

집중 benchmark 테스트 37개(새 모듈 9개 포함)가 통과했다. 새 모듈 줄 커버리지 385/385(100%), 신호 벤치마크 줄 커버리지 345/345(100%)이며 두 파일 모두 미실행 코드 영역이 없다. 속성 테스트는 4096회다. 최대 Timestamp의 KST 범위 초과, 최소 양수 지수에서 MAX 지수로 변할 때의 산술 범위 초과도 실제 입력으로 거부한다. 첫 최소 스냅샷 변형 생존은 1개 입력 반례로 수정했다. 아래 13개는 각각 실제 컴파일·실행으로 검출했다.

| 번호 | 바꾼 것 | 검출 테스트 | 결과 |
|---|---|---|---|
| D39-01 | series.observations().count().saturating_add(1) != points.len() -> series.observations().count().saturating_add(1) == points.len() | signal_zip_rejects_both_shorter_and_longer_point_lists | 컴파일 성공·검출 |
| D39-02 | \|\| candle.status() != IndicatorCandleStatus::Finalized -> 삭제 | index_path_rejects_invalid_sources | 컴파일 성공·검출 |
| D39-03 | \|\| candle.period() != CandlePeriod::Daily -> 삭제 | index_path_rejects_invalid_sources | 컴파일 성공·검출 |
| D39-04 | candle.code() != first.code() -> candle.code() == first.code() | index_path_rejects_invalid_sources, index_path_preserves_points_and_rejects_unconfirmed_or_misaligned_inputs, index_range_and_arithmetic_errors_are_closed, index_rejects_a_single_snapshot, index_path_rejects_invalid_period_and_limits, index_returns_match_independent_integer_ratio | 컴파일 성공·검출 |
| D39-05 | IndicatorCode::Kospi => Ok(BenchmarkKind::Kospi) -> IndicatorCode::Kospi => Ok(BenchmarkKind::Kosdaq) | index_path_preserves_points_and_rejects_unconfirmed_or_misaligned_inputs | 컴파일 성공·검출 |
| D39-06 | price_return(*old, *new) -> price_return(*new, *old) | index_range_and_arithmetic_errors_are_closed, index_path_preserves_points_and_rejects_unconfirmed_or_misaligned_inputs, index_returns_match_independent_integer_ratio | 컴파일 성공·검출 |
| D39-07 | currency == PerformanceCurrency::Native(Currency::Usd) -> currency == PerformanceCurrency::Native(Currency::Krw) | index_path_rejects_invalid_sources, index_path_preserves_points_and_rejects_unconfirmed_or_misaligned_inputs | 컴파일 성공·검출 |
| D39-08 | old >= new -> old > new | index_path_rejects_invalid_period_and_limits | 컴파일 성공·검출 |
| D39-09 | candles.len() > 4097 -> candles.len() > 4098 | index_path_rejects_invalid_period_and_limits | 컴파일 성공·검출 |
| D39-10 | snapshots.len() > 4097 -> snapshots.len() > 4098 | index_path_rejects_invalid_period_and_limits | 컴파일 성공·검출 |
| D39-11 | ordered.len() < 2 -> ordered.len() < 1 | index_rejects_a_single_snapshot | 컴파일 성공·검출 |
| D39-12 | regular.session().interval().end() >= candle.observation().collected_at() -> regular.session().interval().end() > candle.observation().collected_at() | finality_rejects_equal_close_and_late_bar_start | 컴파일 성공·검출 |
| D39-13 | start > regular.session().interval().start() -> start < regular.session().interval().start() | index_range_and_arithmetic_errors_are_closed, finality_rejects_equal_close_and_late_bar_start, index_rejects_a_single_snapshot, index_path_rejects_invalid_period_and_limits, index_path_preserves_points_and_rejects_unconfirmed_or_misaligned_inputs, index_path_rejects_invalid_sources, index_returns_match_independent_integer_ratio | 컴파일 성공·검출 |

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
| D40-07 | member.symbol() == symbol -> member.symbol() != symbol | fractional_weights_and_wide_products_preserve_exact_order, largest_weight_wins_ties_use_id_and_multiple_membership_survives, selection_matches_independent_integer_cross_products | 컴파일 성공·검출 |
| D40-08 | 섹터 교차 곱의 십진 자리 이동 배율 10을 9로 변경 | fractional_weights_and_wide_products_preserve_exact_order | 컴파일 성공·검출 |
| D40-09 | 섹터 교차 곱의 앞자리 캐리 기여를 0으로 변경 | fractional_weights_and_wide_products_preserve_exact_order | 컴파일 성공·검출 |

D-40 인계: T-09는 종목의 전체 후보 섹터 조회를 공급한다. Single/Multiple은 전달된 조회 집합에서 관측한 소속 수이며 전체 섹터 조회의 완전성을 인증하지 않는다. 시가총액이 누락되면 비중이나 소속을 추측하지 않는다. A-09는 선택 결과와 다중 소속 표지를 함께 소비한다. ETF·펀드를 구성 주식으로 분해하지 않는 ADR 0043의 원칙은 유지한다.

## H-46 실행 규약

master 8ba1f6abf3b1050017b649d1d1a5fd63104a6363의 H-46을 받았다. 이번 작업의 속성 테스트는 testkit::property_cases()를 사용한다. 기본 실행은 256회, 전체 검사 환경은 4096회이며, 앞의 4096회 및 손 변형 기록은 실제 당시 실행 결과다. domain에는 H-46이 이미 내부 testkit 개발 의존성을 추가했으므로 별도 의존성이나 패키지를 더하지 않는다. 작업 브랜치 검사는 check-fast를 사용하고 검사 계획이 요구하는 승격은 그대로 따른다.
