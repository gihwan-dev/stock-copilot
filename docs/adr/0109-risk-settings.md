# ADR 0109: A-17 위험 설정 변경 유스케이스

- 상태: 확정 (2026-11-17)


## 결정

SEC-12, P-9, ADR 0039·0080·0091·0100을 따른다. ChangeRiskSettings는 전체 요청을 기존 RiskSettings와 비교한다. 주문 켜기, 시장가 허용, 몇 시간 허용, 비중 완화, 성향 증가 순서로 필요한 확인만 요청하며 모두 Confirmed여야 저장소를 한 번 호출한다. Denied 또는 Unavailable이면 저장하지 않는다. 취소·시간 초과·시스템 오류는 포트의 Unavailable이다. 주문 확인은 호출하지 않는다.

덜 위험한 변경과 주의 기준만 변경하는 경우 확인 없이 저장한다. 같은 값은 저장하지 않는다. 비중 완화는 WeightCaps::change_from, 성향 비교는 domain의 닫힌 순서를 사용한다. 금액·비율 계산은 없다. 부재는 기존 투자 성향과 주문·시장가 꺼짐으로 읽으며 조회 부재 자체를 저장하지 않는다. 몇 시간 선택 중 허용 끄기는 유스케이스가 기존 domain 규칙으로 네이티브 확인 전에 검사하고, 정합하지 않으면 ProfileRejected(HoursNotAllowed)를 반환한다. 저장소는 저장 시 같은 프로필 사실과 함께 정합성을 원자적으로 재검증한다(ADR 0039·0080).

모든 설정 쓰기의 직렬화는 조립 책임이다. 저장 성공 뒤 SettingsChanged를 발행하고 구독 실패는 Changed의 DeliveryReport로 반환한다. 저장소는 다섯 필드와 프로필을 같은 사실로 저장해야 한다. 새로운 포트·domain API·의존성·가짜 메서드·검사 끄기·기존 테스트 변경은 없다.

## 인계

| 받는 작업 | 남은 계약 |
|---|---|
| S-10, U-24 | 단일 설정 변경 진입점과 모든 쓰기의 직렬화, 실제 저장 원자성·실패 원복 |
| O-05, S-08 | 확인 결과 감사와 실제 변경의 원자적 감사 저장. 현재 포트에는 설정 저장과 감사의 트랜잭션이 없다 |
| U-24 | 네이티브 확인 창, 취소·시간 초과·오류의 Unavailable 매핑과 본인 확인 호출 0회 실측 |

## 검사

프로젝트 설정 hooks=true는 확인했으나 프로젝트 신뢰·훅 신뢰와 세션 런타임 적용은 미확인이다. Git 훅과 직접 검사로 검증한다. 최초 실행은 환경의 Xcode 라이선스 오류로 빌드되지 않았다. 사용자 승인 CommandLineTools 환경으로 재실행한다. 실제 API·키·계좌 자료는 사용하지 않는다.

## 첫 구현의 손 변형 검증

