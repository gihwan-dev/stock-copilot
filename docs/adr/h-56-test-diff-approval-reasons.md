# H-56: 기존 테스트 승인 거부 사유

- 상태: 초안

## 배경

A-27은 기존 테스트 변경의 승인 행을 입력해도 여섯 지적이 남았으나 이유가 보이지 않았다. 당시 병합은 예외로 처리됐다.

## 결정

승인 판정과 별도로, 남은 지적과 정확히 같은 승인 행이 있으면 파일 해시, 지적 집합, 이력 신뢰와 구조 보존 실패를 표시한다. 한 파일의 승인은 파일 전체 지적을 함께 검증하므로 같은 파일의 다른 지적이 거부 원인인 경우도 표시한다. 같은 지적에 여러 승인 후보가 있으면 해시가 맞지 않는 오래된 행이 다른 행의 구조 거부 사유를 덮어쓰지 않도록 모든 후보의 사유를 함께 표시한다. 진단은 승인 범위를 바꾸지 않는다.

원격 추적 master가 없거나 원격 기준에 해당 승인 행이 없을 때도 HEAD와 현재 파일에 보이는 승인 행을 진단용으로만 읽어 신뢰 실패를 출력한다. 이 행으로 승인을 허용하지 않으며, 원격 추적 master가 없거나 승인 행이 브랜치 이력에만 있는 실제 이력 복사본에서 기존에는 두 위반에 이유가 없던 사례를 재현했다. 진단은 `trusted_authority`가 실제로 고른 기준 커밋을 `refs/remotes/origin/master`보다 우선해 사용하므로, `HEAD == origin/master`인 직접 커밋에서 실제 판정이 `master^1`을 따를 때도 `master^1`의 구조 거부 사유가 최신 커밋의 승인 파일 변경·삭제로 가려지지 않는다. 또 `Removed`·`NotListed`·`CountDecreased`·`Unreachable`·`SupportRemoved`·`Shadowing`처럼 승인 자체가 허용되지 않는 지적 종류는 신뢰된 행과 미신뢰 행 모두에서 신뢰 실패가 아니라 `승인 불가 지적 종류`를 출력한다. 신뢰된 행과 일치하는 지적에는 그 행의 구체 사유를 우선하되, 현재 브랜치·작업 트리에 신뢰된 기준 행과 완전히 일치하지 않는 새 미신뢰 행이 함께 있으면 `승인 이력 또는 기준 커밋 신뢰 실패`도 함께 출력해 오래된 기준 행의 해시 불일치가 새 미신뢰 행의 신뢰 실패를 가리지 않게 한다. 행이 파싱되지 않거나 어느 출처에도 보이지 않으면 진단을 만들지 못한다.

1차 리뷰·비판 반영으로 진단 정확도를 보강했다. (1) 파일 해시 진단은 `path`와 누락 측(`before`·`current`·`before·current`), 그리고 실제로 다른 `before_sha256`·`after_sha256`의 실제값과 승인값을 각각 출력한다. (2) `unsafe_block_reason`과 `added_support_assertions_reason`은 `safe_block`의 `ItemKind::Support` + `AssertionChange::Approved` 단언 추가 규칙을 1:1로 반영해 자명한 참 단언, 비표준 매크로, 기존 단언 유실, 명시 승인 누락을 단언 수 불일치로 오진하지 않는다. (3) `unsafe_finding_reason`은 `parsed_item`·`parsed_impl` 파싱 검증을 `new_helper_reason`보다 먼저 수행해 파싱 실패를 새 보조 함수 참조로 오진하지 않는다. (4) `unsafe_method_reason`과 `assertion_problem`은 메서드 삭제·중복·속성/가시성·본문 원인과 단언의 자명한 참·호출 종류·내부 호출·매크로·단락 연산 차이를 항목별로 구분해 출력한다. (5) 자기 지적 접두사 중복과 `"; "` 포함 복합 사유의 중복 제거 누락을 제거했다.

