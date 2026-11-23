# ADR D-58: 추정 수익률의 갭·모호 실패 청산 가격

- 상태: 제안 (2026-11-24)
- 날짜: 2026-11-24
- 결정한 사람: 메인 에이전트(D-54 메인 평가 결정 반영, 추가 선택지는 사용자 확인 대기)
- 관련 작업: D-58
- 기준: 33d41e8dd43556a28d6b6b90f0db52bf879f42cf
- 근거: D-58, D-54, ADR 0051, ADR 0072, ADR 0136, ADR 0151

## 맥락

ADR 0151(D-54)은 추정별 수익률 계산 경로 `domain::estimate_return`을 도입하면서 두 가지 청산 가격 정책을 보류로 두었다.

1. 목표·손절 경계를 넘겨 열린 봉(정규장 시가와 장중 분봉 시가 모두): `EstimateReturnError::GapPricePending`으로 보류했다.
2. 같은 봉에서 목표와 손절에 모두 닿은 확정 모호 실패(`OutcomeState::Failure(OutcomeFailure::Ambiguous)`): `EstimateReturnError::AmbiguousExit`로 보류했다.

D-54 메인 평가는 두 경우에 대해 다음 정책을 확정하고 D-58로 등록했다.

- **갭**: 목표·손절 경계를 넘겨 열린 봉은 그 봉의 시가를 청산 가격으로 쓴다(정규장 시가와 장중 분봉 시가 모두). 실제 체결 가능 가격에 가깝고 손절 갭의 손실을 숨기지 않기 위함이다.
- **같은 봉 모호 실패**: 손절 가격으로 청산한 것으로 계산한다(보수적인 쪽).
- **나머지 보류**: 제출 전 도달, 제출가가 이미 경계 밖, 제출 시각 가격 없음, 같은 분봉·장중 제출일의 일봉, 정확히 환산되지 않는 분할, 제출일·청산일 벤치마크 종가 결측은 그대로 둔다.

## 결정

### 규칙 표

| 항목                                    | 결정                                                                                                                                                                                                                                                                                                                                                                                              | 근거                                                                                                   | 버린 대안                                                              |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------- |
| 목표·손절 갭 청산                       | 정규장 일봉 시가와 장중 분봉 시가 모두 `open >= target`이면 목표 시가(`EstimateExitKind::Target`), `open <= stop`이면 손절 시가(`EstimateExitKind::Stop`)로 청산한다. `EstimateReturnError::GapPricePending` 변형을 제거해 불가능한 보류 상태를 타입에서 없앤다                                                                                                                                   | D-54 메인 평가, ADR 0072 `virtual_exit::opening`                                                       | 경계 가격으로 고정(손절 갭 손실 은폐), `GapPricePending` 유지          |
| 같은 봉 모호 실패                       | `OutcomeState::Failure(OutcomeFailure::Ambiguous)`이고 같은 분봉에서 `virtual_exit`이 `VirtualExit::Stop`을 반환한 경우 `EstimateExitKind::Stop`으로 수익률을 계산한다. 시가가 경계 안쪽이면 조정된 손절 경계 가격(`At(봉 종료)`), 시가가 손절 경계 이하(`open <= stop`)인 손절 갭 동반 모호 실패면 봉 시가(`At(봉 시작)`)를 사용한다. 원래 `EstimateOutcome`(모호 실패)은 결과에 그대로 보존한다 | D-54 메인 평가(보수적인 쪽), 손절 갭 손실 은폐 방지                                                    | `AmbiguousExit`로 전부 보류, 새 `EstimateExitKind` 변형 추가           |
| 분봉 시가 도달과 장중 도달의 시각 구분  | `evidence.period() == Some(CandlePeriod::Minute)`에서 `fill.moment() == At(at)`가 봉 시작(`at + 1분 == evidence.at()`)이면 `submitted_at < at`를 요구하고(정규장 일봉 시가 도달의 `< open`과 일관), 봉 종료(`at == evidence.at()`)이면 기존과 같이 `submitted_at.elapsed_until(at) >= 1분`을 요구한다                                                                                             | D-54 3차 리뷰 3번 인계, 앞 분봉 안의 제출(예: 갭 분봉 시작 30초 전)을 같은 분봉으로 오판하는 결함 제거 | 모든 분봉 청산에 1분 경과 일괄 적용(앞 분봉 제출을 불합리하게 보류)    |
| 분봉 근거와 청산 시각 일치 검증         | `evidence.period() == Some(CandlePeriod::Minute)`일 때 `fill.moment()`가 `At(evidence.at())` 또는 `At(evidence.at() - 1분)`이 아니면 `EstimateReturnError::OutcomeMismatch`를 반환한다                                                                                                                                                                                                            | 일봉 양쪽 도달 뒤 분봉이 모두 미도달해 `virtual_exit`이 다른 날·다른 분봉으로 진행한 모순 자료 차단    | `classify_exit`에서 상태만 대조(다른 거래일의 손절을 잘못 결합할 위험) |
| `Unavailable` 우선 분류                 | `classify_exit`에서 `(_, VirtualExit::Unavailable(reason))`을 `AmbiguousExit`보다 먼저 매칭해, 모호 실패 경로에서도 거래량 0(`MissingCandles`), 비종료 1:3 분할(`Adjustment`), 부분 관측 분봉(`MissingCandles`)의 원래 사유를 유지한다                                                                                                                                                            | 나머지 보류 사유 보존(D-58 범위), 기존 `Stop` 경로와 일관                                              | `AmbiguousExit`로 덮어쓰기(원인 손실)                                  |
| 목표 갭 시가 + 같은 분봉 손절 동시 도달 | `OutcomeState::Failure(OutcomeFailure::Ambiguous)`이면서 `virtual_exit`이 `VirtualExit::Target(open)`을 반환한 경우는 두 결정(갭 시가 목표 청산 vs 모호 실패 손절가 청산)이 충돌하므로 임의로 정하지 않고 `Err(EstimateReturnError::AmbiguousExit)`로 보류하며 아래 정의 대기 표에 제안한다                                                                                                       | D-58 지시(두 결정 밖 선택지는 정의 대기로 제안), `Failure` 판정에 `Target` 수익률을 임의로 붙이지 않음 | 임의로 `Target` 채택 또는 임의로 손절가로 강제 치환                    |