별도 임시 복사본에서 원본 대조군을 통과시킨 뒤 각각 독립 컴파일과 테스트를 실행했다. 첫 구현 커밋의 유효 변형 21개는 모두 검출했다. 최초 코드 22개 실행과 구별하며 아래는 최종 실행만 집계한다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 1 | equal-inverted | tests::settings_event_is_sent_once_after_save_and_delivery_failure_preserves_success, tests::later_denial_does_not_save_earlier_confirmed_fields, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::repository_failures_preserve_settings_and_propagate_exact_error, tests::missing_risk_settings_uses_saved_profile_and_missing_all_uses_safe_defaults, tests::every_non_confirmation_preserves_all_settings_and_never_requests_order_presence, tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 0, 테스트 100 검출 |
| 2 | equal-always | tests::repository_failures_preserve_settings_and_propagate_exact_error, tests::later_denial_does_not_save_earlier_confirmed_fields, tests::settings_event_is_sent_once_after_save_and_delivery_failure_preserves_success, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::every_non_confirmation_preserves_all_settings_and_never_requests_order_presence, tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 0, 테스트 100 검출 |
| 3 | skip-save | tests::each_field_preserves_exact_values_and_checks_only_risk_increases, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::settings_event_is_sent_once_after_save_and_delivery_failure_preserves_success | 컴파일 0, 테스트 100 검출 |
| 4 | orders-opposite | tests::every_non_confirmation_preserves_all_settings_and_never_requests_order_presence, tests::repository_failures_preserve_settings_and_propagate_exact_error, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::later_denial_does_not_save_earlier_confirmed_fields, tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 0, 테스트 100 검출 |
| 5 | orders-request-opposite | tests::repository_failures_preserve_settings_and_propagate_exact_error, tests::later_denial_does_not_save_earlier_confirmed_fields, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::each_field_preserves_exact_values_and_checks_only_risk_increases, tests::every_non_confirmation_preserves_all_settings_and_never_requests_order_presence | 컴파일 0, 테스트 100 검출 |
| 6 | orders-or | tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 0, 테스트 100 검출 |
| 7 | market-opposite | tests::repository_failures_preserve_settings_and_propagate_exact_error, tests::later_denial_does_not_save_earlier_confirmed_fields, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 0, 테스트 100 검출 |
| 8 | market-request-opposite | tests::repository_failures_preserve_settings_and_propagate_exact_error, tests::each_field_preserves_exact_values_and_checks_only_risk_increases, tests::later_denial_does_not_save_earlier_confirmed_fields, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none | 컴파일 0, 테스트 100 검출 |
| 9 | hours-opposite | tests::repository_failures_preserve_settings_and_propagate_exact_error, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::later_denial_does_not_save_earlier_confirmed_fields, tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 0, 테스트 100 검출 |
| 10 | hours-request-opposite | tests::repository_failures_preserve_settings_and_propagate_exact_error, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::each_field_preserves_exact_values_and_checks_only_risk_increases, tests::later_denial_does_not_save_earlier_confirmed_fields | 컴파일 0, 테스트 100 검출 |
| 11 | appetite-lt | tests::each_field_preserves_exact_values_and_checks_only_risk_increases, tests::repository_failures_preserve_settings_and_propagate_exact_error, tests::later_denial_does_not_save_earlier_confirmed_fields, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none | 컴파일 0, 테스트 100 검출 |
| 12 | appetite-ge | tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 0, 테스트 100 검출 |
| 13 | appetite-ne | tests::each_field_preserves_exact_values_and_checks_only_risk_increases, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none | 컴파일 0, 테스트 100 검출 |
| 14 | caps-reversed | tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 0, 테스트 100 검출 |
| 15 | caps-wrong-payload | tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 0, 테스트 100 검출 |
| 16 | appetite-wrong-payload | tests::each_field_preserves_exact_values_and_checks_only_risk_increases, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none | 컴파일 0, 테스트 100 검출 |
| 17 | orders-wrong-request | tests::each_field_preserves_exact_values_and_checks_only_risk_increases, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::every_non_confirmation_preserves_all_settings_and_never_requests_order_presence | 컴파일 0, 테스트 100 검출 |
| 18 | market-wrong-request | tests::each_field_preserves_exact_values_and_checks_only_risk_increases, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none | 컴파일 0, 테스트 100 검출 |
| 19 | hours-wrong-request | tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 0, 테스트 100 검출 |
| 20 | initial-orders-enabled | tests::missing_risk_settings_uses_saved_profile_and_missing_all_uses_safe_defaults, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::repository_failures_preserve_settings_and_propagate_exact_error, tests::later_denial_does_not_save_earlier_confirmed_fields | 컴파일 0, 테스트 100 검출 |
| 21 | initial-market-enabled | tests::missing_risk_settings_uses_saved_profile_and_missing_all_uses_safe_defaults, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::later_denial_does_not_save_earlier_confirmed_fields, tests::repository_failures_preserve_settings_and_propagate_exact_error | 컴파일 0, 테스트 100 검출 |

## 첫 구현의 자동 변이와 환경 예외

최초 자동 실행은 application만 테스트하여 testkit-ports의 연동 검사를 선택하지 않았고 14개 미검출을 보고했다. 가짜 없는 순수 변경 판정 테스트를 application에 추가했다. 재실행의 6작업 병렬 실행은 검출 12·컴파일 불가 4·시간 초과 4였다. 저장소가 제공하는 STOCK_COPILOT_MUTANTS_JOBS=1로 실행한 최종 변경분 변이는 선택 20·검출 16·컴파일 불가 4·미검출 0·시간 초과 0으로 통과했다. 컴파일 불가를 검출 수에 포함하지 않는다.

정상 커밋 훅은 session-r1의 R4 테스트 한 건만 Xcode 라이선스 오류로 실패했다. 사용자 환경 안내 2에 따라 나머지 check-ts와 check 본문을 순서대로 검증하고, 모두 통과한 경우에만 해당 커밋에 환경 예외를 적용한다. 테스트나 훅 코드는 수정하지 않았다.

## 리뷰 후 결정

