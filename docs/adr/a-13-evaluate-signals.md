# A-13: 제출된 추정의 신호 평가
- 상태: 초안

## 결정

`EvaluateSignals`는 종목의 스냅숏과 제출 목록을 다시 읽어 마지막 제출 ID가 같을 때만 평가한다. 과제 원장에서 발급된 과제를 찾아 도메인의 보정, 임계값과 신호 판정을 적용한다. 출처는 제출 내용으로 구성하지 않고 A-27의 독립 원장을 읽는다. 저장 충돌이나 제출 조회 시점 차이는 최대 세 번 처음부터 다시 읽고, 폐기된 제출은 즉시 중단한다. 커밋이 돌려준 이벤트만 성공 뒤 순서대로 발행하며 각 전송 결과를 반환한다. 제출 기록은 변경하지 않는다.

새 포트 `SignalEvaluationData::inputs`는 제출별 보정 스냅숏과 별도 최신 원장 위치, 보유 평가, 투자 원칙, 종목·업종 분류, 대상 가격, 캘린더 및 수량 계산에 필요한 비용 사실을 제공한다. 표와 원장 위치는 각각 부재를 표현한다. 정상 빈 표는 부재가 아니다. A-15의 `ResolutionCalibration`은 표와 위치가 둘 다 있는 경우의 공급원이며, 실제 어댑터는 별도 위치도 정확히 조회해야 한다. 표 부재·오래됨·다른 조합은 D-51의 각 사유로 신호 없음을 커밋한다. 입력·제출·과제·출처·발생 이력 조회나 판정 준비가 실패한 경우 `SignalInvalidationRepository::invalidate`로 기존 상태를 조건부 비활성화한 뒤 오류 원인과 이벤트 전달 결과를 함께 반환한다. 이 포트는 읽은 스냅숏 전체가 여전히 같을 때만 현재 신호를 제거하며, 최신 제출과 폐기 원장을 바꾸지 않고 발생 이력은 보존한다. 보정 위치를 읽지 못했어도 기존 상태를 제거할 수 있도록 비활성화는 보정 위치가 아니라 스냅숏과 제출 버전을 조건으로 한다. 충돌하면 처음부터 다시 읽는다. 일치하는 최신 제출이 이미 폐기됐다면 이벤트 없이 `SubmissionDiscarded`를 반환하고 이전 제출만 폐기된 경우에는 새 제출의 조회 오류를 보존한다. 비활성화 저장이 실패하면 조회 오류와 저장 오류를 함께 반환하고 해제 이벤트를 내지 않는다.

매수 발생 ID는 호출자가 제공하며 저장소는 ID 충돌을 거부한다. 호출자는 별개의 새 발생마다 새로운 ID를 사용하고 같은 평가의 충돌 재시도에는 그 ID를 유지한다. 새 매수 발생과 방향 변경에만 도메인이 발생 기록을 만든다. 유지·내용 갱신은 기존 발생을 이어 쓴다. A-27 발생 이력을 `calculate_buy_quantity`에 그대로 전달하고 현재 발생의 자기 제외는 도메인에 맡긴다. 일시 조회 실패로 신호가 해제된 뒤 같은 제출에서 매수가 다시 발생하면 원래 발생의 H 구간이 끝나기 전에는 ADR 0135에 따라 반복 제한을 받을 수 있다. 새 매수 발생에 필요한 현재 시각 이후 H 구간을 만들 달력이 부족하면 `BuySignalRecord::from_signal_checked`가 도메인 판정 불가 사유를 반환하고, 유스케이스는 `Calendar` 신호 없음으로 다시 전이해 기존 신호를 해제한다. 기존 매수 발생의 유지·내용 갱신에는 새 H 구간을 만들지 않는다. 기존 `from_signal`은 종전 수량 불가 반환 계약을 유지한다. 대상 가격은 도메인의 종목 ID 검증을 통과한 관측을 수량 계산에도 전달한다. 비용 사실이 없으면 수량은 `Indeterminate`이며 판단한 신호 자체는 기록한다. 계획과 수량 불가 사유는 평가 결과로 돌려준다. 이 결과는 주문 승인이나 주문 전송 권한이 아니다.

