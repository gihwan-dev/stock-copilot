# S-07 신호·제안 저장소
- 상태: 초안

## 결정

검증 보고서는 제안 ID별로 각 검사항목의 결과를 순서대로 저장한다. 여러 보고서를 한 번에 추가할 때 하나의 트랜잭션으로 처리한다. 보고서를 복원하는 생성자는 검사항목 값만 만들며 검증된 제안이나 승인 권한을 만들지 않는다. 저장소는 시간이나 가격으로 만료를 판정하지 않는다. 저장된 보고서는 화면에 보여 주는 과거 사실이며 승인 시 다시 검증한다.

| 보고서 열 | 도메인 검사 |
|---|---|
| c01 | Instrument |
| c02 | MarketHours |
| c03 | AvailableFundsOrQuantity |
| c04 | OppositePendingOrder |
| c05 | BuyWarnings |
| c06 | Principles |
| c07 | OrderPrice |
| c08 | MarketOrderPermission |
| c09 | AccountMatch |
| c10 | PriceObservation |
| c11 | Expiry |
| c12 | PriceRange |

소비 기록은 제안 ID를 기본 키로 둔 STRICT 테이블에 추가한다. 즉시 쓰기 트랜잭션 안에서 같은 ID의 기존 기록을 확인하면 포트는 `AlreadyConsumed`를 돌려준다. 없는 ID는 삽입하며 다른 데이터베이스 오류는 그대로 실패시킨다. 수정과 삭제, `INSERT OR REPLACE`를 통한 기존 ID의 교체는 트리거로 거부한다. 서로 다른 쓰기 연결의 경쟁도 같은 트랜잭션과 제약을 쓴다. 테스트용 메모리 포트는 동일한 결과와 기록 순서를 제공하고 대본으로 실패를 주입한다.

## 미완료와 인계

- A-14·O-04: 초안 저장소와 소비 기록의 실제 호출 및 주문 절차 조립은 별도 작업이다. 현재 테이블은 제안의 존재를 외래 키로 보장하지 않으므로 실제 주문 경로에 연결하지 않는다.
- A-27·S-18: 최초 기준 `3bdca225c839fac1e2a6ef793977f3fde08683ba`는 `a-27` 끝 `0bd771fb64d8c7b4f48dba76be06c210197923c9`를 포함하지 않았다. 작업 중 확인한 master `0214f7f0a9a32e2b7ca8f96a28888512c00a65ec`에는 A-27이 병합돼 있다. ADR 0143의 스냅숏·기대 상태 커밋, 제출·폐기·발생 이력과 보정 원장 위치가 같은 직렬화 경계에 참여해야 하며 실제 제출·보정 원장은 S-18과 결합해야 한다.
- S-07: `Proposal<Draft>` 저장·복원, 현재 신호와 이력은 아직 구현하지 않았다. ADR 0059와 0104는 DB 값만으로 종목 마스터 조회를 거친 `ResolvedOrderInstrument`를 만들지 않도록 한다. 제안 객체에는 원래 마스터 행과 가격 범위 산출 근거 전체가 남지 않고, 신호 객체에도 변동성·비용 판정의 원자료 전체가 남지 않는다. 기존 마스터·추정 과제 생성 경계를 우회하지 않고 같은 제안·신호를 재구성할 저장 계약이 필요하다. 이 초안의 구현은 S-07 전체 완료가 아니다.

## 제안

신호는 과거 평가의 표시·이력 사실과 현재 재평가 결과를 구분한다. 저장 시 추정 과제와 비용·보정·출처 및 종목 마스터의 근거를 함께 보존하고, 복원에 필요한 사실이 빠지거나 서로 맞지 않으면 닫힌 실패로 처리한다. 제안 복원에는 마스터 조회 결과와 원래 범위의 평가 근거를 함께 제공하고 도메인 생성 경로를 다시 거치는 방안을 제안한다. 이력에서 복원한 사실만으로 주문 수량이나 승인 권한을 만들지 않고, 주문 시 현재 사실을 다시 검증한다. 어떤 근거 묶음으로 원래 제안과 `RaisedSignal`을 완전히 같은 것으로 재구성할지는 S-07·S-18 연결에서 정한다.

## 검증

보고서 각 항목의 왕복과 제안별 격리, 손상 거부, 묶음 삽입 실패 롤백, 소비 기록 중복 거부 및 두 연결 경쟁을 테스트한다. `ProposalRepository`의 제네릭 계약에 대해 초안 저장·복원은 컴파일되고 검증된 제안의 저장·복원은 각각 E0308로 거부된다. 별도의 검증된 제안 저장·복원 메서드를 호출하면 각각 E0599로 거부되며, 보고서를 검증 권한으로 바꾸는 비공개 메서드는 E0624로 거부됨을 별도 오프라인 컴파일 시험으로 확인한다. 시험 실행에는 저장소의 기존 `testkit::check_consumer`와 개발 전용 내부 의존성을 사용한다. 구체적인 초안 어댑터는 아직 없으므로 이 시험이 초안 왕복 구현을 증명하지는 않는다.

