# T-03 속도 제한기

- 상태: 초안

## 결정

API-1·API-2, R-01, 고정 OpenAPI와 toss-openapi-overview의 Rate Limits를 따른다. GET만 대상으로 한다. 요청 간 최소 간격을 ceil(5,000,000,000 / (3 × 한도)) 나노초로 정한다. 최초1건은 허용하고 이후 연속 전송 간격으로 지속 요청률을60% 이하로 유지한다. 짧은 창의 정수 요청 수가 언제나 실수0.6L 이하라는 뜻은 아니다. 초당1회 그룹에도0.6회/초를 표현한다. 정수 회수로 내림해 영구 정지시키지 않는다. API 그룹 안에서는 tokio FIFO Mutex를 전송·응답·재시도 동안 보유한다. 다른 그룹은 별도 Mutex다. 큐에서 취소된 future는 제거되며 이미 보낸 요청의 소비와429 대기 기한은 취소로 되돌리지 않는다.

시간 관측은 주입한 application::Clock뿐이며 대기는 RequestWait를 주입한다. TokioRequestWait는 제품 대기 구현이다. 공개 new는 endpoint, Arc<dyn Clock>, Arc<dyn RequestWait>만 받는다. 임의 URL·헤더·메서드 API는 추가하지 않는다. 생성자 구조 검사와 양성 고정 자료는 승인된 새 주입 서명으로 갱신하고 옛 서명은 거부한다. 기존 단언은 보존한다.

X-RateLimit-Limit는 비어 있지 않은 ASCII 정수 u32만 해석한다. 잘못된 값과 넘침은 무시하고 기존 값을 유지한다. 정상 값은 증가와 감소 모두 그대로 따른다.0은 요청 간격을 정의할 수 없는 사용할 수 없는 용량으로 처리해 마지막 양수 한도를 유지한다. 임의로 기본값을 늘리거나 탐침을 추가하지 않는다. Remaining/Reset은 재충전 예측에 사용하지 않는다. 요청 간격은 마지막 실제 전송 시각을 기준으로 새 한도를 즉시 반영한다. Clock 역행에는 마지막 전송 시각과 대기 기한을 유지한다.

| 그룹 | 문서 한도/초 | 초기 적용 한도/초 | 사용률/초 |
|---|---|---|---|
| ACCOUNT, STOCK_ALL | 1 | 1 | 0.6 |
| ASSET, TRANSACTION, STOCK, RANKING, MARKET_INDICATOR_CHART, SECTOR, SECTOR_RANKING, ORDER_HISTORY | 5 | 5 | 3 |
| STOCK_TRADING_TREND, MARKET_INDICATOR | 10 | 10 | 6 |
| MARKET_INFO | 3 | 3 | 1.8 |
| MARKET_DATA | 15 | 15 | 9 |
| MARKET_DATA_CHART | 20 | 20 | 12 |
| ORDER_INFO | 6, 피크3 | 3 | 1.8 |

ORDER_INFO는 주입 시계의 시간대·피크 전환 정책을 추가하지 않고 문서의 가장 작은 피크 한도로 시작한다. 이후 유효한 서버 헤더가 우선한다. MARKET_INDICATOR_PRICE는 스펙에 대응 GET이 없어 따로 큐를 만들지 않는다. AUTH·ORDER·CONDITIONAL_ORDER·CONDITIONAL_ORDER_HISTORY는 현재 읽기 열거형 밖이므로 이 모듈의 전송 경로에 없다.

| 조건 | 동작 |
|---|---|
| GET429 | 최대3회 재시도, 총4회 전송 |
| Retry-After | overview가 정한 초 형식. ASCII 정수를 i128 포화 계산으로 해석. 날짜 형식은 미지원 |
| 누락·잘못된 Retry-After | 보수적인1초를 적용 |
| 지수 백오프 | Retry-After에1초,2초,4초를 더함. 상한4초. 그룹 대기와는 큰 기한을 적용 |
| 재시도 소진 | 마지막429의 상태·본문·헤더를 가공 없이 반환 |
| Clock 범위를 넘는 대기 기한 | 현재429 원본 반환, 같은 그룹 후속GET은 RateLimited로 종료. 조기 전송 없음 |
| 전송 오류·다른 상태 | 자동 재시도하지 않고 현재 결과를 반환 |
| 토큰 발급 | 기존 고정 POST 단발 경로 유지 |