기존 `Thresholds::compare`를 공개해 정확한 도메인 비교 결과를 관측할 수 있게 했다. 새 계산이나 정책은 추가하지 않았다. 여유분은 임의 입력을 받지 않고 현재 투자 원칙의 위험 성향을 `Margin::for_appetite`에 전달한다. 테스트는 과제에 연결된 비용을 조정해 제출 가능한 정수 확률과 임계값이 정확히 같은 점을 도메인 비교 결과로 찾고 그 바로 아래·같음·바로 위를 유스케이스에 넣는다.

| 전이 | 커밋 성공 뒤 이벤트 |
|---|---|
| 없음 → 발생 | SignalRaised |
| 동일 내용 유지 | 없음 |
| 같은 방향 내용 갱신 | 없음 |
| 발생 → 없음 | SignalCleared |
| 매수 ↔ 매도 | SignalCleared, SignalRaised 순서 |
| 입력 조회 실패 후 조건부 비활성화 | 기존 신호가 있을 때 SignalCleared |
| 충돌·폐기·저장 실패 | 없음 |

## 인계

| 받는 작업 | 내용 |
|---|---|
| S-07, S-18 | 제출·평가·폐기·비활성화·보정 위치의 동일 직렬화 경계와 영속 이력, 원장 사실의 완전성을 실제 저장소에서 구현·검증한다. 비활성화는 보정 위치가 없어도 스냅숏과 최신 제출의 조건이 맞을 때만 현재 신호를 없애며 기존 제출을 폐기로 표시하지 않는다. 발생 ID는 전역적으로 중복되지 않아야 한다. |
| A-14, E-06 | 현재 저장된 신호와 수량 결과의 최신성을 다시 검증한다. 스냅숏 조회·비활성화 저장 오류, 충돌 상한 도달, 종목 불일치 등으로 기존 신호를 해제하는 커밋이 불가능한 경우 기존 저장 상태를 새 제안의 근거로 사용하지 않는다. |
| A-06, A-13 호출자, A-17, J-01 | 입력 변경·만기·재시작 시 재평가와 이벤트 유실 복구를 연결한다. 이벤트는 커밋 뒤 발행하지만 영속 전달은 보장하지 않는다. |
| S-05, S-09, M-02, AI-14, E-02 | 제출별 독립 출처 원장의 실제 사실성과 완전한 조회를 보장한다. |
| A-15, S-18 | 보정 표와 최신 원장 위치의 일관된 조회 및 최초 정상 빈 표를 제공한다. |
| E-06, U-09, U-14 | 같은 방향 내용 갱신의 화면 재조회 경로를 연결한다. |

`apps/desktop`의 현재 포트 묶음은 `SignalInvalidationRepository`나 제출 저장소와의 공통 객체를 요구하지 않는다. 실제 연결 작업은 세 계약이 동일한 영속 직렬화 경계를 쓰도록 포트 묶음과 어댑터를 구성하고 경쟁 테스트로 검증한다. 폐기된 최신 제출이면 조회 오류의 원인이 반환되지 않는다. S-07·S-18은 저장소 불일치 등의 진단을 별도로 보존해야 하는지 검토한다. 현재 별도의 영속 진단은 구현하지 않았다.

## 보호 생성물

새 ADR 추적 후 지정된 생성 스크립트로 `.codex/config.toml`과 `harness/claude-hooks/hashes.json`을 갱신했다. 생성기가 추적 ADR 이름을 자동 열거하므로 초안 이름이 생성물에 나타나지만 생성기를 바꾸거나 보호 범위를 줄이지 않았다. 최신 master 병합에서 D-52의 ADR 등록과 A-13 초안 등록이 충돌해 두 항목을 포함하도록 같은 생성물들을 다시 만들었다. 이 내용 변경은 지시문의 구체적 사전 허용 범위이며 사용자 서명은 없다. 기준의 서명 잠금이 비활성인 사실은 서명이 검증됐다는 뜻이 아니다.

## 검증과 한계

가짜 저장소의 동시성은 단일 프로세스 메모리 계약 검증이다. 실제 계좌, 키, API 및 SQLite 저장은 사용하지 않았다. 프로젝트 신뢰와 이 세션에 훅 신뢰가 적용됐는지는 확인할 수 없다. 세션 신뢰와 별도로 Git 훅 및 명시적으로 실행한 검사 결과를 리뷰 기록에 남긴다.

