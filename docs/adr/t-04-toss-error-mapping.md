# T-04 토스 오류 매핑

- 상태: 초안

## 범위와 확정 결정

리뷰 수정 기준은 74e92be8356426e2e998ab77bcd9ffb44a60b695이며 master 2929bf6411453cfb74570a8b8b221ce54eba2b2a의 변경을 먼저 병합했다. architecture 3.1·3.2·3.8, ADR 0053·0055·0063, R-01 Q4·Q7, 로컬 OpenAPI 1.2.28과 overview 오류표를 따른다. 아래 정책은 메인이 리뷰에서 확정한 결정이다. 사용자 확인 항목으로 남기지 않는다.

순수 파싱과 매핑만 제공한다. HTTP 호출, 토큰 재발급, 시계, 재시도 실행, 주문 전송은 없다. application과 기존 master 테스트는 수정하지 않는다. 서버 메시지·본문·원문 코드 문자열을 오류 값에 담지 않는다.

| 리뷰 지적 | 확정한 처리 |
|---|---|
| 1. 문서 보완 코드 누락 | insufficient-sellable-quantity, duplicate-conditional-order, condition-already-met를 알려진 주문 전용 묶음에 추가한다. 테스트가 R-01과 overview의 HTTP 오류표를 직접 읽어 스펙과 합집합 비교한다. |
| 2. 미지 코드와 OAuth 일시 장애 | 올바른 봉투의 Unknown은 no_body(status)의 HTTP 대기 규칙을 사용한다. 원문 대신 CodeRecognition::Unknown을 남긴다. 표준 OAuth server_error·temporarily_unavailable은 알려진 대기 코드다. |
| 3. 봉투 선택 | 호출자가 map_toss_read_error와 map_toss_token_error 중 하나를 선택한다. 조회에 OAuth 봉투가 오면 Schema 불일치다. |
| 4. 스키마 불일치의 식별자 | JSON을 읽을 수 있으면 code가 없어도 유효한 requestId와 referenceId를 보존한다. |
| 5. 얇은 파서 | serde_json::Value에서 code·식별자·닫힌 커서 힌트만 읽는다. 생성 타입 역직렬화와 빈 requestId 주입을 없앤다. |
| 6. 인증·커서 인계 | invalid-token의 재발급 1회 후 요청 1회는 T-02, invalid-request의 확인 가능한 커서 힌트는 CursorHint로 보존해 T-06에 인계한다. |
| 7. 독립 분류 기대값 | R-01 A-05 열을 ErrorClass 네 분류로 읽어 모든 코드의 class를 확인한다. 사용자 조치·자료 없음의 구체 값, 대기 원인·시간, 필요한 요청 진단은 별도 회귀로 확인한다. PortErrorKind 전체 표를 기대값으로 복제하지 않는다. |
| 8. 생존 변형 | edge-blocked 400 허용, 빈 3xx Schema, 기타 4xx 실패 제외, 공백 Missing, 식별자 + 허용을 각각 회귀로 고정한다. 식별자 ASCII 전수 허용·거부 표와 길이 경계를 검사한다. |

## 코드 목록과 분류

OpenAPI의 code 문자열 예시 44개와 설명에만 있는 fractional-quantity-scale-exceeded 1개를 합쳐 45개다. ApiError.code에는 enum이 없다. R-01 HTTP 오류표의 REST 코드는 52개, overview HTTP 오류표는 49개다. 이 두 표와 스펙을 합친 알려진 REST 코드는 53개다. 스펙 밖 보완은 edge-blocked, ip-not-allowed, unsupported-content-type, edge-rate-limit-exceeded, order-limit-exceeded, insufficient-sellable-quantity, duplicate-conditional-order, condition-already-met의 8개다.

테스트는 세 문서를 직접 읽고 예시·설명·향후 code enum과 HTTP 표의 코드 열을 수집한다. 보완 코드 목록을 하드코딩해 기대 집합을 만들지 않는다. 스펙 45개와 전체 53개라는 검토 기준 개수도 확인한다. ApiErrorCode는 알려진 53개와 원문 없는 Unknown이다. OAuthErrorCode는 스펙 enum 6개와 메인이 확정한 표준 일시 장애 2개, 원문 없는 Unknown이다. 분류 match는 닫힌 열거형을 모두 소진한다.