API-2에는 횟수·상한이 없다.3회와4초 상한은 새 설정을 늘리지 않는 유한 정책이다. overview의 jitter 권고는 이번 필수 요구가 아니므로 난수 의존성을 추가하지 않는다. Retry-After의 지정 대기보다 빨리 재시도하지 않는다.

Clock은 domain::Timestamp의 UTC 1~9999년 범위만 표현한다. Retry-After와 백오프의 합산 기한 또는 다음 전송 간격이 최대 UTC 시각 9999-12-31T23:59:59.999999999Z를 넘으면 도달할 수 없는 대기를 시작하지 않는다. 해당429는 현재 받은 상태·본문·헤더 그대로 반환하며 더 재시도하지 않는다. 그룹에는 원래의 먼 차단 기한을 남겨 조기 전송을 막고, 같은 클라이언트·그룹의 후속GET은 전송·수면 없이 TossHttpError::RateLimited로 종료한다. 다른 그룹은 진행한다. 임의로 Retry-After를 짧게 자르거나 이전 유효 응답으로 대체하지 않는다. 표현 가능한 긴 기한은 그대로 기다리고 호출자는 future를 취소할 수 있다. 서버 지시를 준수하면서 반환 불가능한 future를 만들지 않는 가장 단순하고 보수적인 정책이다. 새로운 사용자 설정이나 조작 절차는 추가하지 않는다.


## 승인된 기존 시험 변경

메인이429 시험의 fixture·생성자 주입 변경과 다른 기존 시험의 생성자 호출부 변경을 승인했다. 기존 단언은 보존한다. testkit의 단발 LocalHttpServer는 유지하고 serve_sequence/ServeSequence를 추가한다. 테스트 자료는 루프백 가짜 응답이며 실제 API는 호출하지 않는다. 세션 프로젝트·훅 신뢰는 확인할 수 없고 Git 훅 설치만 확인했다.

## 의존성과 보호 파일

새 외부 패키지는 없다. 전체 작업 공간 컴파일에서 앱 조립부의 기존 생성자 시험도 주입 인자를 요구했다. 해당 시험의 단언을 보존하고 A-04의 testkit-ports::SteppedClock과 TokioRequestWait를 주입했다. 앱의 testkit-ports 개발 의존성과 Cargo.lock의 내부 간선만 갱신한다. testkit은 I/O 도우미만 남기고 application/domain에 의존하지 않는다. 기존 tokio=1.54.1의 sync 기능만 추가한다(MIT). 추가 전이 패키지는 없다. xtask의 닫힌 생성자 서명에 이번 주입 서명만 등록한다. 새 ADR 등록에 따른 Codex 설정·샌드박스·훅 해시는 저장소 생성기로 갱신한다. 검사 끄기 속성은 없다.

## 인계

- T-02: TossHttpClient::new의 Clock/RequestWait 주입을 조립부에서 공유한다. 토큰 발급은 자동 재시도하지 않는다. AUTH 그룹 제한·단일 토큰 정책은 토큰 관리 소유다.
- T-05, T-20: RateLimited는 서버 지정 차단을 기다릴 수 없는 상태이며 새 클라이언트를 만들어 우회하지 않는다. 상위의 대기 분류·표시 계약에 연결한다. 모든 읽기 호출은 같은 클라이언트를 공유해야 그룹 한도가 공유된다. 서로 다른 클라이언트 객체 사이에 전역 큐는 없다. serve_sequence는 준비된 응답 수만큼 요청을 받고 순서대로 원시 요청을 반환한다.
- T-12, 앱 조립: 실제 Clock과 TokioRequestWait를 주입한다. 실제 키 검증은 사용자 몫이다.

## 검증

구현 커밋과 리뷰·비판·검사 결과 및 손 변형은 후속 실행 결과로 기록한다. 미실행 검사를 통과로 세지 않는다.

## 실제 손 변형