## 손 변형

초기 손 변형은 임시 복사본의 빌드 경로를 `CARGO_TARGET_DIR`로 지정해 실행했다. 저장소의 검사 환경 규칙에 맞지 않아 아래 초기 결과는 참고 기록으로만 두며 완료 기준의 준수 근거로 세지 않는다.

기준 커밋 `36344262612d7f359d6c171c271c6e1550025ccb`를 Python `tempfile.mkdtemp`로 만든 작업 사본에 내보내고 각 변형마다 `cargo test --offline --locked -p testkit-ports --test evaluate_signals`를 실행했다. 기준과 모든 변형 복원 뒤 테스트가 통과했다. 25개 중 컴파일된 24개를 검출했고 18번은 미사용 인자로 컴파일되지 않아 수에서 제외한다. 아래의 각 행은 실제로 실행한 변형이다.

| 번호 | 변형 | 실제 검출 테스트 |
|---|---|---|
| 1 | 최신 대신 첫 제출 | `new_submission_recomputes_and_preserves_older_record`, `concurrent_new_submission_forces_a_fresh_read_before_commit` |
| 2 | 재시도 상한 2 | `conflicts_are_bounded_and_discarded_submissions_do_not_retry` |
| 3 | 재시도 상한 4 | `conflicts_are_bounded_and_discarded_submissions_do_not_retry` |
| 4 | 제출 버전 비교 반전 | `conflicts_are_bounded_and_discarded_submissions_do_not_retry`, `storage_failure_never_publishes_or_persists_a_signal` |
| 5 | 제출 없음 조건 반전 | `absence_of_any_submission_has_no_event` |
| 6 | 충돌을 폐기로 처리 | `conflicts_are_bounded_and_discarded_submissions_do_not_retry`, `concurrent_new_submission_forces_a_fresh_read_before_commit` |
| 7 | 폐기 후 재시도 | `actual_discarded_submission_cannot_be_raised_again`, `conflicts_are_bounded_and_discarded_submissions_do_not_retry` |
| 8 | 발생 이벤트를 해제 이벤트로 발행 | `missing_calibration_clears_after_commit_and_keeps_the_reason`, `decision_failures_clear_the_old_signal_and_preserve_specific_reasons` |
| 9 | 신호 없음 사유 삭제 | `cash_failure_is_preserved_from_domain_without_creating_a_buy`, `concurrent_new_submission_forces_a_fresh_read_before_commit` |
| 10 | 커밋에 스냅숏 반영 위치 전달 | `stale_calibration_is_distinct_and_clears_a_signal`, `missing_calibration_clears_after_commit_and_keeps_the_reason` |
| 11 | 발생 이력 비우기 | `old_buy_occurrence_is_passed_to_domain_quantity_after_clear_and_raise` |
| 12 | 위험 성향 무시 | `threshold_margin_follows_the_current_risk_appetite` |
| 13 | 보정 부재 사유 변경 | `missing_calibration_clears_after_commit_and_keeps_the_reason` |
| 14 | 보정 노후 사유 변경 | `stale_calibration_is_distinct_and_clears_a_signal` |
| 15 | 보정 조합 불일치 사유 변경 | `mismatched_calibration_is_reported_without_a_signal` |
| 16 | 보유 관측 누락 | `missing_calibration_clears_after_commit_and_keeps_the_reason`, `new_submission_recomputes_and_preserves_older_record` |
| 17 | 대상 가격 누락 | `decision_failures_clear_the_old_signal_and_preserve_specific_reasons`, `old_buy_occurrence_is_passed_to_domain_quantity_after_clear_and_raise` |
| 19 | 판정 달력 비우기 | `old_buy_occurrence_is_passed_to_domain_quantity_after_clear_and_raise`, `cash_failure_is_preserved_from_domain_without_creating_a_buy` |
| 20 | 평가 시 최신 원장 위치 누락 | `old_buy_occurrence_is_passed_to_domain_quantity_after_clear_and_raise`, `actual_discarded_submission_cannot_be_raised_again` |
| 21 | 독립 웹 출처 원장 누락 | `cash_failure_is_preserved_from_domain_without_creating_a_buy`, `actual_discarded_submission_cannot_be_raised_again` |
| 22 | 이전 신호 무시한 전이 | `new_submission_recomputes_and_preserves_older_record`, `old_buy_occurrence_is_passed_to_domain_quantity_after_clear_and_raise` |
| 23 | 수량 계산 비용 사실 무시 | `old_buy_occurrence_is_passed_to_domain_quantity_after_clear_and_raise` |
| 24 | 수량 결과 누락 | `old_buy_occurrence_is_passed_to_domain_quantity_after_clear_and_raise` |
| 25 | 제출 버전을 항상 비움 | `missing_calibration_clears_after_commit_and_keeps_the_reason`, `decision_failures_clear_the_old_signal_and_preserve_specific_reasons` |