A-27의 기준 `3cd28ce0690c687f996db5d5b74f45af41bbbd57`과 대상 `0bd771fb64d8c7b4f48dba76be06c210197923c9`의 세 파일을 원문과 SHA-256 그대로 보관하여 임시 저장소에 기준·작업·승인·병합 이력을 구성했다. `assertion_change` 유무 모두 여섯 건이 남았다. 판정기는 기존 구조 보존 규칙에 따라 거부했다. 종속 포트 교체로 단언 구조가 달라지고 새 보조 메서드가 호출되며, 기존 `replace` 메서드가 사라지고 `struct`가 바뀐다. 파일별 모든 지적이 함께 승인되어야 하므로 하나라도 구조 조건에 맞지 않으면 그 파일의 다른 승인도 해소되지 않는다. 이와 같은 변경의 수용 여부는 메인이 내용을 확인하는 예외 병합으로 처리한다.

## 검증

실제 파일 전후 해시와 여섯 지적을 고정하고 두 승인 표시의 판정과 각 지적의 거부 사유를 검증한다. 진단은 이력 신뢰 실패와 해시 불일치도 구분한다.

임시 Git 이력을 만드는 시험은 저장소의 Git 환경 정규화 함수를 사용한다. Git 하위 프로세스에 다른 임시 저장소의 `GIT_DIR`, `GIT_WORK_TREE`, `GIT_INDEX_FILE`을 주입하면 기존 헬퍼는 그 저장소를 가리켜 실패했고, 정규화 후에는 이력 생성이 성공하며 다른 저장소의 HEAD·색인·파일이 그대로인 것을 확인했다.

## 인계

메인이 최종 리뷰와 예외 승인 정책을 확인한다.

## S-06/S-07 추가 재현

실제 승인 커밋 `3acc0bc0b8b5256ad8527c5ffab41442ccddb66c`은 두 테스트 파일을 변경하지 않고 승인 행을 추가한 단일 부모 커밋이다. 그 뒤 두 부모 병합 커밋 `1c85a7887aa3c6b18f0b51ae93919d33a395983e`을 master와 원격 추적 master의 끝으로 두고, 두 부모 작업 끝 `1733c4c18609d0acb2133800492063c2d1ffa955`을 병합 중인 임시 복제본을 만들었다. 충돌 세 파일은 실제 병합 결과 `9ba8d82e339f8a5bb4a0167720accaa51eb05dfa`의 내용으로 해결했다. 문서 정리 파일은 실제 병합 커밋과 다르지만 두 Rust 파일의 전·후 SHA-256은 승인 행과 각각 일치했다.

현재 판정기로 `test-diff-selected`의 소스 비교를 실행하면 두 지적이 그대로 남는다. 이 명령은 nextest 목록 검사를 생략하므로 전체 `test-diff` 실행의 대용 증거는 아니다. 첫 지적은 보호된 문장에 `for` 반복, 클로저와 오류 전파 `?`가 있고, 둘째 지적은 단언 인자에 클로저가 있다. 별도 승인 직후 다른 두 부모 병합을 거쳐 master 병합을 진행하는 독립 임시 Git fixture에서도 같은 해시, 지적, 두 거부 사유를 고정했다. 권한 기준·승인 이력·해시·지적 집합은 해당 재현에서 통과했다.

판정기는 구조 보존 규칙을 적용한 것이지만, 문서에는 차이가 있다. workflow 2.9는 단언 안의 **새로운** 제어 흐름을 거부한다고 쓰고, ADR 0141은 기존 여부를 구분하지 않고 단언 안 제어 흐름을 거부한다고 쓴다. 두 변경은 고정 수치만 바뀌고 기존 클로저는 그대로다. 현재 범위를 넓히지 않고 보수적인 ADR 0141 판정을 유지한다. 정확히 승인된 숫자 리터럴만 바뀌고 나머지 단언의 토큰·호출·제어 흐름이 모두 동일한 경우를 허용할지는 메인에게 정책 결정으로 제안한다. 새로운 흐름, 단언 삭제·우회와 해시·지적 불일치는 계속 거부해야 한다.



## 손 변형

`mktemp -d` 복사본에서 아래 10개 변형을 각각 실행했고 모두 검출했다. 마지막 변형의 첫 문법은 저장소 컴파일 규칙에 걸려 세지 않았고, 컴파일되는 형태로 바꿔 재실행했다.