S-01 master와 생존성 수정을 반영한 제품 코드를 mktemp 복사본에서38개 독립 변형으로 실행했다. 제한기36개는 cargo test --offline --locked -p broker-toss --lib rate_limiter::tests,429 한도 관측 생략 및 불가능 기한 반환 판정2개는 해당 루프백 통합 시험으로 확인했다. 전부 컴파일 후 테스트 실패로 검출됐으며 기준·복구 대조군도 통과했다. 이전23·29·32·36개 및 비판자의 별도4개와 중복 합산하지 않는다. 중간의 시간 초과1회와 변수 참조가 맞지 않아 컴파일되지 않은2회는 검출 수에서 제외했다. 변수 참조를 고친 두 변형의 별도 재실행은 실제 컴파일 후 검출됐다.

| 번호 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| 1 | 한도1을2로 | all_groups_have_documented_conservative_intervals, fallback_limits_match_overview_including_peak_minimum | 검출 |
| 2 | 한도3을4로 | all_groups_have_documented_conservative_intervals, fallback_limits_match_overview_including_peak_minimum | 검출 |
| 3 | 한도10을11로 | all_groups_have_documented_conservative_intervals, fallback_limits_match_overview_including_peak_minimum | 검출 |
| 4 | 시세한도15를16으로 | all_groups_have_documented_conservative_intervals, fallback_limits_match_overview_including_peak_minimum | 검출 |
| 5 | 차트한도20을21로 | all_groups_have_documented_conservative_intervals, fallback_limits_match_overview_including_peak_minimum | 검출 |
| 6 | 기본한도5를6으로 | all_groups_have_documented_conservative_intervals, fallback_limits_match_overview_including_peak_minimum, fifo_cancelled_waiter_is_removed_and_retry_deadline_survives_cancellation, noncancelled_waiters_keep_poll_registration_order, sixty_percent_before_exact_and_after_boundary_and_clock_rollback | 검출 |
| 7 | 사용률분자5를6으로 | all_groups_have_documented_conservative_intervals, reduction_applies_to_previous_send_and_groups_are_independent, sixty_percent_before_exact_and_after_boundary_and_clock_rollback | 검출 |
| 8 | 사용률분모3을2로 | all_groups_have_documented_conservative_intervals, reduction_applies_to_previous_send_and_groups_are_independent, sixty_percent_before_exact_and_after_boundary_and_clock_rollback | 검출 |
| 9 | 간격올림제거 | all_groups_have_documented_conservative_intervals, sixty_percent_before_exact_and_after_boundary_and_clock_rollback | 검출 |
| 10 | 서버한도오프셋 | headers_are_strict_and_zero_keeps_the_last_usable_limit | 검출 |
| 11 | 빈머리말판정반전 | headers_are_strict_and_zero_keeps_the_last_usable_limit, reduction_applies_to_previous_send_and_groups_are_independent | 검출 |
| 12 | 숫자문법all을any로 | headers_are_strict_and_zero_keeps_the_last_usable_limit | 검출 |
| 13 | 전송시각오프셋 | fifo_cancelled_waiter_is_removed_and_retry_deadline_survives_cancellation, reduction_applies_to_previous_send_and_groups_are_independent, sixty_percent_before_exact_and_after_boundary_and_clock_rollback | 검출 |
| 14 | 간격덧셈을뺄셈으로 | only_deadlines_beyond_the_clock_range_fail_without_waiting, reduction_applies_to_previous_send_and_groups_are_independent, sixty_percent_before_exact_and_after_boundary_and_clock_rollback | 검출 |
| 15 | 대기최댓값을최솟값으로 | only_deadlines_beyond_the_clock_range_fail_without_waiting, reduction_applies_to_previous_send_and_groups_are_independent, sixty_percent_before_exact_and_after_boundary_and_clock_rollback | 검출 |
| 16 | 0용량수용 | headers_are_strict_and_zero_keeps_the_last_usable_limit, zero_header_keeps_the_last_usable_capacity | 검출 |
| 17 | 정각허용제거 | fifo_cancelled_waiter_is_removed_and_retry_deadline_survives_cancellation, reduction_applies_to_previous_send_and_groups_are_independent, sixty_percent_before_exact_and_after_boundary_and_clock_rollback | 검출 |
| 18 | 대체대기1초를2초로 | retry_seconds_and_backoff_are_additive_capped_and_strict | 검출 |
| 19 | 첫백오프2초 | overflowing_retry_after_never_shortens_the_requested_wait, retry_seconds_and_backoff_are_additive_capped_and_strict | 검출 |
| 20 | 둘째백오프3초 | fifo_cancelled_waiter_is_removed_and_retry_deadline_survives_cancellation, retry_seconds_and_backoff_are_additive_capped_and_strict | 검출 |
| 21 | 백오프상한8초 | retry_seconds_and_backoff_are_additive_capped_and_strict | 검출 |
| 22 | RetryAfter차감 | fifo_cancelled_waiter_is_removed_and_retry_deadline_survives_cancellation, long_representable_retry_after_is_waited_in_bounded_chunks, overflowing_retry_after_never_shortens_the_requested_wait, retry_seconds_and_backoff_are_additive_capped_and_strict, unrepresentable_deadline_does_not_suspend_admission | 검출 |
| 23 | 재시도기한축소 | fifo_cancelled_waiter_is_removed_and_retry_deadline_survives_cancellation, long_representable_retry_after_is_waited_in_bounded_chunks, overflowing_retry_after_never_shortens_the_requested_wait, retry_seconds_and_backoff_are_additive_capped_and_strict, unrepresentable_deadline_does_not_suspend_admission | 검출 |
| 24 | 캔들그룹변경 | every_read_route_uses_its_openapi_group_and_path | 검출 |
| 25 | 주문조회그룹변경 | every_read_route_uses_its_openapi_group_and_path | 검출 |
| 26 | 환율달력그룹변경 | every_read_route_uses_its_openapi_group_and_path | 검출 |
| 27 | 지표캔들그룹변경 | every_read_route_uses_its_openapi_group_and_path | 검출 |
| 28 | 계좌그룹변경 | every_read_route_uses_its_openapi_group_and_path | 검출 |
| 29 | 송신 기록 삭제 | fifo_cancelled_waiter_is_removed_and_retry_deadline_survives_cancellation, reduction_applies_to_previous_send_and_groups_are_independent, sixty_percent_before_exact_and_after_boundary_and_clock_rollback | 검출 |
| 30 | 십진 자릿수 배수 변경 | long_representable_retry_after_is_waited_in_bounded_chunks, overflowing_retry_after_never_shortens_the_requested_wait, unrepresentable_deadline_does_not_suspend_admission | 검출 |
| 31 | Retry-After 넘침을 기본 대기로 복귀 | overflowing_retry_after_never_shortens_the_requested_wait, unrepresentable_deadline_does_not_suspend_admission | 검출 |
| 32 | Clock 최대 기한의 등호 거부 | only_deadlines_beyond_the_clock_range_fail_without_waiting | 검출 |
| 33 | Clock 범위 검사 반전 | fifo_cancelled_waiter_is_removed_and_retry_deadline_survives_cancellation, long_representable_retry_after_is_waited_in_bounded_chunks, only_deadlines_beyond_the_clock_range_fail_without_waiting, overflowing_retry_after_never_shortens_the_requested_wait, reduction_applies_to_previous_send_and_groups_are_independent, sixty_percent_before_exact_and_after_boundary_and_clock_rollback, unrepresentable_deadline_does_not_suspend_admission | 검출 |
| 34 | 불가능 cooldown 판정 반전 | only_deadlines_beyond_the_clock_range_fail_without_waiting | 검출 |
| 35 | 429에서 한도 관측 생략 | a_429_limit_reduction_controls_the_next_attempt_even_with_zero_retry_after | 검출 |
| 36 | 429 반환 시 불가능 기한 판정 반전 | an_unrepresentable_retry_returns_raw_429_and_fails_closed_for_its_group | 검출 |
| 37 | 거부 시 차단 기한 삭제 | unrepresentable_deadline_does_not_suspend_admission | 검출 |
| 38 | 거부 시 마지막 송신 기록 삭제 | unrepresentable_deadline_does_not_suspend_admission | 검출 |