몇 시간 단위 기간을 선택한 기존 프로필에서 허용을 끄는 요청은 저장 데이터 손상이 아니다. 기존 domain의 with_hours_permission(HoursPermission::default())로 먼저 검사하고 ProfileRejected(HoursNotAllowed)를 돌려준다. 이 거부는 네이티브 확인과 저장보다 앞서며 원래 설정과 프로필을 보존한다. 저장소의 원자적 정합성 재검증도 유지한다. 거부와 확인 불가는 각각 Denied·Unavailable 결과로 표현하여 NotConfirmed(Confirmed)라는 모순된 상태를 없앤다.

| 정의 대기 항목 | 받는 작업 | 현재 동작과 범위 |
|---|---|---|
| AdvancedHoursConfirmation 발급 API, 요청 연계와 재사용 계약 | 주책임 U-24, 연계 O-14·E-09, 저장 경계 S-10 | ADR 0039가 넘긴 봉인 증거 발급 API는 현재 없다. 이번 범위에서 새 domain API나 포트를 만들지 않는다. 현재는 ADR 0080에 따라 AllowHours 확인이 Confirmed인 뒤 RiskSettings 상태를 저장한다. 실제 확인 우회 결함으로 보지는 않으며 봉인 증거 연동은 정의 대기다 |
| 위험 설정 없이 프로필만 있는 초기화·마이그레이션 상태의 허용 여부와 비교 기준 | S-10 | 정상 메모리 저장은 프로필과 위험 설정을 함께 저장한다. 기존 프로필을 존중하는 현재 부재 해석은 유지하며, 실제 어댑터가 이 조합을 허용하는지는 아직 확정하지 않는다 |

리뷰의 ADR 파일명 지적은 기각한다. 사용자 지시와 implementer 규칙이 번호 없는 이 초안 경로를 명시했고 번호 부여는 메인의 병합 책임이다. 기존 테스트·domain 구현·포트·가짜는 수정하지 않았다.

## 리뷰 수정본의 검사와 손 변형

새 회귀 테스트는 수정 전 실제 InvalidData와 EnableOrders 확인 1회를 확인한 뒤 확인 호출 0회 단언에서 실패했다. 수정 후 좁은 검사 13개와 clippy가 통과했다. 자동 변경분 변이는 jobs=1에서 선택 22·검출 17·컴파일 불가 5·미검출 0·시간 초과 0으로 통과했다.

