# A-14: 현재 신호에서 주문 초안 만들기

- 상태: 초안

## 결정

`DraftProposal`은 현재 신호 스냅숏과 해당 종목의 최신 제출·과제·독립 출처를 읽고, 선택 계좌를 지정해 현재 마스터·시세·잔고·정책·캘린더·호가 사실을 다시 가져온다. 이전 평가가 실패하고 기존 신호가 남아도 현재 제출과 평가 결과가 서로 일치하지 않거나 마스터의 내부 ID·시장·심볼이 달라지면 초안을 만들지 않는다. 평가 사유와 수량·호가 도메인 오류는 각각 보존한다.

도메인의 `market_state_at`과 `MarketTradingStatus`로 복수 캘린더의 장 상태와 열림·닫힘·불명 여부를 판정한다. 닫힌 장에서는 주문 가능 여부보다 장 상태를 먼저 판정해 신호를 유지하며 초안을 쓰지 않는다. 장 상태가 불명확하거나 여러 캘린더가 모순되면 닫힌 실패다. 장 판단에 필요한 입력 조회 자체가 실패하면 원래 조회 오류를 돌려준다. 열린 장(정규장·프리·애프터·미국 데이마켓)에서는 내부 ID가 결합된 최신 시세의 종목과 5초 신선도를 도메인에서 확인하고 추정 기준가의 변동성 범위를 검사한다. 범위 밖인 경우 현재 스냅숏을 조건으로 해당 신호의 과제를 폐기하고 커밋의 이벤트를 발행한 뒤 재추정 예약 포트를 호출한다. 만료된 초안만으로는 신호를 폐기하지 않는다. 다시 만들 때 최신 관측을 새 카드 범위에 넣는다.

매수 수량은 도메인의 `calculate_buy_quantity`, 지정가는 `calculate_limit_price`로 계산한다. application의 `buy_limit_intent`는 도메인이 만든 수량 계획과 지정가의 방향·내부 종목 ID·시장·심볼을 결합하고 수량 계산 가격을 초과하면 재계산을 요구한다. application은 확률·범위·수량·가격·만료·장 시간 집계를 직접 산술하거나 비교하지 않는다. 데이터 포트는 요청받은 계좌의 사실을 제공하며 실제 계좌와 응답 결합의 진위는 어댑터의 책임이다.

`ProposalRepository::current_draft`는 수량을 다시 계산하기 전에 현재 신호와 보정 원장, 현재 선택 계좌(U-22), 요청 계좌(`DraftCheck.account`) 및 기존 초안을 확인한다. 저장소 직렬화 경계는 현재 선택 계좌가 비어 있거나 `DraftCheck.account`와 다르면 기존 초안을 건드리지 않고 `AccountChanged`로 거부하며, `DraftProposal`은 이를 `DraftProposalError::AccountChanged`로 닫힌 실패한다. 이 경계로 인해 계좌 전환(`AccountSelectionChanged`, U-22) 전에 시작된 지연된 옛 계좌 요청이 새 계좌의 유효 초안을 대체하거나 옛 계좌 초안을 뒤늦게 저장하지 못한다. 같은 종목·같은 선택 계좌의 유효한 초안이 있으면 그 ID를 반환하고 생성 이벤트를 다시 내지 않는다. 현재 선택 계좌로 바뀐 요청에서 이전 계좌의 초안이 남아 있으면 기존 계좌 초안을 대체 상태로 전환해 재사용하지 않고, 새 계좌 기준으로 계산·저장할 수 있게 한다(새 계좌에서 수량 계산이 실패해도 이전 계좌 초안은 남지 않는다). `draft(id)`도 현재 선택 계좌와 다른 계좌의 초안이나 보정 원장이 전진한 초안은 반환하지 않는다. `store_current_draft`는 같은 확인을 초안 저장과 동일한 직렬화 경계에서 수행하되, 기존 초안 상태를 변경하기 전에 방향·계좌 일치, 스냅숏·보정 원장, 선택 계좌, 관측·제안 유효성(`proposal.check_current`), 중복 ID를 모두 먼저 검증한다. 검증 실패·오류·`ClockRollback`에서는 기존 초안을 대체하지 않고 상태를 그대로 보존한다. 같은 선택 계좌의 두 호출이 동시에 빈 결과를 보더라도 하나만 저장하고 다른 호출에는 기존 ID를 반환한다. 만료 시각 도달이나 카드 기준 가격 범위 이탈은 도메인 판정으로 확인하며, 확인된 만료 또는 달라진 신호 스냅숏의 초안은 현재 조회에서 제외한다. 새 초안 저장이 실패하거나 가격이 되돌아와도 이전 초안을 다시 유효하게 보이지 않는다. 시계 역행과 시세 불명은 교체하지 않고 닫힌 실패다.