### 1차 수정분

1차 리뷰 수정 트리를 별도 복사본에 적용하고 같은 명령으로 기준과 복원 후 테스트를 실행했다. 변형 7개 중 컴파일된 6개를 모두 검출했고, 2번은 도달 불가능 코드로 컴파일되지 않아 제외한다.

| 번호 | 변형 | 실제 검출 테스트 |
|---|---|---|
| 1 | 달력 부족을 대상 가격 오류로 변경 | `missing_future_occurrence_calendar_clears_the_old_sell_signal` |
| 3 | 일시 비활성화를 영구 폐기로 변경 | `failed_input_read_invalidates_old_signal_and_recovery_can_reassess_same_submission` |
| 4 | 비활성화 뒤 원인을 일반 오류로 반환 | `failed_input_read_invalidates_old_signal_and_recovery_can_reassess_same_submission`, `invalidation_with_no_current_signal_returns_the_read_failure_without_an_event` |
| 5 | 비활성화 스냅숏 비교 반전 | `failed_source_or_history_reads_clear_old_signal_and_preserve_the_failure`, `invalidation_with_no_current_signal_returns_the_read_failure_without_an_event` |
| 6 | 기존 발생에도 새 기간 요구 | `existing_buy_occurrence_is_preserved_without_requiring_a_new_horizon`, `raises_once_then_retains_without_another_event` |
| 7 | 새 발생 기간의 시작을 제출 시각으로 변경 | `missing_future_occurrence_calendar_clears_the_old_sell_signal` |

### 2차 수정분의 초기 재실행

커밋 `2e3851000d2f89d7bbb0351663991d2d3a35af7d`의 손 변형 34개도 초기에는 `CARGO_TARGET_DIR`를 지정해 실행했으므로 결과는 완료 기준의 준수 근거에서 제외한다.

### 기본 Cargo 경로 재실행

커밋 `2e3851000d2f89d7bbb0351663991d2d3a35af7d`을 `mktemp -d`로 만든 별도 복사본에 `git archive`로 내보냈다. `CARGO_TARGET_DIR`를 지정하지 않고 변형마다 `cargo nextest run --offline --locked -p testkit-ports --test evaluate_signals`를 순차 실행했다. 기준과 원본 복원 뒤에도 같은 테스트가 통과했다. 검출 34개, 생존 0개, 컴파일 불가 0개, 미적용 0개다.