### 상황 → 결과 표

| 상황                                                                      | `EstimateOutcome` 상태                            | `estimate_return` 결과                                                | `exit_kind`       | `exit_price`         | `exit_at`            |
| ------------------------------------------------------------------------- | ------------------------------------------------- | --------------------------------------------------------------------- | ----------------- | -------------------- | -------------------- |
| 일봉 목표 장중 도달(시가 경계 안)                                         | `Success` (`Daily`)                               | `Ok`                                                                  | `Target`          | 조정 목표가          | `TradingDay(청산일)` |
| 일봉 손절 장중 도달(시가 경계 안)                                         | `Failure(Stop)` (`Daily`)                         | `Ok`                                                                  | `Stop`            | 조정 손절가          | `TradingDay(청산일)` |
| 일봉 목표 갭 시가(`open >= target`, 저가 > 손절)                          | `Success` (`Daily`)                               | `Ok`                                                                  | `Target`          | 일봉 시가            | `At(정규장 시작)`    |
| 일봉 손절 갭 시가(`open <= stop`, 고가 < 목표)                            | `Failure(Stop)` (`Daily`)                         | `Ok`                                                                  | `Stop`            | 일봉 시가            | `At(정규장 시작)`    |
| 일봉 양쪽 도달 + 첫 분봉 목표만 도달(일봉 목표 갭)                        | `Success` (`Minute`)                              | `Ok`                                                                  | `Target`          | 일봉 시가            | `At(정규장 시작)`    |
| 일봉 양쪽 도달 + 첫 분봉 손절만 도달(일봉 손절 갭)                        | `Failure(Stop)` (`Minute`)                        | `Ok`                                                                  | `Stop`            | 일봉 시가            | `At(정규장 시작)`    |
| 분봉 목표 갭 시가(`open >= target`, 저가 > 손절)                          | `Success` (`Minute`)                              | `Ok`                                                                  | `Target`          | 분봉 시가            | `At(분봉 시작)`      |
| 분봉 손절 갭 시가(`open <= stop`, 고가 < 목표)                            | `Failure(Stop)` (`Minute`)                        | `Ok`                                                                  | `Stop`            | 분봉 시가            | `At(분봉 시작)`      |
| 같은 분봉 양쪽 도달(시가 경계 안)                                         | `Failure(Ambiguous)` (`Minute`)                   | `Ok`                                                                  | `Stop`            | 조정 손절가          | `At(분봉 종료)`      |
| 같은 분봉 손절 갭 시가 + 목표 동시 도달(`open <= stop && high >= target`) | `Failure(Ambiguous)` (`Minute`)                   | `Ok`                                                                  | `Stop`            | 봉 시가(`<= 손절가`) | `At(분봉 시작)`      |
| 같은 분봉 목표 갭 시가 + 손절 동시 도달(`open >= target && low <= stop`)  | `Failure(Ambiguous)` (`Minute`)                   | `Err(AmbiguousExit)`                                                  | -                 | -                    | -                    |
| 갭 분봉 시작 30초 전 제출(앞 분봉 안, `submitted_at < bar_start`)         | `Success` / `Failure(Stop)` (`Minute`)            | `Ok`                                                                  | `Target` / `Stop` | 분봉 시가            | `At(분봉 시작)`      |
| 갭 분봉 시작 정각·봉 안 제출(`submitted_at >= bar_start`)                 | `Success` / `Failure(Stop)` (`Minute`)            | `Err(EntryTimingPending)`                                             | -                 | -                    | -                    |
| 같은 분봉 안 제출(일반 도달·모호 실패 모두)                               | `Success` / `Failure(Stop\|Ambiguous)` (`Minute`) | `Err(EntryTimingPending)`                                             | -                 | -                    | -                    |
| 장중 제출일의 일봉 도달·제출 전 도달·제출가 경계 밖                       | 각 확정 상태                                      | `Err(EntryTimingPending)`                                             | -                 | -                    | -                    |
| 부분 관측 분봉 도달·거래량 0 봉                                           | `Failure(Stop\|Ambiguous)` / `Success`            | `Err(ExitUnavailable(MissingCandles))`                                | -                 | -                    | -                    |
| 정확히 환산되지 않는 분할(예: 1:3)                                        | 각 확정 상태                                      | `Err(ExitUnavailable(Adjustment))`                                    | -                 | -                    | -                    |
| 일봉 양쪽 도달이나 모든 분봉 미도달(모순 봉)                              | `Failure(Ambiguous)` (`Minute`)                   | `Err(OutcomeMismatch)`                                                | -                 | -                    | -                    |
| 제출일·청산일 벤치마크 종가 결측                                          | 위 계산 가능 상태                                 | `Ok` (순수익률 유지, `excess() == Err(Benchmark(BenchmarkMismatch))`) | 위와 같음         | 위와 같음            | 위와 같음            |
| 만기 도달                                                                 | `Failure(Expired)` (`None`)                       | `Ok`                                                                  | `Maturity`        | 만기 종가            | `At(만기 시각)`      |