## 승인된 test-diff 9건

아래9개 시험의 기존 단언은 그대로다. 기존 support/impl 변경 지적은 없다. 메인의 이번 승인에 따라 이 변경을 담는 구현 커밋에만 LEFTHOOK=0 예외를 적용하며, 나머지 검사를 먼저 확인한다.

| 파일 | 시험 | 전 → 후 |
|---|---|---|
| crates/adapters/broker-toss/tests/http.rs | broken_transport_never_returns_the_request_or_response_body | endpoint 생성자 → 같은 endpoint와 가짜 Clock/대기 주입 |
| crates/adapters/broker-toss/tests/http.rs | local_transport_preserves_status_headers_and_bytes_without_debug_secrets | endpoint 생성자 → 같은 endpoint와 가짜 Clock/대기 주입; 단발429 → 같은 응답4개, 첫 요청 자료를 기존 단언에 전달 |
| crates/adapters/broker-toss/tests/http.rs | query_and_account_values_are_encoded_and_sent_in_distinct_places | endpoint 생성자 → 같은 endpoint와 가짜 Clock/대기 주입 |
| crates/adapters/broker-toss/tests/http.rs | redirects_are_returned_without_following_the_location_or_leaking_it | endpoint 생성자 → 같은 endpoint와 가짜 Clock/대기 주입 |
| crates/adapters/broker-toss/tests/http.rs | token_issuance_uses_only_the_fixed_form_endpoint_and_preserves_raw_body | endpoint 생성자 → 같은 endpoint와 가짜 Clock/대기 주입 |
| crates/adapters/broker-toss/tests/http.rs | zero_port_and_invalid_secret_are_closed_errors | endpoint 생성자 → 같은 endpoint와 가짜 Clock/대기 주입 |
| crates/adapters/broker-toss/tests/http_limits.rs | body_limit_includes_exactly_eight_mib_and_rejects_the_next_byte | endpoint 생성자 → 같은 endpoint와 가짜 Clock/대기 주입 |
| apps/desktop/src-tauri/src/main.rs | composition_root_constructs_the_fixed_product_endpoint_without_sending_requests | endpoint 생성자 → 같은 endpoint와 SteppedClock/TokioRequestWait 주입 |
| crates/xtask/src/arch_check/source/endpoint.rs | client_construction_is_private_and_requires_the_canonical_constructor | 양성 입력 문자열의 new(endpoint) → new(endpoint, Clock, RequestWait); is_empty 단언 보존 |

