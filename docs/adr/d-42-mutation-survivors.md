# D-42 병합된 domain 변이 미검출 보강

- 상태: 초안

## 결정

기준은 `8ba1f6abf3b1050017b649d1d1a5fd63104a6363`이다. 현행 D-29·D-37 대상 8파일 전체의 515개 변이를 먼저 실행한 뒤 테스트만 추가한다. 기존 테스트 본문과 제품 동작은 변경하지 않는다. 과거 H-46의 건수 대신 이 기준의 실제 목록을 사용한다.

설치된 cargo-mutants 27.1.0의 `--help`와 justfile 및 xtask 레시피를 확인했다. `--no-config -p domain -f <파일명>`으로 파일을 한정한다. 설정의 examine_globs와 파일 옵션을 함께 사용하면 전체 대상이 선택되므로 `--no-config`와 필수 `--cap-lints=true --test-tool=nextest`를 명시했다. 파일별 출력 경로를 분리하고 실행당 1,195초 뒤 소유 프로세스 그룹만 종료하도록 했다. 모든 기본 실행이 제한 안에 완주했다.

변이는 원본 domain 전체 900개 테스트로 측정했다. `STOCK_COPILOT_PROPTEST_CASES`를 제거하여 기본 256회를 사용했다. 프로필·병렬도는 xtask와 같은 debug=0, opt-level=1, debug assertions 및 overflow checks 활성화, 빌드 작업 수1, 변이 작업6개, 테스트 스레드2개다. 로그와 JSON은 작업 사본의 `target/D-42/`에 있다.

## 전후 측정

순서는 검출 / 미검출 / 시간 초과 / 컴파일 불가다. 컴파일 불가를 검출로 세지 않는다.

| 파일 | 전 | 후 |
|---|---|---|
| bootstrap.rs | 39 / 1 / 3 / 16 | 40 / 0 / 3 / 16 |
| signal_return.rs | 34 / 1 / 0 / 32 | 35 / 0 / 0 / 32 |
| signal_benchmark.rs | 16 / 2 / 0 / 7 | 18 / 0 / 0 / 7 |
| virtual_exit.rs | 87 / 6 / 0 / 42 | 93 / 0 / 0 / 42 |
| market_ranking.rs | 47 / 1 / 0 / 25 | 48 / 0 / 0 / 25 |
| investor_flow.rs | 39 / 2 / 0 / 28 | 39 / 2 / 0 / 28 |
| market_index.rs | 32 / 0 / 0 / 32 | 32 / 0 / 0 / 32 |
| sector_composition.rs | 9 / 1 / 0 / 13 | 10 / 0 / 0 / 13 |
| 합계 | 303 / 14 / 3 / 195 | 315 / 2 / 3 / 195 |

## 동치

위치는 기준 SHA의 줄이다. 허용 목록이나 변이 설정을 변경하지 않는다.

| 파일:줄 | 변이 | 관측 동치의 이유 |
|---|---|---|
| investor_flow.rs:462 | validate_institution 첫 `||`를 `&&`로 치환 | 원식은 `x || y || z`, 변이식은 `(x && y) || z`다. x는 기관 매수 합계 차이, y는 매도 차이, z는 순매수 차이다. 모든 행의 순매수는 정수 매수−매도로 검증되며, 합계의 checked 산술 오류는 비교 전에 반환된다. z가 거짓이면 두 차액이 같으므로 x와 y의 진릿값도 같다. z가 참이면 두 식 모두 참이다. 따라서 모든 도달 가능한 입력에서 동일 오류 또는 동일 사실을 반환한다. 중복 조건이며 제품 코드는 유지한다. |
| investor_flow.rs:463 | validate_institution 둘째 `||`를 `&&`로 치환 | Rust 연산자 우선순위에 따라 변이식은 `x || (y && z)`다. x가 참이면 두 식 모두 참이다. x가 거짓이면 매수 합계가 같으므로 매도 합계 차이와 순매수 차이는 동시에 존재하거나 동시에 없다. 따라서 y와 z의 진릿값이 같아 두 식이 동일하다. 첫 변이와 마찬가지로 산술 오류는 조건 전에 처리된다. |

## 시간 초과