| 조회 경계의 분류 | 개수 | 코드 |
|---|---:|---|
| 사용자 조치 | 5 | account-not-found, forbidden, ip-not-allowed, login-user-not-found, token-revoked |
| 대기 | 5 | expired-token, rate-limit-exceeded, edge-rate-limit-exceeded, internal-error, maintenance |
| 데이터 없음 | 3 | stock-not-found, exchange-rate-not-found, sector-not-found |
| 요청 버그 | 9 | invalid-request, account-header-required, invalid-token, unsupported-ranking-duration, unsupported-symbol, unsupported-market, unsupported-date, unsupported-content-type, edge-blocked |
| 조회에 예상하지 않은 주문 응답 | 31 | 아래 주문 코드 표. Validation 버그 |

전체 REST 53개의 조회 경계 분류는 사용자 조치 5, 대기 5, 데이터 없음 3, 버그 40이다. 스펙 45개만 집계하면 4·4·3·34, 문서 보완 8개는 1·1·0·6이다. 미지 코드는 고정 분류 개수에 넣지 않으며 HTTP 상태를 따른다.

### 주문 코드의 경계

ADR 0055대로 다음 주문 전용 코드가 조회 응답으로 들어오면 InvalidResponse(Validation)이다. 실제 주문 응답의 사용자 거절·상태 변화·대기를 구현한 것으로 해석하지 않는다. 특히 already-filled를 주문 실패나 자동 재전송 근거로 만들지 않는다.

| 실제 주문 경로의 의미 | 코드 | 인계 |
|---|---|---|
| 사용자 확인·거절·제약 | account-restricted, amount-order-outside-regular-hours, cancel-restricted, confirm-high-value-required, fractional-quantity-outside-regular-hours, insufficient-buying-power, insufficient-sellable-quantity, investor-exchange-not-integrated, max-order-amount-exceeded, modify-restricted, opposite-pending-order-exists, order-limit-exceeded, order-type-not-allowed, prerequisite-required, price-out-of-range, stock-restricted, duplicate-conditional-order, condition-already-met | T-16, O-01, O-09 |
| 상태 대조·대기·조회 없음 | already-canceled, already-filled, already-modified, already-processing, already-rejected, conditional-order-not-found, order-hours-closed, order-not-found, request-in-progress | T-15, T-16, O-19 |
| 요청·데이터 계약 위반 | fractional-quantity-scale-exceeded, idempotency-key-conflict, market-not-supported-for-stock, us-modify-quantity-not-supported | T-09, T-16, O-01 |

R-01 A-05 열의 분류는 테스트에서 직접 읽는다. 주문 전용 코드는 위 조회 경계 결정을 적용해 Bug를 기대한다. 표에 없는 fractional-quantity-scale-exceeded도 스펙 설명과 위 경계에 따라 검사한다. 모든 알려진 코드를 검사했는지 집합으로 확인한다. 문서의 네 분류에는 세부 진단 값이 없으므로 이 대조는 ErrorClass의 분류 변형을 비교하고, A-05 세부 조치·자료·요청 진단은 별도 독립 기대값으로 검증한다.

## 봉투와 OAuth

TossErrorBody::parse는 원문을 닫힌 코드와 검증된 식별자·커서 힌트로 바꾼다. 두 공개 매핑 함수가 조회·토큰 문맥을 강제한다. map_toss_read_error는 REST 봉투를 받으며 OAuth 봉투는 Schema 불일치로 처리하고 식별자는 유지한다. map_toss_token_error는 OAuth 봉투를 받는다. 토큰 엔드포인트의 429는 OpenAPI가 공통 REST RateLimitExceeded 응답을 참조하므로 REST 봉투도 그 상태에서만 받는다. 그 429의 미지 코드 역시 속도 제한 대기로 처리한다. 토큰 문맥의 다른 REST 봉투는 Schema 불일치다. 봉투 종류가 맞지 않으면 미지 코드 대기 규칙보다 봉투 불일치가 우선한다.

| 토큰 코드 | 허용 상태 | A-01 원인 | A-05 분류 |
|---|---|---|---|
| invalid_client | 401 | InvalidCredentials | 사용자 키 확인 |
| invalid_grant, unauthorized_client | 400 | InvalidCredentials | 사용자 키 확인 |
| access_denied | 403 | IpNotAllowed | 사용자 IP 확인 |
| invalid_request, unsupported_grant_type | 400 | MalformedRequest | 버그 |
| server_error, temporarily_unavailable | 4xx·5xx | ServerError, 주입 RetryDelay | 대기 |
| 미지 문자열 | 아래 HTTP 규칙 | 상태에 따른 원인 | 상태에 따른 분류 |