just mutants-diff의 첫 실행은 앱 시험의 이전 생성자 호출로 컴파일 실패했다. 호출부를 고친 재실행은 당시 승인8건의 test-diff 선행 게이트에서 종료1로 중단됐다. 자동 변이 카탈로그·검출 성공을 주장하지 않는다. 검사 규칙을 우회하지 않았고 별도 손 변형23건이 보완 증거다.

추가 자체 점검: 과대한 Retry-After는 하루 이하 대기로 나눠 Tokio 타이머 넘침을 피한다. 날짜를 앞당기거나 지정 기한을 줄이지 않는다.429와 한도 헤더는 본문을 읽기 전에 저장하여 본문 잘림·취소 때도 그룹 대기가 남는다. 잘린429 뒤 다음 호출이 가짜8초 뒤에만 전송되는 루프백 회귀가 통과했다.

## 최초 구현6949427 전 확인

최종 작업 공간 nextest는1450/1450 통과, 건너뜀0이다. 속성 반복 수4096을 유지했다. 커버리지 대상1048/1048 통과 및 coverage-check 위반 없음이다. 전체 clippy, format, deny의 네 검사, arch/source/raw/disable/sql/migration/capability 검사, 생성물·스키마·Codex 설정·훅 해시 및 hooks-installed가 통과했다. 새 검사 끄기는0건이다.

check-fast는 문서 변경으로 전체 검사에 승격되어 프런트엔드와 하네스 Node 시험498+72+57+87개를 모두 통과한 뒤 cfg(test) 공용 모듈 이름 규칙에서 실패했다. 해당 모듈을 규칙에 맞게 고치고 Rust 정적 검사 및 나머지 단계를 개별 실행해 통과했다. check-fast 명령 자체가 종료0이었다고 보고하지 않는다. test-diff의 승인8건은 그대로 보고한다.

처음 수동 전체 nextest에는 지정 pnpm PATH가 빠져 기존 fresh_node 시험이 구형 Corepack shim의 누락된 pnpm.cjs 때문에 실패했고199개가 실행되지 않았다. 지정 PATH에서 해당 시험1개와 최종 전체1450개가 통과했다. 실패 기록을 통과로 바꾸지 않는다.