| 파일:줄 | 변이 | 원인과 처리 |
|---|---|---|
| bootstrap.rs:180 | Generator::index `>` → `==` | 기존 65,537개 묶음 입력이 거부되지 않는다. ceiling=0이 되어 어떤 후보도 수용하지 못한다. |
| bootstrap.rs:190 | 후보 `<` → `==` | 묶음 수가 2의 거듭제곱이면 ceiling=65,536으로 u16 후보의 범위 밖이다. 종료 불가다. |
| bootstrap.rs:190 | 후보 `<` → `>` | 같은 입력에서 ceiling보다 큰 u16 후보가 없어 종료 불가다. |

셋 모두 원본 테스트가 이미 해당 입력을 실행한다. 반복은 테스트 바깥의 동기 함수 안에 있어 테스트의 반복 상한 단언으로 중단할 수 없다. 제품 함수에 계측 인자를 추가하거나 분기를 바꾸지 않고, 실행기 시간 제한으로 구분한다. 기본 실행은 각각 테스트 제한144초에서 시간 초과했다. 시간 초과를 검출 성공이나 동치로 세지 않는다. 이 작업의 구체 지시인 원인·불가능 사유 기록을 적용한다.

## 인계

- D-05: 기존 portfolio_valuation.rs의 assert_independent_rate ZeroCost 분기는 무작위0 수량 표본에 의존한다. 전체 검사에서 한 번 미측정됐고 후속4,096회 실행에서는 측정됐다. 고정0 수량 예제로 계측을 결정적으로 만드는 보강은 D-05 범위로 넘긴다. 기존 테스트는 수정하지 않았다.
- H-47: 하네스의4초 PID 테스트는 패키지 캐시 잠금 대기 중 child.pid가 생성되기 전에 종료될 수 있었다. 변이 실행을 종료한 단독 재현에서는 통과했다. 날짜별 전체 변이의 완주·시간 초과 정책과 함께 위 무한 반복 변이를 추적한다. 이 작업은 반복 중단을 위한 제품 구조 변경을 하지 않는다.
- D-42 병합 담당: 수급의 중복 조건을 향후 정리할 때 위 정수 관계를 유지한다. 기존 제품 코드·기존 테스트 본문 변경은 없다.
- 프로젝트 설정 `hooks=true`와 lefthook 설치를 확인했다. 사용자별 프로젝트·훅 신뢰 실제 적용 상태는 홈 설정을 읽지 않아 검증하지 못했으며 실제 검사 실행과 Git 훅 결과를 별도 근거로 사용한다.

## 검사와 리뷰

