# A-17: 위험 설정 변경

- 상태: 초안

## 결정

SEC-12, P-9, ADR 0039·0080·0091·0100을 따른다. ChangeRiskSettings는 전체 요청을 기존 RiskSettings와 비교한다. 주문 켜기, 시장가 허용, 몇 시간 허용, 비중 완화, 성향 증가 순서로 필요한 확인만 요청하며 모두 Confirmed여야 저장소를 한 번 호출한다. Denied 또는 Unavailable이면 저장하지 않는다. 취소·시간 초과·시스템 오류는 포트의 Unavailable이다. 주문 확인은 호출하지 않는다.

덜 위험한 변경과 주의 기준만 변경하는 경우 확인 없이 저장한다. 같은 값은 저장하지 않는다. 비중 완화는 WeightCaps::change_from, 성향 비교는 domain의 닫힌 순서를 사용한다. 금액·비율 계산은 없다. 부재는 기존 투자 성향과 주문·시장가 꺼짐으로 읽으며 조회 부재 자체를 저장하지 않는다. 몇 시간 선택 중 허용 끄기의 정합 검사는 저장소가 같은 프로필 사실과 함께 원자적으로 검증한다(ADR 0039·0080).

모든 설정 쓰기의 직렬화는 조립 책임이다. 저장 성공 뒤 SettingsChanged를 발행하고 구독 실패는 Changed의 DeliveryReport로 반환한다. 저장소는 다섯 필드와 프로필을 같은 사실로 저장해야 한다. 새로운 포트·domain API·의존성·가짜 메서드·검사 끄기·기존 테스트 변경은 없다.

## 인계

| 받는 작업 | 남은 계약 |
|---|---|
| S-10, U-24 | 단일 설정 변경 진입점과 모든 쓰기의 직렬화, 실제 저장 원자성·실패 원복 |
| O-05, S-08 | 확인 결과 감사와 실제 변경의 원자적 감사 저장. 현재 포트에는 설정 저장과 감사의 트랜잭션이 없다 |
| U-24 | 네이티브 확인 창, 취소·시간 초과·오류의 Unavailable 매핑과 본인 확인 호출 0회 실측 |

## 검사

프로젝트 설정 hooks=true는 확인했으나 프로젝트 신뢰·훅 신뢰와 세션 런타임 적용은 미확인이다. Git 훅과 직접 검사로 검증한다. 최초 실행은 환경의 Xcode 라이선스 오류로 빌드되지 않았다. 사용자 승인 CommandLineTools 환경으로 재실행한다. 실제 API·키·계좌 자료는 사용하지 않는다.

## 손 변형 검증

별도 임시 복사본에서 원본 대조군을 통과시킨 뒤 각각 독립 컴파일과 테스트를 실행했다. 최종 코드의 유효 변형 21개는 모두 검출했다. 최초 코드 22개 실행과 구별하며 아래는 최종 실행만 집계한다.

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

## 자동 변이와 환경 예외

최초 자동 실행은 application만 테스트하여 testkit-ports의 연동 검사를 선택하지 않았고 14개 미검출을 보고했다. 가짜 없는 순수 변경 판정 테스트를 application에 추가했다. 재실행의 6작업 병렬 실행은 검출 12·컴파일 불가 4·시간 초과 4였다. 저장소가 제공하는 STOCK_COPILOT_MUTANTS_JOBS=1로 실행한 최종 변경분 변이는 선택 20·검출 16·컴파일 불가 4·미검출 0·시간 초과 0으로 통과했다. 컴파일 불가를 검출 수에 포함하지 않는다.

정상 커밋 훅은 session-r1의 R4 테스트 한 건만 Xcode 라이선스 오류로 실패했다. 사용자 환경 안내 2에 따라 나머지 check-ts와 check 본문을 순서대로 검증하고, 모두 통과한 경우에만 해당 커밋에 환경 예외를 적용한다. 테스트나 훅 코드는 수정하지 않았다.