| 번호 | 변형 | 실제 검출 테스트 또는 결과 |
|---|---|---|
| 1 | 제출 조회 실패의 비활성화 생략 | `prerequisite_read_and_preparation_failures_clear_the_old_signal` |
| 2 | 과제 원장 조회 실패의 비활성화 생략 | `prerequisite_read_and_preparation_failures_clear_the_old_signal` |
| 3 | 과제 조회 실패의 비활성화 생략 | `prerequisite_read_and_preparation_failures_clear_the_old_signal` |
| 4 | 판정 준비 실패의 비활성화 생략 | `failed_source_or_history_reads_clear_old_signal_and_preserve_the_failure` |
| 5 | 최신 대신 첫 제출 | `an_older_discard_does_not_mask_a_new_submissions_input_failure` |
| 6 | 재시도 상한 2 | `conflicts_are_bounded_and_discarded_submissions_do_not_retry` |
| 7 | 재시도 상한 4 | `conflicts_are_bounded_and_discarded_submissions_do_not_retry` |
| 8 | 제출 버전 비교 반전 | `actual_discarded_submission_cannot_be_raised_again`, `an_older_discard_does_not_mask_a_new_submissions_input_failure` |
| 9 | 제출 없음 조건 반전 | `absence_of_any_submission_has_no_event` |
| 10 | 충돌을 폐기로 처리 | `concurrent_new_submission_forces_a_fresh_read_before_commit`, `conflicts_are_bounded_and_discarded_submissions_do_not_retry` |
| 11 | 폐기 후 재시도 | `actual_discarded_submission_cannot_be_raised_again` |
| 12 | 발생 이벤트를 해제 이벤트로 발행 | `actual_discarded_submission_cannot_be_raised_again` |
| 13 | 신호 없음 사유 삭제 | `buy_threshold_below_equal_and_above_follow_the_domain_decision`, `cash_failure_is_preserved_from_domain_without_creating_a_buy` |
| 14 | 커밋에 스냅숏 반영 위치 전달 | `mismatched_calibration_is_reported_without_a_signal`, `missing_calibration_clears_after_commit_and_keeps_the_reason` |
| 15 | 발생 이력 비우기 | `old_buy_occurrence_is_passed_to_domain_quantity_after_clear_and_raise` |
| 16 | 위험 성향 무시 | `threshold_margin_follows_the_current_risk_appetite` |
| 17 | 보정 부재 사유 변경 | `missing_calibration_clears_after_commit_and_keeps_the_reason` |
| 18 | 보정 노후 사유 변경 | `stale_calibration_is_distinct_and_clears_a_signal` |
| 19 | 보정 조합 불일치 사유 변경 | `mismatched_calibration_is_reported_without_a_signal` |
| 20 | 보유 관측 누락 | `actual_discarded_submission_cannot_be_raised_again`, `an_older_discard_does_not_mask_a_new_submissions_input_failure` |
| 21 | 대상 가격 누락 | `actual_discarded_submission_cannot_be_raised_again`, `an_older_discard_does_not_mask_a_new_submissions_input_failure` |
| 22 | 미래 제출 시 평가 시각을 제출 시각으로 올림 | `decision_failures_clear_the_old_signal_and_preserve_specific_reasons` |
| 23 | 판정 달력 비우기 | `actual_discarded_submission_cannot_be_raised_again`, `an_older_discard_does_not_mask_a_new_submissions_input_failure` |
| 24 | 평가 시 최신 원장 위치 누락 | `actual_discarded_submission_cannot_be_raised_again`, `an_older_discard_does_not_mask_a_new_submissions_input_failure` |
| 25 | 독립 웹 출처 원장 누락 | `actual_discarded_submission_cannot_be_raised_again`, `an_older_discard_does_not_mask_a_new_submissions_input_failure` |
| 26 | 이전 신호 무시한 전이 | `decision_failures_clear_the_old_signal_and_preserve_specific_reasons`, `existing_buy_occurrence_is_preserved_without_requiring_a_new_horizon` |
| 27 | 수량 계산 비용 사실 무시 | `old_buy_occurrence_is_passed_to_domain_quantity_after_clear_and_raise` |
| 28 | 수량 결과 누락 | `old_buy_occurrence_is_passed_to_domain_quantity_after_clear_and_raise` |
| 29 | 비활성화 충돌을 성공으로 처리 | `stale_read_failure_does_not_clear_a_newer_submission` |
| 30 | 달력 부족 이유 삭제 | `missing_future_occurrence_calendar_clears_the_old_sell_signal` |
| 31 | 동일 발생에 새 만기 요구 | `existing_buy_occurrence_is_preserved_without_requiring_a_new_horizon` |
| 32 | 제출 버전을 항상 비움 | `actual_discarded_submission_cannot_be_raised_again`, `an_older_discard_does_not_mask_a_new_submissions_input_failure` |
| 33 | 폐기된 최신 제출을 비활성화 성공으로 처리 | `discarded_submission_is_not_retried_when_input_read_fails` |
| 34 | 다른 제출의 폐기를 최신 제출에 적용 | `an_older_discard_does_not_mask_a_new_submissions_input_failure` |