## 손 변형

별도 mktemp 복사본과 빌드 디렉터리에서 각 변형을 따로 컴파일·실행했다. 보고서와 가짜의 변형에 사용한 소스는 소비 기록 수정 전후에 같다. 소비 기록은 교체 방지 수정 후 테스트 7개가 통과한 상태에서 다시 변형했다. 소비 상태 비교에 관한 변형은 1차 리뷰 뒤 이를 빠짐없는 `match`로 바꾸기 전에 실행했다. 아래 28개는 당시 각각 테스트 실패로 검출됐다. 변형마다 첫 번째로 출력된 실패 테스트를 적는다.

| 번호 | 변형 | 잡은 테스트 |
|---|---|---|
| 1 | Instrument 결과를 MarketHours 결과로 저장 | `validation_report_tests::report_batch_rolls_back_on_second_insert_failure` |
| 2 | MarketHours 결과를 AvailableFundsOrQuantity 결과로 저장 | `validation_report_tests::validation_reports_preserve_each_check_and_are_scoped_by_proposal` |
| 3 | AvailableFundsOrQuantity 결과를 OppositePendingOrder 결과로 저장 | `validation_report_tests::validation_reports_preserve_each_check_and_are_scoped_by_proposal` |
| 4 | OppositePendingOrder 결과를 BuyWarnings 결과로 저장 | `validation_report_tests::validation_reports_preserve_each_check_and_are_scoped_by_proposal` |
| 5 | BuyWarnings 결과를 Principles 결과로 저장 | `validation_report_tests::validation_reports_preserve_each_check_and_are_scoped_by_proposal` |
| 6 | Principles 결과를 OrderPrice 결과로 저장 | `validation_report_tests::validation_reports_preserve_each_check_and_are_scoped_by_proposal` |
| 7 | OrderPrice 결과를 MarketOrderPermission 결과로 저장 | `validation_report_tests::validation_reports_preserve_each_check_and_are_scoped_by_proposal` |
| 8 | MarketOrderPermission 결과를 AccountMatch 결과로 저장 | `validation_report_tests::validation_reports_preserve_each_check_and_are_scoped_by_proposal` |
| 9 | AccountMatch 결과를 PriceObservation 결과로 저장 | `validation_report_tests::validation_reports_preserve_each_check_and_are_scoped_by_proposal` |
| 10 | PriceObservation 결과를 Expiry 결과로 저장 | `validation_report_tests::validation_reports_preserve_each_check_and_are_scoped_by_proposal` |
| 11 | Expiry 결과를 PriceRange 결과로 저장 | `validation_report_tests::corrupted_report_fails_closed_without_affecting_other_proposals` |
| 12 | PriceRange 결과를 Instrument 결과로 저장 | `validation_report_tests::corrupted_report_fails_closed_without_affecting_other_proposals` |
| 13 | 기존 소비 상태 비교 반전 | `consumption_tests::consumption_is_unique_and_isolated_by_proposal` |
| 14 | 중복 소비를 신규 소비로 반환 | `consumption_tests::consumption_is_unique_and_isolated_by_proposal` |
| 15 | 데이터베이스 오류를 기존 소비로 처리 | `consumption_tests::trigger_failure_does_not_record_consumption` |
| 16 | 없는 소비 기록을 소비됨으로 판정 | `consumption_tests::consumption_is_unique_and_isolated_by_proposal` |
| 17 | 신규 소비를 기존 소비로 반환 | `consumption_tests::consumption_is_unique_and_isolated_by_proposal` |
| 18 | 보고서 일괄 저장의 첫 행 누락 | `validation_report_tests::corrupted_report_fails_closed_without_affecting_other_proposals` |
| 19 | 실패 보고서를 성공으로 기록 | `validation_report_tests::report_batch_rolls_back_on_second_insert_failure` |
| 20 | 복원된 시장 시간을 종목 결과로 읽음 | `validation_report_tests::validation_reports_preserve_each_check_and_are_scoped_by_proposal` |
| 21 | 소비 기록 수정 금지 트리거 제거 | `consumption_tests::consumption_records_cannot_be_changed_or_deleted` |
| 22 | 가짜의 기존 소비 판정 반전 | `tests::memory_consumption_is_atomic_across_threads` |
| 23 | 가짜의 중복 소비를 신규로 반환 | `tests::memory_consumption_is_atomic_across_threads` |
| 24 | 가짜의 소비 상태 판정 반전 | `tests::memory_consumption_distinguishes_first_duplicate_and_scripted_error` |
| 25 | 가짜 보고서 제안 키 필터 반전 | `tests::memory_reports_preserve_order_scope_and_scripted_errors` |
| 26 | 가짜 대본의 소비 오류를 성공으로 대체 | `tests::memory_consumption_distinguishes_first_duplicate_and_scripted_error` |
| 27 | 가짜 보고서 저장 순서 반전 | `tests::memory_reports_preserve_order_scope_and_scripted_errors` |
| 28 | 소비 기록의 교체 금지 트리거 제거 | `consumption_tests::sqlite_rejects_conflict_replacement_of_consumption` |