손 변형의 앞선 시도에서 컴파일 불가 2개는 검출 수에서 제외했다. 그 두 변형을 유효하게 바꾼 최종 재실행은 대조군 통과, 독립 컴파일 25개 성공, 검출 25·생존 0·컴파일 불가 0이며 복구 대조군도 통과했다. 아래 표는 수정본의 최종 실행만 집계한다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 1 | 같은 값 판정 반전 | tests::each_field_preserves_exact_values_and_checks_only_risk_increases, tests::missing_risk_settings_uses_saved_profile_and_missing_all_uses_safe_defaults, tests::settings_event_is_sent_once_after_save_and_delivery_failure_preserves_success, tests::hours_horizon_rejection_precedes_unrelated_native_confirmation, tests::every_non_confirmation_preserves_all_settings_and_never_requests_order_presence, tests::later_denial_does_not_save_earlier_confirmed_fields, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::repository_failures_preserve_settings_and_propagate_exact_error | 컴파일 성공·검출 |
| 2 | 항상 같은 값으로 판정 | tests::each_field_preserves_exact_values_and_checks_only_risk_increases, tests::every_non_confirmation_preserves_all_settings_and_never_requests_order_presence, tests::repository_failures_preserve_settings_and_propagate_exact_error, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::later_denial_does_not_save_earlier_confirmed_fields, tests::settings_event_is_sent_once_after_save_and_delivery_failure_preserves_success, tests::hours_horizon_rejection_precedes_unrelated_native_confirmation | 컴파일 성공·검출 |
| 3 | 요청 대신 이전 값 저장 | tests::each_field_preserves_exact_values_and_checks_only_risk_increases, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::settings_event_is_sent_once_after_save_and_delivery_failure_preserves_success | 컴파일 성공·검출 |
| 4 | 이전 주문 허용 비교 반전 | tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::later_denial_does_not_save_earlier_confirmed_fields, tests::repository_failures_preserve_settings_and_propagate_exact_error, tests::each_field_preserves_exact_values_and_checks_only_risk_increases, tests::every_non_confirmation_preserves_all_settings_and_never_requests_order_presence | 컴파일 성공·검출 |
| 5 | 요청 주문 허용 비교 반전 | tests::later_denial_does_not_save_earlier_confirmed_fields, tests::each_field_preserves_exact_values_and_checks_only_risk_increases, tests::every_non_confirmation_preserves_all_settings_and_never_requests_order_presence, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::repository_failures_preserve_settings_and_propagate_exact_error | 컴파일 성공·검출 |
| 6 | 주문 증가 조건을 OR로 변경 | tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 성공·검출 |
| 7 | 이전 시장가 허용 비교 반전 | tests::repository_failures_preserve_settings_and_propagate_exact_error, tests::later_denial_does_not_save_earlier_confirmed_fields, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 성공·검출 |
| 8 | 요청 시장가 허용 비교 반전 | tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::later_denial_does_not_save_earlier_confirmed_fields, tests::each_field_preserves_exact_values_and_checks_only_risk_increases, tests::repository_failures_preserve_settings_and_propagate_exact_error | 컴파일 성공·검출 |
| 9 | 이전 시간 허용 비교 반전 | tests::repository_failures_preserve_settings_and_propagate_exact_error, tests::each_field_preserves_exact_values_and_checks_only_risk_increases, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::later_denial_does_not_save_earlier_confirmed_fields | 컴파일 성공·검출 |
| 10 | 요청 시간 허용 비교 반전 | tests::repository_failures_preserve_settings_and_propagate_exact_error, tests::each_field_preserves_exact_values_and_checks_only_risk_increases, tests::later_denial_does_not_save_earlier_confirmed_fields, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none | 컴파일 성공·검출 |
| 11 | 성향 증가를 감소로 비교 | tests::later_denial_does_not_save_earlier_confirmed_fields, tests::repository_failures_preserve_settings_and_propagate_exact_error, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 성공·검출 |
| 12 | 같은 성향도 증가로 비교 | tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 성공·검출 |
| 13 | 모든 성향 차이를 증가로 비교 | tests::each_field_preserves_exact_values_and_checks_only_risk_increases, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none | 컴파일 성공·검출 |
| 14 | 비중 완화 비교 방향 반전 | tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 성공·검출 |
| 15 | 확인에 이전 상한 전달 | tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 성공·검출 |
| 16 | 확인에 이전 성향 전달 | tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 성공·검출 |
| 17 | 주문 확인을 시간 확인으로 교체 | tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::every_non_confirmation_preserves_all_settings_and_never_requests_order_presence, tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 성공·검출 |
| 18 | 시장가 확인을 주문 확인으로 교체 | tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 성공·검출 |
| 19 | 시간 확인을 시장가 확인으로 교체 | tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 성공·검출 |
| 20 | 초기 주문 켜짐 | tests::repository_failures_preserve_settings_and_propagate_exact_error, tests::later_denial_does_not_save_earlier_confirmed_fields, tests::missing_risk_settings_uses_saved_profile_and_missing_all_uses_safe_defaults, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none | 컴파일 성공·검출 |
| 21 | 초기 시장가 허용 | tests::repository_failures_preserve_settings_and_propagate_exact_error, tests::later_denial_does_not_save_earlier_confirmed_fields, tests::all_confirmations_precede_one_atomic_write_and_reductions_need_none, tests::missing_risk_settings_uses_saved_profile_and_missing_all_uses_safe_defaults | 컴파일 성공·검출 |
| 22 | 시간 허용 정합 검사 대상 반전 | tests::hours_horizon_rejection_precedes_unrelated_native_confirmation | 컴파일 성공·검출 |
| 23 | 허용 끄기를 기존 허용 유지로 검증 | tests::hours_horizon_rejection_precedes_unrelated_native_confirmation | 컴파일 성공·검출 |
| 24 | 거부를 확인 불가로 반환 | tests::later_denial_does_not_save_earlier_confirmed_fields, tests::each_field_preserves_exact_values_and_checks_only_risk_increases, tests::every_non_confirmation_preserves_all_settings_and_never_requests_order_presence | 컴파일 성공·검출 |
| 25 | 확인 불가를 거부로 반환 | tests::every_non_confirmation_preserves_all_settings_and_never_requests_order_presence, tests::each_field_preserves_exact_values_and_checks_only_risk_increases | 컴파일 성공·검출 |

## 최종 인계 책임

봉인 증거 발급 API·확인 요청 연계·일회성 및 재사용 방지 계약의 주책임은 U-24다. O-14·E-09가 승인 경로와 연결하고 S-10이 저장 경계를 맡는다. 메인은 U-24의 완료 조건 또는 열린 항목에 이 계약을 등록해야 한다. 이번 작업에서는 지시대로 tasks.md와 progress.md를 수정하지 않는다. 2회차 비판은 새 차단·중요 코드 결함이 없음을 확인했고, 남은 두 문서 보완을 반영했다.