두 메서드는 신호 스냅숏 전체와 신호가 사용한 보정 원장 위치도 대조한다. 보정 원장만 전진해 신호 리비전은 그대로인 경쟁도 거부한다. `store_current_draft`는 저장하려는 초안의 주문 방향과 현재 신호의 방향 일치도 확인한다. 기존 단독 초안 가짜는 이 결합을 제공할 수 없어 조건부 조회·저장을 닫힌 오류로 돌려준다. 신호·초안 통합 가짜는 한 잠금에서 비교와 저장을 수행하고, 내부 종목 ID를 알 수 없는 단독 `store_draft`는 닫힌 오류로 둔다. 충돌은 처음부터 다시 읽고 세 번으로 제한한다. 조회 또는 저장 경로에서 단조 시계 경쟁으로 다른 호출이 조금 더 늦은 시각의 초안을 먼저 저장해 `ClockRollback`이 난 경우에도 다시 읽어 기존 초안을 확인하며, 실제 시계 역행이 지속되면 세 번 뒤 `DraftStatus(ClockRollback)`으로 닫힌 실패한다. 마지막 재시도가 아직 평가되지 않은 제출과 신호의 불일치였다면 `SignalChanged`, 조건부 저장 충돌이었다면 `ConcurrentUpdate`를 반환한다. `ProposalCreated`는 새 초안의 저장 성공 뒤에만 발행한다. 조회·저장·예약 실패는 원인을 보존한다. 수집 중 시간이 지나 시세가 낡거나 장이 닫힌 경우를 막기 위해 주입된 시계로 수집 뒤와 저장 직전에 다시 판정하며, 저장 직전에는 도메인의 `Proposal::check_current`로 시계 역행·만료·관측·범위를 함께 확인한다. 이 경계는 검증된 제안이나 승인 권한을 만들지 않는다.

## 매도 결정과 후속 구현

메인은 매도 초안의 기본 목표를 보유 수량 전량으로 정했다. 주문 수량은 매도가능수량을 넘지 않으며, 사용자가 승인 전에 수량을 줄일 수 있다는 전제다. 이는 현재 design 6.5의 수량 변경 불가 문구보다 나중에 내려진 결정이다. 이 작업은 수량 규칙과 사용자 조정 경로를 구현하지 않는다. 매도 신호는 보존하고 `SellDraftUnavailable`로 초안을 만들지 않는다. 매수에는 영향을 주지 않는다.

후속 작업 행 초안은 다음과 같다.

| ID | 작업 | 완료 기준 | 선행 |
|---|---|---|---|
| D-57 | 매도 초안의 기본 수량 규칙 | 기본 목표는 보유 수량 전량이며 매도가능수량을 넘지 않음. 승인 전에 사용자가 수량을 줄일 수 있다는 전제를 반영해 예제와 속성 테스트로 고정. 완료 뒤 A-14의 매도 초안 생성 경로에 연결 (A-14에서 넘어옴) | D-26, D-34 |

## 인계