- 새 예제 테스트11개 집중 실행:11 passed. domain 전체 대상 clippy 통과. 검사 끄기·외부 의존성 추가 없음. 기존 테스트 본문은 변경하지 않았다.
- 손 변형 결과에 따라 만기일 장중 분봉 관측을 보강했다. 최종 자동 전수의 virtual_exit는 이 단언을 포함한다. 이후 도우미의 전체 경로를 use로 줄였으며 단언과 입력은 같다.
- 자동 전후 각각515개의 변이 이름 집합이 파일별로 같은지 JSON으로 대조했다. 최종 미검출2개는 위 동치 목록과 정확히 같다. 동치를 제외한 미검출0, 시간 초과3이다. bootstrap 최종 테스트 시간 제한은85초였고 셋 모두 같은 무한 반복으로 종료됐다.
- signal_benchmark의 첫 재실행은 커밋 훅의 의존성 재설치와 파일 복사가 겹쳐 I/O 오류로 종료1이었다. 이를 전수 결과로 세지 않고, 재설치가 끝난 뒤25개 전체를 다시 실행해 종료0을 얻었다. 다른 모든 파일도 실행당20분 안에 완주했다.
- `just mutants-diff 8ba1f6abf3b1050017b649d1d1a5fd63104a6363`: 종료0, 선택 변이0개. application·domain·order-execution을 밤 전체 대상으로 알렸다. 테스트 추가만 있는 변경분의 선택 결과이므로 검출력 증거로 세지 않는다.
- 정상 커밋 훅 첫 실행은 새 테스트63줄이60줄 제한을 넘어서 실패했다. 도우미 경로 정리로 해결하고 clippy를 통과했다. 둘째 실행은 전체1,304개 중1,219개 통과·1개 실패·84개 미실행이었다. 기존 하네스의 deadline_stops_nextest_child_groups_and_preserves_unrelated_pid가 child.pid를 읽지 못했고 패키지 캐시 잠금 대기가 있었다. 하네스·기존 테스트를 바꾸지 않고 변이 실행 종료 뒤 단독 재현과 정상 훅으로 확인한다.
- 새 ADR 등록을 위해 `.codex/config.toml`과 `harness/claude-hooks/hashes.json`을 저장소 스크립트로 재생성했다. `harness/sandbox/fragment.mjs --write`도 실행했으며 결과 변경은 없었다. 보호 규칙은 완화하지 않았다.
- 변이 실행 종료 뒤 기존 하네스 PID 테스트를 단독 실행했다: 1 passed, 296 skipped, 종료0. 하네스와 기존 테스트는 수정하지 않았다.
- 셋째 정상 훅은 전체1,304개 테스트를 통과했다. 커버리지 테스트956개도 통과했지만 새 테스트의 실패 전용 panic 줄3곳과 기존 portfolio_valuation.rs:1576이 미실행으로 보고돼 커버리지 판정이 실패했다. 새 테스트는 결과 종류·내용을 직접 단언하는 형태로 정리했다. 기존 포트폴리오 속성 테스트는0 수량을 생성할 수 있으나 고정 예제가 없어 해당 분기의 측정 여부가 표본에 의존한다. 이 테스트를 수정하지 않았다.
- 단언 정리 후 `cargo clippy --locked -p domain --all-targets -- -D warnings` 통과. `STOCK_COPILOT_PROPTEST_CASES=4096 just coverage-check`:956 passed, `coverage-check: 위반 없음`, 종료0. 커버리지 예외나 정책 변경은 없다.
- 최종 단언 형태에서도 손 변형22개를 모두 재실행했다:21개 검출,1개 동치 생존, 컴파일 실패0, 시간 초과0. 원본 집중 테스트11개 통과 뒤 실행했으며 임시 복사본 복원을 바이트 비교로 확인했다.
- 지정 리뷰·비판, 정상 훅 결과는 후속 기록한다.

## 손 변형

임시 복사본에서 실행 후 변형별 결과를 기록한다. 컴파일 실패와 실행하지 않은 변형은 성공으로 세지 않는다.

손 변형 최종 실행은 원본 집중 테스트11개가 통과한 임시 복사본에서21개를 재실행하고, 기관의 순매수만 비교하는 변형1개를 추가 실행했다. 합계22개 중21개는 새 테스트 실패로 검출됐고,1개는 위에 증명한 수급 동치 변이로 생존했다. 컴파일 실패0, 시간 초과0이다. 초기 실행에서 매도 분봉 변이2개가 생존해 만기일 장중 단언을 추가한 뒤21개 전부 재실행했다. 최종 실행 후 복사본의 변형 대상 소스가 원본과 같은지 바이트 비교했다. 전수 변이 후에는 같은 입력·기대값을 유지하며 도우미 경로를 use로 줄이고, 커버리지에 잡힌 실패 전용 분기를 직접 단언으로 정리했다. 최종 테스트 형태에서도22개 전부 재실행해 같은 검출·동치 결과를 확인했다.

