# A-14: 현재 신호에서 주문 초안 만들기

- 상태: 초안

## 결정

`DraftProposal`은 현재 신호 스냅숏과 해당 종목의 최신 제출·과제·독립 출처를 읽고, 선택 계좌를 지정해 현재 마스터·시세·잔고·정책·캘린더·호가 사실을 다시 가져온다. 이전 평가가 실패하고 기존 신호가 남아도 현재 제출과 평가 결과가 서로 일치하지 않거나 마스터의 내부 ID·시장·심볼이 달라지면 초안을 만들지 않는다. 평가 사유와 수량·호가 도메인 오류는 각각 보존한다.

도메인의 장 상태를 사용한다. 닫힌 장에서는 주문 가능 여부보다 장 상태를 먼저 판정해 신호를 유지하며 초안을 쓰지 않는다. 장 상태가 불명확하거나 여러 캘린더가 모순되면 닫힌 실패다. 장 판단에 필요한 입력 조회 자체가 실패하면 원래 조회 오류를 돌려준다. 열린 장에서는 내부 ID가 결합된 최신 시세의 종목과 5초 신선도를 도메인에서 확인하고 추정 기준가의 변동성 범위를 검사한다. 범위 밖인 경우 현재 스냅숏을 조건으로 해당 신호의 과제를 폐기하고 커밋의 이벤트를 발행한 뒤 재추정 예약 포트를 호출한다. 만료된 초안만으로는 신호를 폐기하지 않는다. 다시 만들 때 최신 관측을 새 카드 범위에 넣는다.

매수 수량은 도메인의 `calculate_buy_quantity`, 지정가는 `calculate_limit_price`로 계산한다. application의 `buy_limit_intent`는 도메인이 만든 수량 계획과 지정가의 방향·내부 종목 ID·시장·심볼을 결합하고 수량 계산 가격을 초과하면 재계산을 요구한다. application은 확률·범위·수량·가격·만료를 직접 산술하지 않는다. 데이터 포트는 요청받은 계좌의 사실을 제공하며 실제 계좌와 응답 결합의 진위는 어댑터의 책임이다.

`ProposalRepository::current_draft`는 수량을 다시 계산하기 전에 현재 신호와 보정 원장 및 기존 초안을 확인한다. 유효한 초안이 있으면 그 ID를 반환하고 생성 이벤트를 다시 내지 않는다. 계좌가 달라도 내부 종목 ID가 같으면 같은 범위다. `store_current_draft`는 같은 확인을 초안 저장과 동일한 직렬화 경계에서 반복한다. 두 호출이 동시에 빈 결과를 보더라도 하나만 저장하고 다른 호출에는 기존 ID를 반환한다. 만료 시각 도달이나 카드 기준 가격 범위 이탈은 도메인 판정으로 확인하며, 확인된 만료 또는 달라진 신호 스냅숏의 초안은 현재 조회에서 제외한다. 새 초안 저장이 실패하거나 가격이 되돌아와도 이전 초안을 다시 유효하게 보이지 않는다. 시계 역행과 시세 불명은 교체하지 않고 닫힌 실패다.

두 메서드는 신호 스냅숏 전체와 신호가 사용한 보정 원장 위치도 대조한다. 보정 원장만 전진해 신호 리비전은 그대로인 경쟁도 거부한다. `store_current_draft`는 저장하려는 초안의 주문 방향과 현재 신호의 방향 일치도 확인한다. 기존 단독 초안 가짜는 이 결합을 제공할 수 없어 조건부 조회·저장을 닫힌 오류로 돌려준다. 신호·초안 통합 가짜는 한 잠금에서 비교와 저장을 수행하고, 내부 종목 ID를 알 수 없는 단독 `store_draft`는 닫힌 오류로 둔다. 충돌은 처음부터 다시 읽고 세 번으로 제한한다. 저장 직전 단조 시계 경쟁으로 다른 호출이 조금 더 늦은 시각의 초안을 먼저 저장해 `ClockRollback`이 난 경우에도 다시 읽어 기존 초안을 확인하며, 실제 시계 역행이 지속되면 세 번 뒤 `DraftStatus(ClockRollback)`으로 닫힌 실패한다. 마지막 재시도가 아직 평가되지 않은 제출과 신호의 불일치였다면 `SignalChanged`, 조건부 저장 충돌이었다면 `ConcurrentUpdate`를 반환한다. `ProposalCreated`는 새 초안의 저장 성공 뒤에만 발행한다. 조회·저장·예약 실패는 원인을 보존한다. 수집 중 시간이 지나 시세가 낡거나 장이 닫힌 경우를 막기 위해 주입된 시계로 수집 뒤와 저장 직전에 다시 판정하며, 저장 직전에는 도메인의 `Proposal::check_current`로 시계 역행·만료·관측·범위를 함께 확인한다. 이 경계는 검증된 제안이나 승인 권한을 만들지 않는다.