## 1차 리뷰 수정 결정

한도0의 무한 대기는 실제 회귀로 확인해 수정했다. 스펙은0에 대한 회복 절차나 유효 용량 최솟값을 정하지 않는다. 사용할 수 없는0을 무시하고 이미 확인한 양수 한도를 유지하는 가장 단순한 정책을 택한다. 큰 Retry-After 및 벽시계 역행 대기는 요구대로 보수적으로 유지한다.33개 ReadRoute와 스펙의 그룹·경로 대조,429 한도 감소 및 비취소 FIFO 시험을 보완했다. 옛 생성자 양성 고정 자료는 새 승인 서명으로 갱신했으며 기존 단언은 유지했다.

## 최신 master 반영 후 검사

master bec3895a13aaa0150616c5a3403739c91a2fc78c 수신 뒤 just base=<동일 SHA> check-fast를 실행했다. 문서 변경으로 전체 just check에 승격됐고 전체1466/1466, 커버리지1058/1058 및 coverage-check 위반 없음까지 통과했다. 마지막 test-diff는 위9건으로 종료1이며 다른 실패는 없다. 이어지는 disable-report·disable-report-since·hooks-installed도 별도 실행해 통과했고 새 끄기는0건이다. 자동 mutants-diff 재실행도 같은 승인9건의 선행 게이트에서 종료1이므로 자동 변이 통과를 주장하지 않는다.

## 리뷰2 수정과 인계

숫자로만 된 Retry-After에는 스펙상 최댓값이 없다. u64 넘침을 기본1초로 바꾸는 처리를 제거하고 십진수 각 자릿수를 i128 포화 누적한다. 나노초 및 마감 시각도 포화 계산한다. 표현 범위를 넘는 지정 대기는 가장 먼 마감으로 남겨 더 이른 전송을 허용하지 않는다. 잘못된 문자와 누락의 기본 대기는 그대로다.

중복 헤더 해석은 스펙에 정의가 없어 기존 reqwest 첫 값 추출을 유지한다. ORDER_INFO는 첫 최신 한도 헤더를 관측한 즉시 줄어든 간격을 적용한다. 실제 피크·승인 최신가 PERF-3·토큰 만료 연계는 각각 해당 통합 작업에서 확인한다. FIFO를 깨는 우선순위나 사용자 설정을 새로 만들지 않는다.

A-04 통합으로 testkit은 I/O만, 포트 가짜는 testkit-ports로 분리됐다. 처음 추가했던 FixedClock은 최종 패치에서 제거하고 공유 SteppedClock을 생성 시험에 주입한다. 제품에서 실제 GET을 호출할 때 전진하는 실제 Clock을 넣어야 한다는 인계는 유지한다.

## A-04 반영 최종 후보 검사

기준775bc89f0d2876458ca51927db94766b13ad9950에서 전체 승격 check-fast를 실행해1544/1544 및 커버리지1059/1059, coverage-check 위반 없음이 확인됐다. 종료1의 유일한 원인은 승인된 test-diff9건이다. 후속 검사 끄기 보고와 훅 설치도 별도 통과했다. 자동 변이는 같은 선행 게이트에서 중단됐고 수치상 통과로 세지 않는다. 원본8acabec에 넘침 회귀를 붙인 별도복사본은 컴파일 후 실패, 포화 계산 수정본은 동일 시험 통과다. 최초 in-place 회귀 명령은 의존성 컴파일 중 수정이 반영돼 통과했으므로 red 증거로 세지 않았다.

## 최종 시험 보강

세 번째 Opus 뒤 제품 코드를 바꾸지 않고 관측값 초기화, 전송 중 다른 그룹 완료, HTTP500/503 및 Transport 비재시도, overview fallback 한도 대조를 보완했다. broker82/82 및 전체1548/1548 통과다. 제품 부분 바이트는 세 번째 리뷰 대상ee10c01과 같고 커버리지1059/1059 통과를 유지한다. 재시도 횟수 상한은3회이며 전체 호출의 벽시계 반환 기한은 아니다. caller는 future를 취소할 수 있고 서버 지정 대기를 임의로 줄이지 않는다.