알려진 OAuth 8개는 사용자 조치 4, 대기 2, 버그 2다. 1xx·2xx·3xx나 수신 없음과 실제 오류 본문이 함께 들어오면 대기를 만들지 않고 Validation으로 처리한다. error_description·error_uri·message·data의 임의 값은 파싱 성공이나 분류를 좌우하지 않으며 보존하지 않는다.

## 상태·본문·미지 코드

| 조건 | 결과 |
|---|---|
| 올바른 봉투의 미지 코드 + 5xx | Unavailable(ServerError, 주입 RetryDelay), CodeRecognition::Unknown |
| 올바른 봉투의 미지 코드 + 429 | Unavailable(RateLimited, 주입 RetryDelay), CodeRecognition::Unknown |
| 올바른 봉투의 미지 코드 + 나머지 4xx | InvalidResponse(Schema), CodeRecognition::Unknown |
| 1xx·2xx·3xx + 오류 본문 또는 오류 매핑 호출 | InvalidResponse(Validation) |
| 수신 없음 + 본문 없음 | Network 대기 |
| 수신 없음 + 비어 있지 않은 본문 | Validation. 공백만 있는 본문도 여기에 해당 |
| 본문 없음·비 JSON·code 스키마 불일치 + 5xx | ServerError 대기 |
| 본문 없음·비 JSON·code 스키마 불일치 + 429 | RateLimited 대기 |
| 본문 없음·비 JSON·code 스키마 불일치 + 나머지 4xx | Schema 버그 |
| 알려진 읽기 코드 + 문서 상태 불일치 | Validation 버그 |
| OAuth 봉투가 조회 응답으로 들어옴 | 실패 HTTP에서는 Schema, 실패 아닌 상태에서는 Validation |
| 알려진 주문 전용 코드 + 조회의 4xx·5xx | Validation 버그 |

본문 없음은 길이가 0인 문자열이다. 공백을 trim해 Missing으로 바꾸지 않는다. edge-blocked는 401·403·404·414에서만 받으며 400은 Validation이다. HttpStatus::OtherClientError도 실패 상태다. code가 문자열이면 Known 또는 Unknown, code가 없거나 문자열이 아니면 Absent라는 CodeRecognition을 결과에 보존한다. 표지는 닫힌 열거형이며 원문 코드 문자열을 담지 않는다.

## 식별자와 커서 힌트

DiagnosticIds는 독립적인 Option<DiagnosticId> 두 개다. 없음·request만·reference만·둘 다를 보존한다. 길이는 1~128바이트, 문자 집합은 ASCII 영숫자·하이픈·밑줄·점이다. +, 공백, 제어 문자, 다른 문장 부호와 비 ASCII는 거부한다. ASCII 0~127을 명시적 허용 표와 전수 대조하며 길이 0·1·128·129와 비 ASCII도 검사한다. 잘못된 한 식별자는 버리되 반대쪽 유효 값은 유지한다.

JSON이 읽히면 REST의 error 객체 또는 OAuth의 루트에서 식별자를 먼저 추출한다. code가 없어도 유효한 식별자는 Malformed와 최종 오류에 보존한다. message가 없어도 문자열 code와 식별자를 읽을 수 있다. 생성 타입으로 본문 전체를 역직렬화하지 않으므로 빈 requestId를 주입하지 않는다. 이 모듈에서 broker-toss-schema 사용을 제거했다. 기존 T-01 schema 계약 테스트가 생성 crate를 계속 사용하므로 crate 의존 선언은 유지한다.

OpenAPI /api/v1/transactions의 400 예시 invalidCursor에는 error.data.field가 cursor다. invalid-request와 이 정확한 field 조합만 CursorHint::Invalid로 보존한다. 다른 코드, 다른 field, 복수 field 문자열, data 없음은 Unspecified다. 본문·pattern·커서 원문은 보존하지 않는다. 같은 스펙의 cursorQueryMismatch 예시는 data가 없어 Invalid로 추측하지 않고 Unspecified로 보존한다. 실제 두 예시를 파일에서 읽어 테스트한다.

## 인계

