# ADR 0136: A-15 추정 판정 유스케이스

- 상태: 확정 (2026-11-18)

- 기준: 896049a64bbd02c263288964421e44dbcb2bb6e2
- 근거: A-15, ACC-9, design 6.2·6.6, ADR 0051·0065·0080·0119·0135 및 이번 지시문

## 결정

`application::resolve_estimates` 공개 모듈의 `EstimateResolutionRepository`는 A-12와 같은 완전 제출 원장에서 거래 여부와 무관하게 모든 미판정 제출을 열거한다. 기록은 과제 ID로 식별한다. 각 제출에 시장 자료를 모아 domain 판정을 호출하고, 확정된 판정만 그 판정의 ID·실제 모델·원래 확률로 보정 사실을 만든다. `Calibration::with_fact`는 선택된 표본만이 아닌 기존 모든 사실을 보존하고 기존 생성·중복·겹침 규칙을 다시 적용한다. 서로 겹치는 같은 종목 추정은 모두 판정하지만 보정 집계에는 domain이 선택한 것만 들어간다.

ADR 0041의 모델·기간 조합당 전체 사실 65,536건 상한을 유지한다. `with_fact`는 상한을 복사 전에 확인한 뒤 기존 생성자의 조합·ID 중복·모순 규칙을 적용한다. 상한에 닿으면 판정과 보정표를 따로 쓰거나 기존 사실을 버리지 않고, 해당 추정을 미판정으로 유지하며 `TooManyFacts` 사유를 기록한다. 이 상태는 추가 정책 결정 전까지 자동으로 해소되지 않는다. `facts`는 선택되지 않은 사실까지 포함한 전체 불변 목록으로, 저장 어댑터가 상한 안에서 원래 판정과 함께 복원하는 데 쓴다.

기존 `EstimateRepository`의 개별 판정·보정 쓰기는 원자적 묶음을 보장하지 못한다. D-51의 `CalibrationSnapshot`과 `CalibrationLedgerHead`가 도입되어 새 포트는 `ResolutionCalibration`으로 보정표와 최신 원장 위치를 함께 돌려준다. 스냅숏이 없으면 `MissingCalibration`, 조합이 다르면 `MismatchedCombination`, 반영 위치와 최신 위치가 다르면 `StaleCalibration`으로 멈춘다. 정상 빈 표도 위치가 있는 스냅숏으로 공급한다.

새 포트의 `commit_resolution`은 원본 제출의 존재와 동일성, 미판정, 해당 모델·기간의 스냅숏과 최신 원장 위치가 `previous`와 일치하는지를 한 트랜잭션에서 검증한다. 저장소가 단조 증가하는 다음 위치를 정하고 판정·보정표·그 표의 반영 위치와 최신 원장 위치를 함께 갱신한다. 이미 판정된 ID는 `AlreadyResolved`, 이전 스냅숏 또는 위치가 바뀌면 `Conflict`, 저장 실패는 아무것도 쓰지 않은 오류다. 반환이 불확정인 실제 저장 결과는 단순 저장 실패로 바꾸지 않는다. 호출자는 충돌을 보고 받고 다음 실행에서 새 표본으로 재시도한다. 실제 어댑터의 원자성은 S-18의 책임이며 가짜 실행만으로 영속 트랜잭션을 증명하지 않는다.

오래된 실행이 이미 확정된 추정에 다른 사실을 계산하면 보정표 갱신에서 `ConflictingEstimate`가 먼저 나올 수 있다. 이때 `resolution`으로 같은 ID의 불변 판정을 다시 읽고, 판정에서 재구성한 사실이 읽은 보정표와 일치하면 `AlreadyResolved`, 판정이 없으면 원래 보정 오류, 서로 다르면 저장소 무결성 오류를 반환한다. 재조회 실패는 저장소 오류를 보존한다. S-18은 이 조회와 원자 커밋이 같은 원장의 사실을 보게 한다.

시장 자료 포트는 제출과 판정 시각을 받아 자료를 조회한다. 자료 부족의 domain 사유와 시장 조회 `PortError`, 저장소 조회 `SystemPortError`를 구분해 원래 오류를 보존한다. 완전한 자료가 없는 경우 `record_unavailable`은 미판정 확인과 사유 기록을 원자적으로 수행한다. 이미 판정된 추정은 사유로 덮지 않는다. 이후 성공 커밋에서 해당 미판정 사유를 정리한다. 사유 기록 자체가 실패하면 저장 오류를 보고한다. 종목 키 이벤트는 성공 커밋 이후 기록당 한 번 발행하고 구독 실패는 별도로 반환한다. 프로세스 종료를 가로지르는 내구 이벤트 전달은 ADR 0065의 범위가 아니다.

