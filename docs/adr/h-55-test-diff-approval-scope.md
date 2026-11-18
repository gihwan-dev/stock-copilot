# H-55: 승인된 테스트 변경의 정확한 범위

- 상태: 초안

## 배경

메인이 승인한 단언 기대값과 지원 `impl`의 변경도 기존 판정기는 허용하지 않아 변경분 변이의 사전 조건에서 멈췄다.

## 결정

기존 승인 행 형식을 유지하고 단언 문장이 바뀌는 각 지적 행에만 `assertion_change = true`와 비어 있지 않은 기존 `reason`을 요구한다. 파일별 전·후 SHA-256과 지적 문자열의 정확한 집합을 비교하며, 승인 이력은 원격 추적 master의 단일 부모 직접 커밋이라는 기존 경계를 유지한다. `modified`와 함수·`impl`의 `support-modified`에 적용한다. 지원 메서드의 서명, 연관 상수와 타입 변경도 같은 파일 해시와 지적의 명시 승인 범위에서 다룬다.

단언 삭제, 지적 목록 밖 변경과 해시 불일치는 거부한다. 승인된 단언도 문장 수와 호출 종류·대상, 내부 매크로 및 단락 연산 구조를 보존해야 하며, 자명한 참 단언과 단언 안의 제어 흐름을 거부한다. 지원 함수의 준비 영역에서 조기 반환·조건·반복은 기존처럼 거부한다. 구조 비교만으로 의미상 약화를 완전히 판정할 수 없으므로 메인이 정확한 파일 차이를 심사한다. `mutants-diff`는 동일한 `test-diff` 판정을 그대로 사용한다.

## 검증과 한계

실제 승인 자료의 파일별 지적 건수와 같은 9개 파일·36개 지적 합성 사례, 단언 삭제와 우회 반례, 변경분 변이 사전 조건의 승인·해시 불일치 사례를 검증한다. 해석 불가능하거나 구조가 더 복잡한 승인 변경은 보수적으로 거부한다.

## 인계

H-55: 메인은 이 묶음의 기존 테스트 변경 8건을 별도 기계 승인 없이 승인하고 전체 검사에서 같은 8건만 남는지 확인해 예외 병합한다. 후속 작업의 승인은 master 단일 부모 직접 커밋의 승인 파일과 실제 차이를 대조한다.


## 2차 리뷰 후 보강

별도 명시 승인된 지원 준비 코드에는 기존 단언을 보존하는 비자명한 단언을 추가할 수 있다. 기존 단언 삭제, 승인 누락, 단언 안 제어 흐름과 항상 참인 단언은 계속 거부한다. 실패를 요구하는 expect_err·unwrap_err 호출을 없애는 우회도 거부한다. 준비 변수 이름이 check인 것과 실제 check 호출은 구별한다. 동일한 정수의 자가 대소 비교와 참 상수로 무조건 성공하는 논리합도 거부한다. 테스트 파일의 정확한 승인 해시를 master의 단일 부모 커밋에 기록해야 하는 신뢰 경계는 바꾸지 않는다.

## 손 변형