- T-02: invalid-token은 재발급 1회 후 원래 요청 1회, token-revoked도 재발급 1회 후 재요청하고 반복 시 중단한다. 매퍼는 이 절차 뒤의 최종 실패를 분류한다. 토큰 응답은 map_toss_token_error, 조회 응답은 map_toss_read_error를 호출한다. 독립 토큰 발급용 ReadEndpoint는 A-01에 없으므로 해당 진단 엔드포인트 계약은 후속 포트 작업에서 정한다.
- T-03: Retry-After와 data.retryAfterSeconds·retryAfterAt 파싱·검증, 재시도 상한과 백오프를 맡는다. 매퍼는 RetryDelay를 그대로 보존하며 Unknown을 0으로 바꾸지 않는다.
- T-06: CursorHint::Invalid는 스펙의 data.field=cursor라는 사실만 뜻한다. 그 힌트에 따라 커서를 생략하고 처음부터 조회한다. data 없는 cursorQueryMismatch는 다른 invalid-request와 구별할 기계적 근거가 없으므로 실제 응답 실측과 구분 계약이 필요하다. Unspecified를 커서 오류 아님이나 재시작 허가로 해석하지 않는다.
- T-15·T-16·O-19: 주문 코드 31개의 실제 주문 상태·거절·대기 의미를 구현한다. 조회 경계의 Validation을 주문 분류에 재사용하지 않는다.
- A-06·A-19: 식별자·CodeRecognition·CursorHint는 TossMappedError에 보존되며 port_error()에는 포함되지 않는다. 진단·동기화 경계는 필요한 사실을 추출하기 전에 어댑터 결과를 버리지 않는다.

## 의존성과 검증

새 외부 의존성, 검사 끄기, 실제 API 호출은 없다. application의 path 의존과 기존 serde_json을 사용한다. 생성 타입은 공개 표면에 없고 오류 파싱 모듈에서는 더 이상 사용하지 않는다. 프로젝트·Codex 훅 신뢰 상태는 도구로 확인하지 못했으며 실제 정상 Git 훅 실행을 검사 증거로 사용한다.

수정 전 누락 코드·식별자 유실·message/data 의존·유효한 미지 코드의 잘못된 분류를 테스트 실패로 재현했다. 최종 코드의 좁은 검사, 손 변형 전후 원본 테스트와 최종 커밋 전체 검사 결과는 구현 보고에 기록한다. 아래 손 변형 표는 컴파일된 뒤 테스트가 실패한 경우만 검출로 센다.

## 손 변형 결과

별도 임시 복사본에서 36개 모두 컴파일 후 검출했다. 생존 0, 컴파일 불가 0이며 변형 전후 원본 테스트가 통과했다. M01~M05는 리뷰에서 생존했던 다섯 변형이다. 변이 실행의 cap-lints는 도구의 표준 변형 컴파일 모드이며 제품 설정이나 검사 해제를 추가하지 않았다.