## 최종 비판3 이후 생존성 수정

최종 독립 비판은 도달 불가능한 Clock 기한의 실제 생존성 문제를 중요1건으로 채택했다. 장기 대기 자체와 모든 호출의 짧은 반환 기한이 원 요구라는 주장은 구분한다. 수정 전 회귀는 컴파일 후 Pending 때문에 실패했고, 수정 후 같은 회귀가 RateLimited로 즉시 완료됨을 확인했다. 실제 GET에서는 아주 긴 숫자 헤더의 현재429 원본 보존, 같은 그룹 후속 요청의 전송 없는 종료, 다른 그룹200 완료를 함께 고정했다. 86400초의 표현 가능한 대기는 네 번의429까지259207초를 온전히 기다린다. 최대 Clock 기한 정각·1ns 초과 및 마지막 전송 간격이 범위를 넘는 경계도 검사한다. 이후의 전체 검사와 범위 한정 새 Codex 검증은 리뷰 기록에 별도 적는다.


최종 생존성 수정본의 최신master 기준 검사는 전체1584/1584, 커버리지1059/1059 및coverage-check 위반 없음이다. check-fast 자체는 승인test-diff9건으로 종료1이며 그 밖의 검사는 통과했다. 손 변형36개는 컴파일 후 전부 검출했고 기준·복구도 통과했다. 자동 변이는 동일test-diff 게이트에서 중단돼 통과로 세지 않는다. 새 검사 끄기는0건이다.


새 Codex의 범위 한정 독립 검증은fc6f6db에서 차단0·중요0, 최종49/49와 별도 손 변형4개 검출이었다. 검증 뒤 부모는 거부 시 송신·차단 기록을 보존하는 직접 단언만 보강하고 각각의 삭제 변형2개를 검출했다. 제품 부분은 독립 검증 대상과 같다. 검증의 기준450fc1e는 대상의 조상이 아니므로 두SHA 직접 비교로 한정하며 공통 조상은69ae7cd다. 자세한 범위와 준비 실패는 리뷰 기록에 적었다.


상태 보존 회귀의 최종 스코프 표현은 선택16개와clippy를 다시 통과했고 두 삭제 변형도 검출했다. 제품 부분은 독립 검증본과 동일하다. 부모의 최종 평가는 차단0·중요0이며 실제GET 동시FIFO 통합 시험은 비차단 인계로 남긴다.


## T-02·T-20 최종 통합

메인의 추가 승인으로T-20 db986ca,DS-07 92a5eae,T-02 4a587ef3df592cf56a207739a7a59ad620e0056d를수신했다. TokenManager는 이미보관한TossHttpClient의get을사용하므로같은제한기를통과한다.토큰재발급도클라이언트를교체하지않아GET예산이이어진다.토큰POST자동재시도는없다.AUTH그룹의별도전송제한은이번승인범위밖인계로남긴다.

T-02인증상태시험은기존수동FakeClock을보존하고클라이언트대기에별도의전진하는가짜Clock/RequestWait를주입한다.새통합회귀는인증Clock과HTTPClock/대기를같은가짜시간으로공유해토큰재발급전후두GET간격과429대기,POST단발을검증한다.응답을보류한옛요청과새요청의인증경쟁시험은새요청을Quotes그룹으로준비한다.같은그룹FIFO를깨지않고기존인증상태·요청수단언을모두보존한다.

T-20 serve_recordings와현재200·404녹화는단발도우미를그대로사용하고클라이언트생성에가짜시간만주입한다.testkit의ScriptedHttpServer(scripted_http_server.rs)와serve_sequence(http_server.rs)는둘다유지한다.기능중복정리는메인후속작업에인계하며이번에합치지않는다.별도4회차리뷰는메인지시에따라진행하지않고통합범위와회귀결과를기록한다.


## 최신 master 대비 승인된 test-diff 23건

기준은T-02병합4a587ef3df592cf56a207739a7a59ad620e0056d다.최초9건에T-20시험1건,T-02시험9건·준비도우미4건이추가돼총23건(시험19·도우미4)이다.도우미는old endpoint-only 생성자에서가짜Clock/RequestWait를받는client준비로바뀌었다.별도인증경쟁시험은새요청을Quotes그룹으로준비하고기존오류·상태·호출수단언을보존했다.생성자·준비변경은메인의(b)(d)승인범위다.