## 정의 대기와 선택지

두 확정 결정(갭 → 봉 시가, 같은 봉 모호 실패 → 손절가)만으로 하나로 정해지지 않는 경계 선택지는 임의로 결정하지 않고 보류한 뒤 아래 표로 제안한다.

| 항목                                                                      | 현재 동작                                                                                                                             | 선택지                                                                                                               | 사용자에게 보이는 영향                                                                                                                                                                                                                                          |
| ------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 목표 갭 시가 + 같은 분봉 손절 동시 도달 (`open >= target && low <= stop`) | `Err(EstimateReturnError::AmbiguousExit)`로 보류                                                                                      | A. 시가 갭 우선: `Target`·봉 시가로 청산 / B. 모호 실패 우선: `Stop`·조정 손절가로 청산 / C. 현재 보류 유지          | A는 장 시작 직후 목표가 이상으로 출발한 사실을 반영하지만 성적표 `Miss` 판정에 양의 목표 수익률이 표시된다. B는 실패 판정과 손실 방향이 일치하지만 목표 위로 갭 상승한 봉을 손실로 낮춰 표시한다. C는 충돌을 숨기지 않지만 극단 변동성 표본이 수익률에서 빠진다 |
| 분봉 시가 갭의 봉 시작 정각 제출 (`submitted_at == bar_start`)            | `Err(EstimateReturnError::EntryTimingPending)`으로 보류(`submitted_at < bar_start`만 허용)                                            | A. 현재 보류 유지(일봉 개장 정각 제출과 동일) / B. 정각 제출을 시가 이전으로 간주해 시가 청산 허용                   | A는 경계 안쪽 제출가와 동시각 경계 밖 분봉 시가의 순서를 추측하지 않는다. B는 표본 수가 늘지만 시가 형성 직후 제출에 가짜 수익·손실이 계산될 수 있다                                                                                                            |
| 일봉 시가와 첫 분봉 시가가 서로 다른 모순 원자료(일봉 양쪽 도달일)        | D-27·D-29와 같이 원자료 정합성은 T-07 전제로 두고 `virtual_exit`의 일봉 시가를 사용(다른 분봉·날짜 불일치만 `OutcomeMismatch`로 차단) | A. 현재 유지(T-07 정규화 책임) / B. `estimate_return`에서 일봉 시가와 첫 분봉 시가 일치를 추가 검증해 불일치 시 거부 | A는 정상 피드에서 추가 제약 없이 동작하지만 깨진 외부 피드가 들어오면 일봉 시가가 적용된다. B는 깨진 피드를 도메인에서 차단하지만 일봉·분봉의 수정 기준 미세 차이로 정상 자료가 거부될 수 있다                                                                  |