| 작업 | 내용 |
|---|---|
| O-04 | 메인 결정으로 A-04 인계의 정상 초안→검증→주문 요약 연결 시험을 A-14에서 옮긴다. O-04가 A-14를 선행으로 요구하고 이 시험은 O-04 주문 게이트를 요구하는 의존 순환을 끊기 위함이다. 확인 세 응답, 요청 열 필드·횟수·순서와 공개 `user_presence_order` 계약은 그대로 검증한다. 또한 `proposal_id` 조회에서 만료·대체된 초안 ID와 애초에 없는 ID를 구분해 만료 시 최신가 재생성 경로를 잇고, 소비 기록(ADR 0148)이 있는 초안은 재사용·재승인 대상에서 제외한다. |
| S-22, S-18, U-22 | 신호·제출 상태의 리비전과 보정 원장 위치 비교, 현재 선택 계좌(U-22·S-10) 일치 검증(`AccountChanged`), 종목별 현재 초안 조회·교체·저장(검증 오류 시 무손상 원자성)과 소비 기록(ADR 0148) 제외를 같은 실제 DB 트랜잭션으로 구현하고 동시 연결·계좌 전환 경쟁을 검증한다. A-14 메모리 가짜의 `draft(id)`는 현재 유효한 선택 계좌 초안만 반환하므로, S-22는 O-04가 만료·대체된 초안 ID와 없는 ID를 구분할 수 있는 조회 계약도 함께 확정한다. 선택 계좌와 응답 사실의 결합도 실제 어댑터 계약 시험으로 확인한다. 단독 초안 저장소의 닫힌 실패를 실제 결합 구현으로 대체한다. |
| A-17, J-01 | 재추정 예약의 내구성·중복 제거와 예약 실패 복구를 구현한다. 현재 포트 호출은 스케줄러나 영속 예약의 증거가 아니며 폐기 성공 뒤 예약 실패는 원인과 이벤트 전달 결과를 호출자에게 돌려준다. |
| D-57, A-14, O-04 | 전량·매도가능수량 상한의 도메인 규칙을 테스트하고 그 뒤 매도 초안 경로를 연결한다. 승인 전 수량을 줄이는 사용자 경로와 주문 승인 시 최신 사실 재검증을 구현한다. |

## 기존 테스트 변경

`crates/application/tests/ports_a02_contract.rs`의 `content_and_signal_storage_preserve_domain_types`에서 `ProposalRepository` 고정 메서드 기대 목록을 `store_draft`, `draft` 두 항목에서 두 기존 항목과 새 `current_draft`, `store_current_draft` 네 항목으로 갱신했다. 기존 두 서명과 목록 전체의 동등성 단언은 그대로다. 메인이 승인한 같은 기존 테스트 한 건의 포트 계약 갱신이다.

## 검증 경계

실제 키·계좌·API·주문은 사용하지 않는다. 가짜 저장소는 단일 프로세스 잠금의 계약만 검증한다. 프로젝트 신뢰와 현재 세션의 훅 신뢰는 확인하지 못했다. Git 훅 설치와 실제 검사 실행은 별도로 기록한다.

## 손 변형

3차 리뷰 수정·`origin/master` 병합·회귀 보강 뒤 인덱스 트리 `a379f52d2a7d907937631f95d084e0a29eb88181`의 별도 임시 사본(`target/A-14/mutations/copy`)에서 변형마다 한 번씩 컴파일·실행했다. 기준과 복원 후 시험은 통과했고, 32개 모두 컴파일·검출됐다(생존 0, 컴파일 불가 0). 임시 사본은 실행 직후 삭제했다.