`TooManyFacts`는 미판정 사유로 저장한다. 그 외 보정 오류는 실행 결과로 반환하며 내구 진단 기록은 보장하지 않는다. `MissingFact`는 확정 판정이 보정 사실을 만들지 못하는 방어 결과로, 현재 domain의 성공·실패 변환에서는 발생하지 않는다. 저장소 오류 자체의 내구 진단은 별도 저장소 오류 처리 계약이 필요하다.

저장된 미판정 사유는 마지막 처리 실패의 이력이지 현재 자료 상태가 아니다. 이후 실행이 `Ongoing`이면 과거 사유를 현재 결측으로 표시하지 않는다. 무조건 삭제하면 늦게 끝난 오래된 실행이 더 최근의 결측 사유를 지울 수 있어, 현재 상태의 영속 전이가 필요할 때는 별도 버전 비교 계약을 정한다. 자료 공급자는 ADR 0051의 만기를 계산할 수 있는 연속된 미래 거래소 달력을 제공한다. 정규 거래일 기준 며칠·몇 주~몇 달·반년 이상은 시작 거래일 이후 각각 5·40·126번째 정규장까지가 필요하다. 달력이 부족하면 임의로 외삽하지 않고 `Calendar` 사유로 재시도한다.

ADR 0051의 기존 공개 함수는 마감 뒤 빠진 필수 분봉을 모호 실패로 처리한다. 이번 A-15 지시의 자료 불완전 시 미판정 유지가 더 구체적이므로 별도 `evaluate_with_complete_data`에서 필수 분봉 누락을 `MissingCandles`로 반환한다. 목표·손절·같은 온전한 분봉의 동시 도달·만기 등의 계산과 다른 검증은 동일 domain 경로를 쓴다. 기존 호출의 누락 처리와 기존 테스트는 보존한다. 자료를 공급하는 쪽은 행사 목록과 캘린더의 출처·완전성 및 수정 기준을 보장해야 한다.

마지막 분봉의 필요 여부는 다음 분봉 종료 초가 판정 범위 끝 이내인지로 확인한다. 지원하는 `Timestamp`의 연도 범위에서는 다음 분봉 종료 초가 현재 위치의 초보다 항상 크므로, 앞 조건이 참이면 현재 위치가 범위 끝보다 작다는 비교는 중복이다. 동작은 그대로 두고 이 중복 비교를 제거했으며, 정확한 분봉 종료·정규장 종료 경계를 회귀 시험으로 확인한다.

## 제안

A-15의 판정 결과는 성공·실패와 그 근거를 기록한다. 추정 자체의 수익률을 이 판정에 추가하는 규칙은 문서에서 독립적으로 정해지지 않았다. design 6.6의 가상 청산 수익률은 신호에 대한 별도 D-29 계약이므로 거래하지 않은 모든 추정의 수익률로 재해석하지 않는다. 제안: 추정별 수익률의 진입가·청산가·비용·벤치마크 및 자료 누락 규칙을 정의한 뒤 domain에서 계산하고 별도 사실로 연결한다.

조합당 65,536건 이후의 전체 이력 처리도 정의 대기다. 제안: 실제 보존 규모와 복원·판정 비용을 측정하고, 전체 사실 보존과 중복·겹침 선택의 의미를 유지할 이력 집계·저장 구조 및 상한 변경을 함께 결정한다. 임의로 과거 사실을 자르거나 판정만 따로 기록하지 않는다. 성적표의 전체 관측 65,536건 제한도 같은 결정에서 다룬다.

## 인계