| 번호 | 변형 | 실행한 테스트와 결과 |
|---|---|---|
| 1 | 부모 하나 제한을 루트에도 허용 | h55_an_approval_introduced_in_a_root_commit_is_not_authority: caught |
| 2 | 부모 하나를 둘로 바꿈 | h55_explicitly_approved_assertion_expectation_change_passes_but_unmarked_does_not: caught |
| 3 | 승인·대상 이력 관계 OR을 AND로 | 첫 선택 survived; 재실행 later_main_approval_applies_to_a_historical_master_commit caught |
| 4 | 관계 없는 이력의 거부 조건 AND을 OR로 | h55_explicitly_approved_assertion_expectation_change_passes_but_unmarked_does_not: caught |
| 5 | 전 해시 비교 반전 | h55_explicitly_approved_assertion_expectation_change_passes_but_unmarked_does_not: caught |
| 6 | 후 해시 비교 반전 | h55_explicitly_approved_assertion_expectation_change_passes_but_unmarked_does_not: caught |
| 7 | 전후 해시 불일치 OR을 AND로 | h55_diff_precondition_accepts_only_the_exact_approved_assertion_change: caught |
| 8 | 정확한 지적 집합 비교 반전 | h55_explicitly_approved_assertion_expectation_change_passes_but_unmarked_does_not: caught |
| 9 | 고립 지적이 비어 있어야 함 | h55_explicitly_approved_assertion_expectation_change_passes_but_unmarked_does_not: caught |
| 10 | 현재 지적 포함 비교 반전 | h55_explicitly_approved_assertion_expectation_change_passes_but_unmarked_does_not: caught |
| 11 | 명시 단언 승인 값을 거부로 | h55_explicitly_approved_assertion_expectation_change_passes_but_unmarked_does_not: caught |
| 12 | 없는 단언 승인 필드를 승인으로 | 원 변형 unviable (검출 수 제외); 좁은 대체 변형 h55_explicitly_approved_assertion_expectation_change_passes_but_unmarked_does_not caught |
| 13 | 잘못된 필드 타입을 거부 대신 허용 | h55_assertion_approval_field_requires_a_boolean: caught |
| 14 | 단언 수가 줄어도 허용 | h55_an_assertion_approval_does_not_authorize_assertion_deletion: caught |
| 15 | 단언 승인 판정 반전 | h55_explicitly_approved_assertion_expectation_change_passes_but_unmarked_does_not: caught |
| 16 | 단언 매크로 이름 비교 반전 | h55_explicitly_approved_assertion_expectation_change_passes_but_unmarked_does_not: caught |
| 17 | 단언 호출 대상 비교 반전 | h55_explicitly_approved_assertion_expectation_change_passes_but_unmarked_does_not: caught |
| 18 | 내부 매크로 동일성 비교 반전 | h55_explicitly_approved_assertion_expectation_change_passes_but_unmarked_does_not: caught |
| 19 | 단락 연산 동일성 비교 반전 | h55_explicitly_approved_assertion_expectation_change_passes_but_unmarked_does_not: caught |
| 20 | 자명한 참 단언 보호 제거 | 첫 선택 survived; 재실행 h55_explicit_approval_rejects_obviously_true_comparisons caught |
| 21 | 승인 상태 비교 반전 | h55_explicitly_approved_assertion_expectation_change_passes_but_unmarked_does_not: caught |
| 22 | Rust 파일 경로 검사 반전 | h55_explicitly_approved_assertion_expectation_change_passes_but_unmarked_does_not: caught |
| 23 | impl 지원 항목 허용 제거 | h55_approved_impl_associated_constant_change_is_accepted: caught |
| 24 | 연관 타입의 단언 보존 비교 반전 | h55_impl_type_change_cannot_delete_a_compile_time_assertion: caught |
| 25 | 지적 집합의 부분 일치만 요구 | 첫 선택 survived; 재실행 approval_rejects_a_declared_finding_that_did_not_occur caught |
| 26 | 자명한 비교의 연산자 반전 | h55_explicit_approval_rejects_obviously_true_comparisons: caught |
| 27 | 단언 불린 상수 반전 | h55_explicit_approval_rejects_obviously_true_comparisons: caught |
| 28 | 단언 부정 연산 제거 | h55_explicit_approval_rejects_obviously_true_comparisons: caught |
| 29 | 단언 괄호 정규화 제거 | h55_explicit_approval_rejects_obviously_true_comparisons: caught |
| 30 | 동일한 단언 식 비교 반전 | h55_explicit_approval_rejects_obviously_true_comparisons: caught |
| 31 | check 준비 변수를 일반 검증 호출로 처리 | h55_preparation_named_check_is_not_mistaken_for_a_check_call: caught |
| 32 | expect_err·unwrap_err 확인을 보호 대상에서 제외 | h55_approval_cannot_hide_removing_expected_error_checks: caught |