## 결과

- `crates/domain/src/estimate_return.rs`의 갭 청산과 같은 봉 모호 실패 청산이 순수 도메인 정확 십진 산술(`Price`, `Money`, `Ratio`, `RatioQuotient`)로 완결된다.
- 기존 `GapPricePending` 변형은 제거되고, `AmbiguousExit`는 목표 갭 시가와 같은 분봉 손절 도달이 충돌하는 정의 대기 경로에만 남는다.
- 기존 D-54 테스트 6건(보류 기대값 4건, `adjusted_entry` 인자 축소 2건)이 변경되며, `harness/test-diff/approvals.toml` 반영은 메인에 인계한다.
- 인계:
  - `A-15`·`S-18`: 제출 시점 가격 관측·비용·완전 봉·기업 행사를 `estimate_return`에 연결하고 `AmbiguousExit`·`EntryTimingPending`·`ExitUnavailable` 보류 사유를 보존한다.
  - `E-08`: 갭 시가 청산과 모호 실패 손절가 청산 결과, `AmbiguousExit` 보류 사유, 벤치마크 근사 표지를 성적표·복기에 표시한다.
  - `T-07`: 일봉·분봉 시가와 현지 오프셋의 정합성을 보장한다.
- 바뀌면 다시 볼 조건: 위 정의 대기 표의 세 항목 중 하나라도 사용자가 방향을 확정하면 해당 분기를 갱신한다.

## 1차 구현의 손 변형

`target/D-58/` 아래 격리 사본과 별도 Cargo 출력 디렉터리에서 `RUSTFLAGS="-D warnings"` 조건으로 새·변경 코드의 변형 26건을 하나씩 실제 실행했다. 26건 모두 컴파일 후 테스트 실패로 검출됐다(검출 26/26, 생존 0, 컴파일 불가 0, 시간 초과 0). 실행 뒤 `target/D-58/`을 삭제했다.