| 받는 작업 | 내용 |
|---|---|
| S-18 | A-12 제출과 같은 저장 원장에서 누락 없는 미판정 열거와 ID별 판정 조회, 중복 거부, 최초 정상 빈 스냅숏, 조합별 원장 위치 및 판정·보정 스냅숏·최신 위치의 조건부 원자 커밋을 구현한다. 상한 초과를 포함한 사유와 상한 이내 전체 사실을 재시작 뒤 복원한다. 복원·조회·중복 커밋에서 확정 판정에 대응하는 보정 사실의 누락과 불일치를 `RepositoryIntegrity`로 거부하는 계약 시험을 둔다. 기존 종목 조회와 같은 저장 사실을 돌려준다. 미판정 사유를 현재 결측으로 오인하지 않는다. 보정 무결성 오류의 내구 진단은 저장소 오류 처리 계약과 함께 정한다 |
| A-27 | S-18이 제공하는 같은 원장의 스냅숏과 최신 위치를 D-51 계약대로 조회하고, 저장 시 위치 변화와 없음·정상 빈 표·오래됨을 구별한다 |
| T-07, T-13, S-23, J-12 | 달력·수정 봉·기업 행사 입력의 종목·기간·수정 기준·완전성 및 필요한 분봉 보존과 앱 재시작 뒤 재시도를 보장한다. 영구 결측의 건수·비율과 보관 기간은 실측한다 |
| D-17, 성적표 연동 작업 | 전체 관측 65,536건 상한과 조합별 보정 이력 상한을 후속 이력 정책과 함께 검토한다. 영구 결측의 미판정 건수를 성적표의 분모·표시와 함께 검토한다. 현재 A-15는 성적표를 호출하지 않는다 |
| D-29, 성적표 연동 작업 | 신호 가상 청산 수익률과 추정별 수익률의 의미를 분리하고 위 제안의 소유 작업·규칙을 정한다 |

## 검증

시작 시 프로젝트 신뢰와 세션 훅 해시 신뢰의 적용은 확인하지 못했다. frozen 설치 및 Git lefthook 설치는 성공했다. 첫 변경분 검사에서 타입 별칭에 대한 raw-types 위반이 발견되어 별칭을 제거했다. D-51 병합 정상 훅에서 전체 시험 2,249개와 커버리지 대상 1,192개가 통과했고, 새 원장 위치 계약을 포함한 대상 시험 27개와 Clippy도 통과했다. 기존 기준 테스트의 변경은 없으며 승인된 test-diff 변경은 0건이다.

변경분 자동 변이의 첫 실행은 40개 중 검출 21, 생존 3, 시간 초과 1, 컴파일 불가 15였다. 장 종료 경계의 생존 두 개는 시험을 더해 검출했고, 나머지는 위에서 설명한 결과 동등 비교를 제거했다. 조기 완료 시 동시성 시험이 무기한 기다리지 않도록 시작과 완료 신호를 구분했다. D-51 병합 후 최신 master 기준 재실행은 38개 중 검출 23, 컴파일 불가 15, 생존·시간 초과 0이다. 첫 실행의 시간 초과는 재실행에서 검출됐고 컴파일 불가는 검출 수에 넣지 않는다.

다음 손 변형은 D-51 병합 후 코드와 시험이 담긴 인덱스 트리 `56bd30083981931c57703f50d8f9dec85d53c49c`를 `mktemp -d` 복사본에 내보내 하나씩 실행한 결과다. 42개 모두 컴파일되고 표의 시험에서 실패했다. 첫 시도의 두 컴파일 실패는 세지 않고 유효한 변형으로 고쳐 다시 실행했다. 첫 시도에서 대기한 시험 프로세스 하나는 직접 종료했으며, 동기화 보강 뒤 전체 변형을 다시 실행한 결과만 표에 적는다.