## 3차 리뷰 후 보강

독립 비판은 별도 master 승인 이력과 정확한 해시를 갖춘 합성 저장소에서 준비 매크로의 조건부 반환과 새 보조 함수의 프로세스 종료가 기존 단언을 건너뛰는 사례를 실제 `test-diff`와 nextest로 재현했다. 준비 매크로의 표현식 인자를 방문하고, 파싱되지 않는 구문은 제어 흐름 토큰을 보수적으로 검사한다. 전체 변경에서 새로 도입된 함수와 메서드 이름이 승인 대상 본문에 늘면 다른 파일에 있어도 준비 승인으로 해소하지 않는다.

이는 Rust 이름 해석이나 임의 매크로 확장을 실행하는 검사가 아니다. 무관한 동명 식별자를 거부할 수 있고 식별자 개수가 같아도 의미가 바뀌는 경우, 간접 호출 및 기존 함수의 행동 변화는 판정하지 못한다. 메인이 정확한 승인 대상 차이와 호출 의미를 계속 검토한다. 현재 승인 후보 8건은 기존 기준과 대상 트리의 별도 구조 검사에서 통과했지만 실제 master 승인 기록은 아니다.

## 3차 수정 손 변형

| 변형 | 검출 테스트 |
|---|---|
| 파싱된 매크로 제어 결과 무시 | approved_preparation_cannot_hide_control_flow_inside_a_macro: caught |
| 반복 구문 매크로 검사 무시 | approved_preparation_cannot_hide_control_flow_in_macro_repeat_syntax: caught |
| 새 함수 이름을 등록하지 않음 | approved_preparation_cannot_hide_an_exit_in_a_new_helper: caught |
| 새 메서드 이름을 등록하지 않음 | approved_preparation_cannot_hide_an_exit_in_a_new_method: caught |
| 새 함수 식별자 수 비교 반전 | approved_preparation_cannot_call_a_new_helper_in_another_file: caught |

처음 시도한 두 변형은 사용하지 않는 코드·변수 오류로 컴파일되지 않아 검출 수에서 제외했고, 위 표의 대체 변형은 실제 테스트로 검출했다.

## 등록 인계

메인은 초안을 저장소에 등록하고 보호 설정 생성물에 현재 이름을 기록하도록 결정했다. 메인이 병합 때 번호를 부여한 뒤 저장소 절차대로 보호 생성물을 다시 만든다. 테스트와 검사 코드는 초안 이름을 참조하지 않는다.

## 3차 수정 검증

수정 범위는 8609b825d8915a2eb8a56873b557afe27bce3144 → 77ab38679a1de07b312f56e619e2a7d3607e4e19다. 정상 훅은 Node 992개, Rust nextest 2,223개와 커버리지 1,147개를 통과했으나 test-diff는 기존 테스트 변경 8건에 대한 별도 master 승인 부재로 실패했다. 이어 남은 끄기 보고와 훅 설치 검사는 따로 통과했고 감사 공간은 전후 0개였다. 이 커밋에 한해 지시문이 허용한 예외를 적용했으며, 변경분 변이는 같은 사전 조건에서 멈춰 실행되지 않았다.

## 최신 master 병합 시도

3bdca225c839fac1e2a6ef793977f3fde08683ba를 정상 훅으로 병합하려 했고 병합 트리 85384f1605fe05768b5931e4e1275d9d13c45122에서 Node 992개, Rust nextest 2,339개, 커버리지 1,192개가 통과했다. 기존 테스트 변경 8건의 별도 master 승인 부재로 test-diff가 실패해 병합을 취소했다. 실행별 임시 공간은 before=0 after=0이었다. 변경분 변이는 실행되지 않았고 원격 결과도 없다.