## 매도 결정과 후속 구현

메인은 매도 초안의 기본 목표를 보유 수량 전량으로 정했다. 주문 수량은 매도가능수량을 넘지 않으며, 사용자가 승인 전에 수량을 줄일 수 있다는 전제다. 이는 현재 design 6.5의 수량 변경 불가 문구보다 나중에 내려진 결정이다. 이 작업은 수량 규칙과 사용자 조정 경로를 구현하지 않는다. 매도 신호는 보존하고 `SellDraftUnavailable`로 초안을 만들지 않는다. 매수에는 영향을 주지 않는다.

후속 작업 행 초안은 다음과 같다.

| ID | 작업 | 완료 기준 | 선행 |
|---|---|---|---|
| D-57 | 매도 초안의 기본 수량 규칙 | 기본 목표는 보유 수량 전량이며 매도가능수량을 넘지 않음. 승인 전에 사용자가 수량을 줄일 수 있다는 전제를 반영해 예제와 속성 테스트로 고정. 완료 뒤 A-14의 매도 초안 생성 경로에 연결 (A-14에서 넘어옴) | D-26, D-34 |

## 인계

| 작업 | 내용 |
|---|---|
| O-04 | 메인 결정으로 A-04 인계의 정상 초안→검증→주문 요약 연결 시험을 A-14에서 옮긴다. O-04가 A-14를 선행으로 요구하고 이 시험은 O-04 주문 게이트를 요구하는 의존 순환을 끊기 위함이다. 확인 세 응답, 요청 열 필드·횟수·순서와 공개 `user_presence_order` 계약은 그대로 검증한다. |
| S-22, S-18 | 신호·제출 상태의 리비전과 보정 원장 위치 비교, 종목별 현재 초안 조회·교체·저장을 같은 실제 DB 트랜잭션으로 구현하고 동시 연결 경쟁을 검증한다. 선택 계좌와 응답 사실의 결합도 실제 어댑터 계약 시험으로 확인한다. 단독 초안 저장소의 닫힌 실패를 실제 결합 구현으로 대체한다. |
| A-17, J-01 | 재추정 예약의 내구성·중복 제거와 예약 실패 복구를 구현한다. 현재 포트 호출은 스케줄러나 영속 예약의 증거가 아니며 폐기 성공 뒤 예약 실패는 원인과 이벤트 전달 결과를 호출자에게 돌려준다. |
| D-57, A-14, O-04 | 전량·매도가능수량 상한의 도메인 규칙을 테스트하고 그 뒤 매도 초안 경로를 연결한다. 승인 전 수량을 줄이는 사용자 경로와 주문 승인 시 최신 사실 재검증을 구현한다. |

## 기존 테스트 변경

`crates/application/tests/ports_a02_contract.rs`의 `content_and_signal_storage_preserve_domain_types`에서 `ProposalRepository` 고정 메서드 기대 목록을 `store_draft`, `draft` 두 항목에서 두 기존 항목과 새 `current_draft`, `store_current_draft` 네 항목으로 갱신했다. 기존 두 서명과 목록 전체의 동등성 단언은 그대로다. 메인이 승인한 같은 기존 테스트 한 건의 포트 계약 갱신이다.

## 검증 경계

실제 키·계좌·API·주문은 사용하지 않는다. 가짜 저장소는 단일 프로세스 잠금의 계약만 검증한다. 프로젝트 신뢰와 현재 세션의 훅 신뢰는 확인하지 못했다. Git 훅 설치와 실제 검사 실행은 별도로 기록한다.

## 손 변형