| 번호 | 바꾼 것 | 검출한 시험 |
|---|---|---|
| 1 | 열거를 한 건으로 제한 | `tests::distinct_instruments_update_the_same_model_without_losing_either_fact` |
| 2 | 첫 제출 누락 | `tests::calibration_head_for_another_model_is_rejected` |
| 3 | 보고 ID 고정 | `tests::report_keeps_the_identity_of_each_submitted_estimate` |
| 4 | 봉 전체 누락 | `tests::calibration_head_for_another_model_is_rejected` |
| 5 | 달력 전체 누락 | `tests::calibration_head_for_another_model_is_rejected` |
| 6 | 판정 시각 대신 제출 시각 | `tests::calibration_head_for_another_model_is_rejected` |
| 7 | 진행 중을 충돌로 반환 | `tests::ongoing_and_storage_failure_do_not_commit_or_publish` |
| 8 | 시장 자료 오류를 진행 중으로 반환 | `tests::market_read_failure_keeps_endpoint_and_retry_reason_for_the_next_run` |
| 9 | 판정 자료 오류를 진행 중으로 반환 | `tests::incomplete_minute_and_daily_data_remain_pending_and_retry` |
| 10 | 기존 보정표 사실 제거 | `tests::conflicting_stored_outcome_and_calibration_report_repository_integrity` |
| 11 | 없는 보정 스냅숏을 진행 중으로 반환 | `tests::missing_calibration_snapshot_keeps_the_estimate_pending` |
| 12 | 원자 저장에 과거 원장 위치 전달 | `tests::distinct_instruments_update_the_same_model_without_losing_either_fact` |
| 13 | 판정 이벤트 종류 변경 | `tests::concurrent_resolution_commits_only_once_and_rejects_replacement` |
| 14 | 판정 이벤트 중복 | `tests::concurrent_resolution_commits_only_once_and_rejects_replacement` |
| 15 | 판정 이벤트 종목 고정 | `tests::distinct_instruments_update_the_same_model_without_losing_either_fact` |
| 16 | 보정 사실 ID 교체 | `tests::conflicting_stored_outcome_and_calibration_report_repository_integrity` |
| 17 | 보정 확률 고정 | `tests::distinct_instruments_update_the_same_model_without_losing_either_fact` |
| 18 | 미판정 사유 대신 달력 사유 저장 | `tests::incomplete_minute_and_daily_data_remain_pending_and_retry` |
| 19 | 이미 판정을 충돌로 반환 | `tests::concurrent_resolution_commits_only_once_and_rejects_replacement` |
| 20 | 보정 갱신에 새 사실 생략 | `tests::single_fact_extension_rejects_conflicting_ids_and_different_combinations` |
| 21 | 필수 분봉 누락을 모호 실패로 반환 | `tests::final_minute_is_required_when_it_has_just_finished` |
| 22 | 상한 직전 추가 거부 | `tests::established_combination_limit_applies_to_single_fact_updates` |
| 23 | 추가 후 빈 보정표 생성 | `tests::single_fact_extension_rejects_conflicting_ids_and_different_combinations` |
| 24 | 전체 보정 사실 조회를 빈 목록으로 변경 | `tests::single_fact_extension_rejects_conflicting_ids_and_different_combinations` |
| 25 | 일치하지 않아도 이미 판정으로 반환 | `tests::conflicting_stored_outcome_and_calibration_report_repository_integrity` |
| 26 | 판정과 보정 사실 일치 조건 반전 | `tests::conflicting_stored_outcome_and_calibration_report_repository_integrity` |
| 27 | 진짜 보정 충돌을 이미 판정으로 오분류 | `tests::conflicting_table_without_resolution_stays_an_error_and_read_failure_is_preserved` |
| 28 | 판정 재조회 실패를 보정 충돌로 치환 | `tests::conflicting_table_without_resolution_stays_an_error_and_read_failure_is_preserved` |
| 29 | 보정표 상한 사유 기록 생략 | `tests::full_calibration_table_leaves_resolution_pending_with_reason` |
| 30 | 마감된 분봉 누락을 달력 오류로 변경 | `tests::final_minute_is_required_when_it_has_just_finished` |
| 31 | 실패 판정을 진행 중으로 반환 | `tests::calibration_head_for_another_model_is_rejected` |
| 32 | 성공 판정을 진행 중으로 반환 | `tests::conflicting_table_without_resolution_stays_an_error_and_read_failure_is_preserved` |
| 33 | 사유 기록 성공을 진행 중으로 반환 | `tests::incomplete_minute_and_daily_data_remain_pending_and_retry` |
| 34 | 사유 기록 경합의 이미 판정됨을 충돌로 반환 | `tests::stale_data_failure_cannot_overwrite_a_completed_resolution` |
| 35 | 원자 저장 충돌을 이미 판정으로 반환 | `tests::commit_conflict_keeps_the_estimate_pending_without_an_event` |
| 36 | 기존 분봉 결측의 장 종료 비교를 다음 시각까지 연장 | `tests::existing_missing_minute_changes_from_ongoing_to_ambiguous_at_close` |
| 37 | 완료된 마지막 분봉의 필수 경계를 제외 | `tests::final_minute_is_required_when_it_has_just_finished` |
| 38 | 정확한 장 종료를 아직 장중으로 처리 | `tests::completed_later_session_uses_daily_candle_at_close` |
| 39 | 다른 조합의 원장 위치를 허용 | `tests::calibration_head_for_another_model_is_rejected` |
| 40 | 스냅숏과 원장 위치 일치 검사를 반전 | `tests::commit_conflict_keeps_the_estimate_pending_without_an_event` |
| 41 | 오래된 보정표를 없는 보정표로 분류 | `tests::stale_calibration_snapshot_never_commits_or_publishes` |
| 42 | 오래된 보정표를 정상으로 통과 | `tests::stale_calibration_snapshot_never_commits_or_publishes` |