| 번호 | 변형 | 결과 | 검출 테스트 |
|---|---|---|---|
| 1 | crates/domain/src/bootstrap.rs:41:9: replace WithheldCount::as_usize -> usize with 1 | 컴파일 성공·검출 | signal_return_review::tests::d42_multiple_missing_comparisons_preserve_the_withheld_count |
| 2 | crates/domain/src/investor_flow.rs:463:9: replace &#124;&#124; with && in validate_institution | 컴파일 성공·동치 생존 | 동치 표 investor_flow.rs:463 |
| 3 | crates/domain/src/market_ranking.rs:305:36: replace < with <= in ranking_item | 컴파일 성공·검출 | market_ranking::review_tests::d42_zero_amount_and_volume_are_available_ranking_facts |
| 4 | crates/domain/src/sector_composition.rs:134:44: replace < with <= in sector_amount | 컴파일 성공·검출 | sector_composition::tests::d42_zero_sector_money_remains_available_and_distinct_from_absence |
| 5 | crates/domain/src/signal_benchmark.rs:26:28: replace > with >= in SignalBenchmark::from_candles | 컴파일 성공·검출 | signal_benchmark::tests::d42_calendar_limit_accepts_4097_consistent_facts |
| 6 | crates/domain/src/signal_benchmark.rs:51:39: replace > with >= in SignalBenchmark::from_candles | 컴파일 성공·검출 | signal_benchmark::tests::d42_distinct_settlement_days_can_share_the_previous_market_close |
| 7 | crates/domain/src/signal_return.rs:449:9: replace &#124;&#124; with && in validate_rates | 컴파일 성공·검출 | signal_return_review::tests::d42_each_combined_cost_side_has_an_independent_inclusive_limit |
| 8 | crates/domain/src/virtual_exit.rs:365:56: replace && with &#124;&#124; in evaluate_rows | 컴파일 성공·검출 | virtual_exit::d42_tests::d42_daily_selection_uses_the_current_trading_day<br>virtual_exit::d42_tests::d42_sell_minutes_and_daily_rows_wait_for_the_recorded_maturity |
| 9 | crates/domain/src/virtual_exit.rs:367:74: replace < with > in evaluate_rows | 컴파일 성공·검출 | virtual_exit::maturity_tests::d42_elapsed_intraday_maturity_still_uses_the_minute_close |
| 10 | crates/domain/src/virtual_exit.rs:428:91: replace == with != in entry_close | 컴파일 성공·검출 | virtual_exit::d42_tests::d42_regular_session_intraday_entry_is_not_a_close |
| 11 | crates/domain/src/virtual_exit.rs:450:34: replace == with != in daily_exit | 컴파일 성공·검출 | virtual_exit::d42_tests::d42_sell_minutes_and_daily_rows_wait_for_the_recorded_maturity |
| 12 | crates/domain/src/virtual_exit.rs:551:21: replace == with != in scan_minutes | 컴파일 성공·검출 | virtual_exit::d42_tests::d42_sell_minutes_and_daily_rows_wait_for_the_recorded_maturity |
| 13 | crates/domain/src/virtual_exit.rs:551:28: replace && with &#124;&#124; in scan_minutes | 컴파일 성공·검출 | virtual_exit::d42_tests::d42_sell_minutes_and_daily_rows_wait_for_the_recorded_maturity |
| 14 | 보류 건수 상수0 | 컴파일 성공·검출 | signal_return_review::tests::d42_multiple_missing_comparisons_preserve_the_withheld_count |
| 15 | 비교 불가 건수 두 배 증가 | 컴파일 성공·검출 | signal_return_review::tests::d42_multiple_missing_comparisons_preserve_the_withheld_count |
| 16 | 비교 불가/근사 건수 바꿈 | 컴파일 성공·검출 | signal_return_review::tests::d42_multiple_missing_comparisons_preserve_the_withheld_count |
| 17 | 청산 가격을 진입 가격으로 교체 | 컴파일 성공·검출 | virtual_exit::d42_tests::d42_daily_selection_uses_the_current_trading_day<br>virtual_exit::d42_tests::d42_sell_minutes_and_daily_rows_wait_for_the_recorded_maturity<br>virtual_exit::maturity_tests::d42_elapsed_intraday_maturity_still_uses_the_minute_close |
| 18 | 시가총액을 거래대금으로 교체 | 컴파일 성공·검출 | sector_composition::tests::d42_zero_sector_money_remains_available_and_distinct_from_absence |
| 19 | 거래대금을 시가총액으로 교체 | 컴파일 성공·검출 | sector_composition::tests::d42_zero_sector_money_remains_available_and_distinct_from_absence |
| 20 | 거래량을 거래대금 입력으로 교체 | 컴파일 성공·검출 | market_ranking::review_tests::d42_zero_amount_and_volume_are_available_ranking_facts |
| 21 | 거래대금을 거래량 입력으로 교체 | 컴파일 성공·검출 | market_ranking::review_tests::d42_zero_amount_and_volume_are_available_ranking_facts |
| 22 | 기관 검증을 순매수 합계만 비교하도록 축소 | 컴파일 성공·검출 | investor_flow::tests::d42_equal_net_does_not_excuse_wrong_buy_and_sell_totals |