| 번호 | 바꾼 것                                                                       | 검출한 시험                                                                                                                                 |
| ---- | ----------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| 1    | `minute_bar_match`: `TradingDay` 분기를 `Matched`로 반환                      | `d58_inconsistent_ambiguous_bars_are_rejected_as_outcome_mismatch`                                                                          |
| 2    | `minute_bar_match`: `At` 분기에서 무조건 `Matched` 반환                       | `d58_inconsistent_ambiguous_bars_are_rejected_as_outcome_mismatch`                                                                          |
| 3    | `minute_bar_match`: 봉 종료 일치 조건(`at == evidence_at`) 제거               | `d54_ambiguous_failure_has_a_distinct_exit_error` 외 11건                                                                                   |
| 4    | `minute_bar_match`: 봉 시작 1분 차 일치 조건 제거                             | `d54_gap_prices_wait_for_the_estimate_policy` 외 7건                                                                                        |
| 5    | `minute_bar_match`: `elapsed_until` 방향 역전                                 | `d54_gap_prices_wait_for_the_estimate_policy` 외 7건                                                                                        |
| 6    | `minute_bar_match`: 1분 정확 일치 대신 임의 양수 경과(`.is_ok()`) 허용        | `d58_inconsistent_ambiguous_bars_are_rejected_as_outcome_mismatch`                                                                          |
| 7    | `minute_bar_match`: 1분 일치 비교를 `!=`로 반전                               | `d54_gap_prices_wait_for_the_estimate_policy` 외 8건                                                                                        |
| 8    | `resolved_exit`: `Minute` 대신 `Daily`에만 `minute_bar_match` 적용            | `d54_gap_prices_wait_for_the_estimate_policy` 외 13건                                                                                       |
| 9    | `resolved_exit`: `minute_bar_match` 불일치 비교를 `==`로 반전                 | `d54_ambiguous_failure_has_a_distinct_exit_error` 외 16건                                                                                   |
| 10   | `resolved_exit`: `minute_bar_match` 거부 분기 삭제                            | `d58_inconsistent_ambiguous_bars_are_rejected_as_outcome_mismatch`                                                                          |
| 11   | `classify_exit`: `Stop` 분기에서 `OutcomeFailure::Ambiguous` 제거             | `d54_ambiguous_failure_has_a_distinct_exit_error` 외 8건                                                                                    |
| 12   | `classify_exit`: `Ambiguous + Stop` 청산을 `Target` 종류로 오분류             | `d54_ambiguous_failure_has_a_distinct_exit_error` 외 6건                                                                                    |
| 13   | `classify_exit`: `Ambiguous + Stop` 청산을 `AmbiguousExit` 오류로 회귀        | `d54_ambiguous_failure_has_a_distinct_exit_error` 외 9건                                                                                    |
| 14   | `classify_exit`: `Ambiguous + Target`(목표 갭 동반)을 `Target`으로 임의 허용  | `d58_ambiguous_failure_with_stop_gap_uses_open_and_target_gap_remains_pending`                                                              |
| 15   | `classify_exit`: `Ambiguous + Target`(목표 갭 동반)을 `Stop`으로 오분류       | `d58_ambiguous_failure_with_stop_gap_uses_open_and_target_gap_remains_pending`                                                              |
| 16   | `classify_exit`: `Ambiguous + Target`을 `OutcomeMismatch`로 뭉개기            | `d58_ambiguous_failure_with_stop_gap_uses_open_and_target_gap_remains_pending`                                                              |
| 17   | `classify_exit`: `Ambiguous + Unavailable`을 `AmbiguousExit`로 덮기           | `d58_partial_observation_ambiguous_bar_remains_unavailable`, `d58_zero_volume_ambiguous_bar_remains_unavailable`                            |
| 18   | `check_timing`: 분봉 봉 시작 판정 `at < evidence.at()`를 `<=`로 완화          | `d58_entry_day_ambiguous_failure_exits_at_stop_and_same_minute_stays_pending`, `d58_minute_gap_timing_distinguishes_bar_start_from_bar_end` |
| 19   | `check_timing`: 분봉 봉 시작 판정 `at < evidence.at()`를 `>`로 반전           | `d54_gap_prices_wait_for_the_estimate_policy` 외 2건                                                                                        |
| 20   | `check_timing`: 분봉 시가 갭 정각 제출 차단(`>= at`)을 `> at`로 완화          | `d58_daily_both_with_first_minute_gap_allows_preopen_submission`, `d58_minute_gap_timing_distinguishes_bar_start_from_bar_end`              |
| 21   | `check_timing`: 분봉 시가 갭 시각 조건(`>= at`)을 `<= at`로 반전              | `d54_gap_prices_wait_for_the_estimate_policy` 외 7건                                                                                        |
| 22   | `check_timing`: 분봉 시가 갭 시각 검사 제거                                   | `d58_daily_both_with_first_minute_gap_allows_preopen_submission`, `d58_minute_gap_timing_distinguishes_bar_start_from_bar_end`              |
| 23   | `check_timing`: 분봉 장중 도달 최소 1분 조건(`>= MINUTE`)을 `> MINUTE`로 변경 | `d54_minute_exit_and_entry_calendar_evidence` 외 3건                                                                                        |
| 24   | `check_timing`: 일봉 시가 갭 제출 시각 조건(`< at`)을 `<= at`로 완화          | `d54_daily_open_boundary_hit_requires_strictly_earlier_submission`                                                                          |
| 25   | `adjusted_entry`: 제출가 목표 경계 차단(`>= target`)을 `> target`으로 완화    | `d58_entry_price_at_exact_target_or_stop_boundary_remains_pending`                                                                          |
| 26   | `adjusted_entry`: 제출가 손절 경계 차단(`<= stop`)을 `< stop`으로 완화        | `d58_entry_price_at_exact_target_or_stop_boundary_remains_pending`                                                                          |