| 변형 | 결과와 검출 시험 |
|---|---|
| 선택된 승인 권한 존재 검사 제거 | 검출: h56_reports_untrusted_approval_without_remote_master |
| 이전 SHA 일치 조건 반전 | 검출: h56_a27_actual_files_and_approval_history_explain_every_remaining_finding; h56_keeps_trusted_reasons_when_untrusted_copies_match; h56_approval_rejection_reports_an_incomplete_finding_set |
| 승인 지적 집합 비교 반전 | 검출: h56_a27_actual_files_and_approval_history_explain_every_remaining_finding; h56_keeps_trusted_reasons_when_untrusted_copies_match; h56_approval_rejection_reports_an_incomplete_finding_set; h56_reports_every_candidate_when_one_finding_has_multiple_approvals |
| 제어 흐름의 클로저 진단 제거 | 검출: h56_keeps_trusted_reasons_when_untrusted_copies_match; h56_reports_every_candidate_when_one_finding_has_multiple_approvals |
| 제어 흐름의 for 진단 제거 | 검출: h56_s06_merge_after_separate_approval_preserves_structural_rejection |
| 제어 흐름의 오류 전파 진단 제거 | 검출: h56_s06_merge_after_separate_approval_preserves_structural_rejection |
| 동일 지적의 후보 사유 덮어쓰기 | 검출: h56_reports_every_candidate_when_one_finding_has_multiple_approvals |
| 현재 파일의 승인 행 진단 제거 | 검출: h56_reports_untrusted_approval_only_in_current_files |
| 브랜치 HEAD 승인 행 진단 제거 | 검출: h56_reports_untrusted_approval_only_in_branch_history |
| 신뢰된 거부 사유를 미신뢰 행으로 덮어쓰기 | 검출: h56_keeps_trusted_reasons_when_untrusted_copies_match |
| (인계 후 재실행) `diagnose`에서 선택된 `authority`보다 `origin/master` 우선 | 검출: h56_diagnose_uses_selected_master_parent_authority_on_direct_master_commit |
| (인계 후 재실행) `approvable_rule`에서 `Removed`를 승인 가능으로 분류 | 검출: h56_reports_ineligible_rule_reason_for_all_unapprovable_finding_kinds |
| (인계 후 재실행) `diagnose_group`에서 비승인 지적 종류 분기 제거 | 검출: h56_reports_ineligible_rule_reason_for_all_unapprovable_finding_kinds |
| (인계 후 재실행) `untrusted_approval_rows`에서 비승인 지적 종류 분기 제거 | 검출: h56_reports_ineligible_rule_reason_for_all_unapprovable_finding_kinds |
| (인계 후 재실행) `append_reason` 중복 사유 검사 반전 | 검출: h56_reports_every_candidate_when_one_finding_has_multiple_approvals |
| (1차 수정) `before_sha256` 불일치 항목 생략 | 검출: h56_reports_exact_mismatched_hash_fields_and_missing_file_sides; h56_stale_trusted_row_does_not_mask_new_untrusted_branch_row; h56_reports_every_candidate_when_one_finding_has_multiple_approvals |
| (1차 수정) `unsafe_block_reason`에서 단언 수 비교를 무조건 `!=`로 회귀 | 검출: h56_support_approved_assertion_addition_reports_actual_failure_reason |
| (1차 수정) `added_support_assertions_reason`의 잔여 단언 `safe_added_assertion` 검사 생략 | 검출: h56_support_approved_assertion_addition_reports_actual_failure_reason |
| (1차 수정) `Rule::Modified`에서 `parsed_item` 전에 `references_new_helper` 호출 | 검출: h56_parse_failure_does_not_misreport_new_helper_reference |
| (1차 수정) `SupportModified(impl)` 파싱 실패 분기에서 `references_new_helper` 우선 호출 | 검출: h56_parse_failure_does_not_misreport_new_helper_reference |
| (1차 수정) `format_group_diagnosis`에서 자기 지적 접두사 생략 조건 제거 | 검출: h56_support_approved_assertion_addition_reports_actual_failure_reason |
| (1차 수정) `untrusted_approval_rows`에서 신뢰 그룹 존재 시 미신뢰 행 무시 | 검출: h56_stale_trusted_row_does_not_mask_new_untrusted_branch_row |
| (1차 수정) `unsafe_method_reason`에서 메서드 삭제 상세 문구 제거 | 검출: h56_impl_method_and_assertion_problem_report_specific_causes |