| 종류 | 항목 |
|---|---|
| 기존 시험 | `broker-toss::contracts contract_tests::recordings_replay_over_local_client` |
| 기존 시험 | `broker-toss::http tests::broken_transport_never_returns_the_request_or_response_body` |
| 기존 시험 | `broker-toss::http tests::local_transport_preserves_status_headers_and_bytes_without_debug_secrets` |
| 기존 시험 | `broker-toss::http tests::query_and_account_values_are_encoded_and_sent_in_distinct_places` |
| 기존 시험 | `broker-toss::http tests::redirects_are_returned_without_following_the_location_or_leaking_it` |
| 기존 시험 | `broker-toss::http tests::token_issuance_uses_only_the_fixed_form_endpoint_and_preserves_raw_body` |
| 기존 시험 | `broker-toss::http tests::zero_port_and_invalid_secret_are_closed_errors` |
| 기존 시험 | `broker-toss::http_limits tests::body_limit_includes_exactly_eight_mib_and_rejects_the_next_byte` |
| 준비 도우미 | `broker-toss::token_management tests::fn advancing_manager` |
| 준비 도우미 | `broker-toss::token_management tests::fn manager` |
| 준비 도우미 | `broker-toss::token_management tests::fn observe_issuance_at` |
| 준비 도우미 | `broker-toss::token_management tests::fn planned_reuse_run` |
| 기존 시험 | `broker-toss::token_management tests::cancelled_explicit_recovery_keeps_duplicate_key_stop_until_recovery_succeeds` |
| 기존 시험 | `broker-toss::token_management tests::cancelled_renewal_disconnects_once_shares_failure_and_can_recover` |
| 기존 시험 | `broker-toss::token_management tests::connection_changes_use_the_same_bus_as_other_application_publishers` |
| 기존 시험 | `broker-toss::token_management tests::expiry_failures_and_recovery_publish_only_changed_state` |
| 기존 시험 | `broker-toss::token_management tests::failed_explicit_recovery_keeps_duplicate_key_stop_until_recovery_succeeds` |
| 기존 시험 | `broker-toss::token_management tests::failed_renewal_stage_stops_the_sequence_and_keeps_connection_closed` |
| 기존 시험 | `broker-toss::token_management tests::issuance_precedes_close_reconnect_subscriptions_and_order_reconciliation` |
| 기존 시험 | `broker-toss::token_management tests::late_invalid_retry_after_another_failure_reports_failure_instead_of_superseded` |
| 기존 시험 | `broker-toss::token_management tests::queued_requests_share_success_even_when_refresh_becomes_due_during_issuance` |
| 기존 시험 | `src-tauri::bin/src-tauri tests::composition_root_constructs_the_fixed_product_endpoint_without_sending_requests` |
| 기존 시험 | `xtask::bin/xtask arch_check::source::endpoint::tests::client_construction_is_private_and_requires_the_canonical_constructor` |


최종 통합 전체 검사는1619/1619, 커버리지1059/1059 및 coverage-check 위반 없음이다. 전체 명령은 승인 test-diff23건(시험19·준비도우미4)으로 종료1이며 그 밖의 검사와 후속 끄기 보고·훅 설치는 통과했다. 새 검사 끄기와 새 외부 패키지는0건이다. 기존38개 변형은 제품 코드 바이트 동일성을 확인한 이전 증거로만 유지한다. 추가4회차 리뷰는 메인 지시에 따라 하지 않았다.


병합 완료 후 `just test-diff 4a587ef3df592cf56a207739a7a59ad620e0056d`의 항목23개가 병합 비교 문맥의 목록과 정확히 일치했다. 일반 `disable-report-since`도 새 끄기0건으로 통과했다. 최신 기준의 자동 변이는 승인된 동일23건의 선행 test-diff에서 종료1이므로 실행·통과로 세지 않는다. 이 재확인 뒤에는 문서만 갱신했으며 코드·시험은 전체1619개 검사 때와 같다. 메인 master와승인된9ba977a는실제병합부모로보존했다. master이동·push·PR은없다.