| 번호 | 바꾼 것 | 검출한 테스트 |
|---|---|---|
| 1 | 재시도 상한 3→2 | `tests::calibration_head_advancing_without_signal_revision_never_saves_stale_draft` |
| 2 | 재시도 상한 3→4 | `tests::calibration_head_advancing_without_signal_revision_never_saves_stale_draft` |
| 3 | 스냅숏 제출 일치 비교 반전 | `tests::calibration_head_advancing_without_signal_revision_never_saves_stale_draft` |
| 4 | 제출 기록 일치 비교 반전 | `tests::account_switch_before_store_rejects_and_hides_old_draft` |
| 5 | 추정 가격 범위 판정 반전 | `tests::calibration_head_advancing_without_signal_revision_never_saves_stale_draft` |
| 6 | 초기 판정의 불명 장을 닫힌 장으로 | `tests::closed_and_unknown_market_never_write_or_discard` |
| 7 | 재판정의 불명 장을 닫힌 장으로 | `tests::recheck_unknown_market_fails_closed_instead_of_market_closed` |
| 8 | 재판정의 신호 유효기간 검사 제거 | `tests::pre_store_recheck_rejects_signal_expired_within_price_freshness` |
| 9 | 매도 검사를 가격 범위 검사 앞으로 이동 | `tests::sell_signal_outside_price_range_discards_and_schedules_reestimate` |
| 10 | 조회 경로 계좌 변경 거부를 부재로 통과 | `tests::stale_account_request_after_account_switch_never_supersedes_new_draft` |
| 11 | 저장 경로 계좌 변경 거부를 동시 갱신 재시도로 | `tests::account_switch_before_store_rejects_and_hides_old_draft` |
| 12 | 조회 경로 시계 역행 재시도 제거 | `tests::concurrent_monotonic_clock_race_returns_existing_and_rollback_fails_closed` |
| 13 | 저장 경로 시계 역행 재시도 제거 | `tests::concurrent_monotonic_clock_race_returns_existing_and_rollback_fails_closed` |
| 14 | 저장 직전 `proposal.check_current` 제거 | `tests::pre_store_check_current_rejects_local_clock_rollback_without_storing` |
| 15 | 저장 직전 두 번째 `recheck` 대신 `read_at` 재사용 | `tests::pre_store_recheck_rejects_late_market_close_and_stale_price` |
| 16 | 마스터 내부 ID 일치 검사 제거 | `tests::reused_symbol_with_different_master_instrument_id_fails_closed` |
| 17 | 마스터 시장·심볼 일치 검사 제거 | `tests::changed_master_identity_does_not_turn_old_signal_into_draft` |
| 18 | 발급 과제 내용 비교 반전 | `tests::calibration_head_advancing_without_signal_revision_never_saves_stale_draft` |
| 19 | 재평가한 신호 일치 비교 반전 | `tests::calibration_head_advancing_without_signal_revision_never_saves_stale_draft` |
| 20 | 생성 이벤트를 만료 이벤트로 변경 | `tests::concurrent_drafts_for_one_instrument_create_and_emit_once` |
| 21 | 매수 지정가 방향 검사 반전 | `tests::account_switch_before_store_rejects_and_hides_old_draft` |
| 22 | 지정가 내부 ID 비교 반전 | `tests::buy_intent_rejects_wrong_direction_internal_id_and_higher_limit` |
| 23 | 매수 지정가 상한 초과 거부 제거 | `tests::buy_intent_rejects_wrong_direction_internal_id_and_higher_limit` |
| 24 | 복수 캘린더 상충 검사 제거 | `tests::conflicting_market_calendars_fail_closed_without_changing_signal` |
| 25 | 프리마켓을 닫힌 장으로 분류 | `tests::kr_extended_open_sessions_create_drafts` |
| 26 | 정확히 5초인 오래된 시세 허용 | `tests::clock_advancing_during_input_read_rejects_stale_price` |
| 27 | `draft(id)` 조회의 선택 계좌 대조 제거 | `tests::account_switch_before_store_rejects_and_hides_old_draft` |
| 28 | `draft(id)` 조회의 보정 원장 위치 대조 제거 | `tests::draft_lookup_hides_proposal_after_calibration_head_advances` |
| 29 | `store_current_draft`의 `proposal.check_current` 제거 | `tests::store_current_draft_preserves_existing_draft_on_validation_errors` |
| 30 | `current_draft`의 선택 계좌 대조 제거 | `tests::stale_account_request_after_account_switch_never_supersedes_new_draft` |
| 31 | `store_current_draft`의 선택 계좌 대조 제거 | `tests::account_switch_before_store_rejects_and_hides_old_draft` |
| 32 | `store_current_draft`의 방향 일치 검사 제거 | `tests::store_current_draft_preserves_existing_draft_on_validation_errors` |