| ID | 변형 | 컴파일 | 결과 | 검출 테스트 예시 |
|---|---|---|---|---|
| M01 | edge-blocked를 400에서도 허용 | 성공 | 검출 | `tests::edge_blocked_is_not_accepted_at_400` |
| M02 | 본문 없는 3xx를 Schema로 변경 | 성공 | 검출 | `tests::redirects_whitespace_and_other_clients_keep_distinct_causes` |
| M03 | 기타 4xx를 실패 상태에서 제외 | 성공 | 검출 | `tests::redirects_whitespace_and_other_clients_keep_distinct_causes` |
| M04 | 공백 본문을 Missing으로 변경 | 성공 | 검출 | `tests::redirects_whitespace_and_other_clients_keep_distinct_causes` |
| M05 | 식별자에 + 허용 | 성공 | 검출 | `tests::identifiers_have_an_exhaustive_ascii_allow_and_reject_table` |
| M06 | 식별자 공백 여부 조건 반전 | 성공 | 검출 | `tests::diagnostic_identifier_validation_is_bounded` |
| M07 | 128바이트 식별자 거부 | 성공 | 검출 | `tests::diagnostic_identifier_validation_is_bounded` |
| M08 | 129바이트 식별자 허용 | 성공 | 검출 | `tests::diagnostic_identifier_validation_is_bounded` |
| M09 | 식별자에 모든 ASCII 허용 | 성공 | 검출 | `tests::diagnostic_identifier_validation_is_bounded` |
| M10 | 식별자의 점 거부 | 성공 | 검출 | `tests::diagnostic_identifier_validation_is_bounded` |
| M11 | 식별자의 밑줄 거부 | 성공 | 검출 | `tests::diagnostic_identifier_validation_is_bounded` |
| M12 | requestId 누락 | 성공 | 검출 | `tests::oauth_unknowns_mismatches_and_diagnostics_are_sanitized` |
| M13 | referenceId 누락 | 성공 | 검출 | `tests::diagnostic_identifier_validation_is_bounded` |
| M14 | 스키마 불일치의 식별자 폐기 | 성공 | 검출 | `tests::valid_ids_survive_a_missing_code_and_message` |
| M15 | 코드 없음 표지를 Known으로 변경 | 성공 | 검출 | `tests::absent_codes_are_distinct_from_unknown_codes` |
| M16 | REST 미지 코드 표지를 Known으로 변경 | 성공 | 검출 | `tests::unknown_read_and_oauth_codes_keep_all_other_client_failures_as_schema` |
| M17 | OAuth 미지 코드 표지를 Known으로 변경 | 성공 | 검출 | `tests::unknown_read_and_oauth_codes_keep_all_other_client_failures_as_schema` |
| M18 | 조회 응답의 OAuth 봉투 허용 | 성공 | 검출 | `tests::callers_choose_read_or_token_envelopes` |
| M19 | 커서 힌트 필드를 from으로 변경 | 성공 | 검출 | `tests::cursor_hint_is_closed_and_only_uses_the_documented_field` |
| M20 | 다른 오류 코드에도 커서 힌트 부여 | 성공 | 검출 | `tests::cursor_hint_is_closed_and_only_uses_the_documented_field` |
| M21 | 표준 OAuth 일시 장애를 버그로 변경 | 성공 | 검출 | `tests::standard_oauth_transients_wait_and_known_codes_are_marked` |
| M22 | REST 미지 코드를 항상 Schema로 변경 | 성공 | 검출 | `tests::status_mismatches_and_unknown_codes_follow_http` |
| M23 | OAuth 미지 코드를 항상 Schema로 변경 | 성공 | 검출 | `tests::oauth_unknowns_mismatches_and_diagnostics_are_sanitized` |
| M24 | 토큰 REST 429 봉투 조건 반전 | 성공 | 검출 | `tests::callers_choose_read_or_token_envelopes` |
| M25 | 만료 토큰의 대기 원인 변경 | 성공 | 검출 | `tests::transient_causes_keep_the_injected_retry_delay` |
| M26 | 속도 제한의 대기 원인 변경 | 성공 | 검출 | `tests::transient_causes_keep_the_injected_retry_delay` |
| M27 | 종목 없음의 대상 변경 | 성공 | 검출 | `tests::user_actions_and_missing_data_keep_the_a05_payload` |
| M28 | 주입한 대기 시간 폐기 | 성공 | 검출 | `tests::transient_causes_keep_the_injected_retry_delay` |
| M29 | 조회 엔드포인트를 Accounts로 변경 | 성공 | 검출 | `tests::transient_causes_keep_the_injected_retry_delay` |
| M30 | HTTP 상태를 BadRequest로 변경 | 성공 | 검출 | `tests::transient_causes_keep_the_injected_retry_delay` |
| M31 | 본문 없는 5xx의 원인을 Network로 변경 | 성공 | 검출 | `tests::missing_or_invalid_bodies_follow_status_fallbacks` |
| M32 | 문서 보완 조건주문 코드 이름 변조 | 성공 | 검출 | `tests::write_only_codes_are_unexpected_responses_on_the_read_port` |
| M33 | 표준 OAuth 일시 장애 코드 이름 변조 | 성공 | 검출 | `tests::inventories_equal_the_spec_examples_descriptions_and_oauth_enum` |
| M34 | 계좌 헤더 누락의 진단 원인 변경 | 성공 | 검출 | `tests::request_bug_diagnostics_keep_the_documented_distinctions` |
| M35 | OAuth 잘못된 키를 권한 문제로 변경 | 성공 | 검출 | `tests::oauth_has_independent_credentials_ip_and_request_expectations` |
| M36 | 상태 일치 연산자 반전 | 성공 | 검출 | `tests::oauth_unknowns_mismatches_and_diagnostics_are_sanitized` |