리뷰 수정 뒤 인덱스 트리 `e88c6e94b253bbebed03aab5e92d26835a52b819`의 별도 임시 사본에서 변형마다 한 번씩 컴파일·실행했다. 기준과 복원 후 시험은 통과했고, 컴파일된 28개는 모두 검출됐다. 7번 장 닫힘 변형은 사용되지 않는 열거형 오류로 컴파일되지 않아 수에 넣지 않았다. 임시 사본은 삭제했다.

| 번호 | 바꾼 것 | 검출한 테스트 |
|---|---|---|
| 1 | 재시도 상한 3→2 | `tests::calibration_head_advancing_without_signal_revision_never_saves_stale_draft` |
| 2 | 재시도 상한 3→4 | `tests::calibration_head_advancing_without_signal_revision_never_saves_stale_draft` |
| 3 | 스냅숏 제출 일치 비교 반전 | `tests::calibration_head_advancing_without_signal_revision_never_saves_stale_draft` |
| 4 | 제출 기록 일치 비교 반전 | `tests::calibration_head_advancing_without_signal_revision_never_saves_stale_draft` |
| 5 | 추정 가격 범위 판정 반전 | `tests::calibration_head_advancing_without_signal_revision_never_saves_stale_draft` |
| 6 | 불명 장을 닫힌 장으로 | `tests::closed_and_unknown_market_never_write_or_discard` |
| 8 | 마스터 내부 ID 비교 반전 | `tests::calibration_head_advancing_without_signal_revision_never_saves_stale_draft` |
| 9 | 과제 내부 ID 비교 반전 | `tests::calibration_head_advancing_without_signal_revision_never_saves_stale_draft` |
| 10 | 마스터 시장·심볼 비교 반전 | `tests::calibration_head_advancing_without_signal_revision_never_saves_stale_draft` |
| 11 | 발급 과제 내용 비교 반전 | `tests::calibration_head_advancing_without_signal_revision_never_saves_stale_draft` |
| 12 | 재평가한 신호 비교 반전 | `tests::calibration_head_advancing_without_signal_revision_never_saves_stale_draft` |
| 13 | 저장 충돌 시 즉시 중단 | `tests::calibration_head_advancing_without_signal_revision_never_saves_stale_draft` |
| 14 | 생성 이벤트를 만료 이벤트로 | `tests::conflicts_reread_entire_snapshot_and_stop_after_three_attempts` |
| 15 | 신호 없음을 닫힌 장으로 | `tests::no_current_signal_returns_without_a_draft` |
| 16 | 매수 지정가 방향 비교 반전 | `tests::buy_intent_rejects_wrong_direction_internal_id_and_higher_limit` |
| 17 | 지정가 내부 ID 비교 반전 | `tests::buy_intent_rejects_wrong_direction_internal_id_and_higher_limit` |
| 18 | 지정가 심볼 비교 반전 | `tests::buy_intent_rejects_wrong_direction_internal_id_and_higher_limit` |
| 19 | 매수 계획 가격 상한 비교 반전 | `tests::buy_intent_rejects_wrong_direction_internal_id_and_higher_limit` |
| 20 | 시세 내부 ID 비교 반전 | `tests::buy_intent_rejects_wrong_direction_internal_id_and_higher_limit` |
| 21 | 정확히 5초인 오래된 시세 허용 | `tests::clock_advancing_during_input_read_rejects_stale_price` |
| 22 | 조건부 저장 스냅숏 비교 반전 | `tests::conflicts_reread_entire_snapshot_and_stop_after_three_attempts` |
| 23 | 매수 호가를 매도 방향으로 계산 | `tests::calibration_head_advancing_without_signal_revision_never_saves_stale_draft` |
| 24 | 제안 계좌를 다른 계좌로 변경 | `tests::open_market_saves_draft_and_public_validation_remains_closed` |
| 25 | 조건부 저장의 보정 원장 대조 제거 | `tests::calibration_head_advancing_without_signal_revision_never_saves_stale_draft` |
| 26 | 제출·신호 불일치를 저장 충돌로 분류 | `tests::failed_a13_identity_transition_leaves_old_signal_unusable_for_drafting` |
| 27 | 마지막 재시도 원인을 버림 | `tests::failed_a13_identity_transition_leaves_old_signal_unusable_for_drafting` |
| 28 | 캘린더 충돌 검사 제거 | `tests::conflicting_market_calendars_fail_closed_without_changing_signal` |
| 29 | 닫힌 장 전에 매수 제한 검사 | `tests::closed_market_takes_precedence_over_buy_restriction` |
