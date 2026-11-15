# T-04 토스 읽기 오류 매핑

- 상태: 초안

## 범위와 근거

시작 병합 HEAD는 e0536387fe36cf8e74451008ec7c6bcb253653bc이며 master cb38274a4152d4171928b36c65e3f3a8407a65d7을 포함한다. architecture 3.1·3.2·3.8, ADR 0053·0055·0063, R-01 Q4·Q7과 로컬 OpenAPI 1.2.28을 따른다. 순수 파싱과 매핑만 제공한다. HTTP 호출, 토큰 재발급, 시계, 재시도 실행, 주문 전송은 없다.

공개 입력 TossErrorBody는 원문을 즉시 닫힌 코드와 검증된 진단 식별자로 바꾼 값이다. ErrorResponse와 OAuth2ErrorResponse는 비공개 파서 안에서만 사용한다. 공개 시그니처와 필드에 생성 타입이 없다. map_toss_error는 대기 중인 조회의 ReadEndpoint, A-01 HttpStatus, TossErrorBody, T-03이 정규화한 RetryDelay를 받아 TossMappedError를 반환한다. port_error()로 A-01 PortError를 얻으며 ClassifiedError는 A-05에 위임한다. 진단 식별자는 어댑터 결과에 함께 보존한다. application의 닫힌 필드를 바꾸지 않는다.

## 코드 목록과 조회 분류

OpenAPI의 code 문자열 예시에서 44개, 백틱으로 둘러싼 설명에서 fractional-quantity-scale-exceeded 1개를 추가해 45개다. ApiError.code에는 enum이 없다. R-01 Q7의 스펙 밖 보완 5개(edge-blocked, ip-not-allowed, unsupported-content-type, edge-rate-limit-exceeded, order-limit-exceeded)를 더해 ApiErrorCode는 알려진 코드 50개와 원문을 담지 않는 Unknown이다. 테스트는 스펙 파일에서 code 예시·설명·향후 code enum을 수집해 명시적 알려진 목록과 집합 비교한다. OAuth의 enum도 별도 비교한다. 분류 match에는 와일드카드가 없다.

| 조회 분류 | 수 | 코드 | A-01 원인 |
|---|---:|---|---|
| 사용자 조치 | 5 | account-not-found, forbidden, ip-not-allowed, login-user-not-found, token-revoked | 각각 AccountNotFound, Forbidden, IpNotAllowed, LoginUserNotFound, TokenRevokedRepeated |
| 대기 | 5 | expired-token, rate-limit-exceeded, edge-rate-limit-exceeded, internal-error, maintenance | 각각 ExpiredToken, RateLimited, RateLimited, ServerError, Maintenance. RetryDelay 원값 보존 |
| 데이터 없음 | 3 | stock-not-found, exchange-rate-not-found, sector-not-found | Stock, ExchangeRate, Sector |
| 요청 버그 | 9 | invalid-request, account-header-required, invalid-token, unsupported-ranking-duration, unsupported-symbol, unsupported-market, unsupported-date, unsupported-content-type, edge-blocked | MalformedRequest, MissingAccountHeader, InvalidToken, UnsupportedQuery, UnsupportedContentType, EdgeBlocked |
| 조회에 예상하지 않은 주문 응답 | 28 | 아래 표 | InvalidResponse(Validation) |

스펙 45개만 집계하면 사용자 조치 4, 대기 4, 데이터 없음 3, 버그 34다. R-01 보완 5개는 사용자 조치 1, 대기 1, 버그 3이다. 전체 50개는 사용자 조치 5, 대기 5, 데이터 없음 3, 버그 37이다. Unknown은 별도 Schema 버그다.

### 주문 코드의 경계

ADR 0055는 주문 생성·정정·취소 전용 오류를 조회 PortError로 억지 변환하지 않고 주문 작업에 인계하도록 정했다. 따라서 다음은 **조회 응답으로 들어왔을 때** Validation 버그로 닫는다. 이 분류를 실제 주문 응답에 적용하지 않는다. 예를 들어 already-filled를 주문 실패나 자동 재전송 근거로 만들지 않는다. 전체 코드가 조회 경계에서 안전하게 처리된다는 뜻이며, 주문 코드의 제품 의미를 구현했다는 뜻은 아니다.

| 실제 주문 경로의 R-01 의미 | 코드 | 인계 |
|---|---|---|
| 사용자 확인·거절·제약 | account-restricted, amount-order-outside-regular-hours, cancel-restricted, confirm-high-value-required, fractional-quantity-outside-regular-hours, insufficient-buying-power, investor-exchange-not-integrated, max-order-amount-exceeded, modify-restricted, opposite-pending-order-exists, order-limit-exceeded, order-type-not-allowed, prerequisite-required, price-out-of-range, stock-restricted | T-16, O-01, O-09 |
| 상태 대조·대기·조회 없음 | already-canceled, already-filled, already-modified, already-processing, already-rejected, conditional-order-not-found, order-hours-closed, order-not-found, request-in-progress | T-15, T-16, O-19 |
| 요청·데이터 계약 위반 | fractional-quantity-scale-exceeded, idempotency-key-conflict, market-not-supported-for-stock, us-modify-quantity-not-supported | T-09, T-16, O-01 |

## OAuth

| 코드 | 허용 상태 | A-01 원인 | A-05 분류 |
|---|---|---|---|
| invalid_client | 401 | InvalidCredentials | 사용자 키 확인 |
| invalid_grant | 400 | InvalidCredentials | 사용자 키 확인. 미해결 참조 |
| unauthorized_client | 400 | InvalidCredentials | 사용자 키 확인. 미해결 참조 |
| access_denied | 403 | IpNotAllowed | 사용자 IP 확인 |
| invalid_request | 400 | MalformedRequest | 버그 |
| unsupported_grant_type | 400 | MalformedRequest | 버그 |
| 미지 문자열 | 4xx·5xx | Schema | 버그. 원문 버림 |

OAuth enum에 일시 장애 코드가 없다. server_error와 temporarily_unavailable도 현재 스펙에서는 Unknown으로 처리해 자동 대기를 허가하지 않는다. 본문이 없거나 파싱되지 않는 5xx는 아래 HTTP 대기 규칙을 따른다. error_description과 error_uri는 보존하지 않는다.

## 상태·본문의 닫힌 규칙

| 조건 | 원인 |
|---|---|
| 수신 상태 없음 + 본문 없음 | Network, 주입 RetryDelay |
| 1xx·2xx·3xx에서 오류 매핑 호출 | Validation |
| 수신 상태 없음 + 비어 있지 않은 본문 | Validation |
| 유효한 알려진 읽기 코드 + 문서 상태 일치 | 위 코드 표 |
| 알려진 코드 + 문서 상태 불일치 | Validation. 상태보다 코드만 믿고 대기하지 않음 |
| 유효한 미지 코드 + 4xx·5xx | Schema. 5xx여도 미지 코드를 대기로 덮지 않음 |
| 본문 없음·비 JSON·스키마 불일치 + 5xx | ServerError, 주입 RetryDelay |
| 본문 없음·비 JSON·스키마 불일치 + 429 | RateLimited, 주입 RetryDelay |
| 본문 없음·비 JSON·스키마 불일치 + 나머지 4xx | Schema |
| 알려진 주문 전용 코드 + 4xx·5xx | 조회 계약의 Validation |

HttpStatus는 A-01의 닫힌 상태 범주다. internal-error·maintenance는 ServerError 범주, edge-blocked는 401·403·404·414를 받는다. 나머지 읽기 코드는 R-01의 상태와 정확히 대응한다. token-revoked는 T-02가 재발급 1회를 처리한 뒤 최종 실패를 넘기는 경계다. 순수 매퍼 자체에는 재발급 전후 상태가 없다.

## 진단·본문·대기

DiagnosticIds는 request와 reference의 독립적인 Option<DiagnosticId> 두 개다. 없음·request만·reference만·둘 다를 보존한다. DiagnosticId는 1~128바이트 ASCII 영숫자와 하이픈·밑줄·점만 허용한다. 범위를 벗어나거나 문자열이 아닌 식별자는 버리고 반대쪽 유효 값은 유지한다. 길이와 문자 집합은 R-01의 두 예시를 받을 수 있는 보수적 상한이며 사용자 확인 대상이다. 임의 서버 문장을 식별자로 대입하는 경로가 없다.

생성 ApiError의 requestId 필수 제약과 실제 앞단의 referenceId 전용 응답 차이를 어댑터에서 흡수한다. 두 원본 식별자를 먼저 검사한 뒤 생성 타입 디코드용 requestId만 빈 문자열로 정규화한다. 이 임시 빈 값은 진단 식별자로 노출하지 않는다. code·message·data의 생성 타입 검증은 유지한다.

TossErrorBody와 TossMappedError에는 서버 message, data, OAuth 상세, 원문 코드, JSON 본문, 헤더, 자격 증명이 없다. Debug는 검증된 식별자와 닫힌 필드만 표시한다. Display는 고정 진단 표식만 표시한다. UI 문장은 만들지 않는다. RetryDelay::Unknown은 0으로 바꾸지 않으며 Known의 0·나노초도 그대로 보존한다. Retry-After와 data.retryAfterSeconds·retryAfterAt 파싱, 백오프와 재시도 상한은 ADR 0063대로 T-03에 인계한다.

## 의존성과 검증

메인 승인에 따라 broker-toss → application(path, 0.1.0, 기능 없음)을 추가했다. 전이 작업 공간 의존은 기존 domain이며 외부 패키지 추가는 없다. 내부 crate의 라이선스 필드는 선언되어 있지 않다. serde_json 1.0.154는 기존 개발 의존을 일반 의존으로 옮겼다. 생성 crate를 통해 이미 제품 그래프에 있었고 버전·기능·전이 패키지는 늘지 않는다. arch-check는 기존 허용 간선을 사용하며 규칙·허용 목록 수정은 없다.

테스트는 구현 전 추가했고, 존재하지 않는 공개 매핑 API의 컴파일 실패를 확인한 뒤 구현했다. 문서 기반 원인 표, OAuth 분류 표, 스펙 집합 대조, 식별자 네 경우와 경계, Debug·Display의 원문 부재, 상태 불일치와 미지 코드, 대기 원값을 검증한다. 손 변형은 별도 임시 복사본에서 25개를 실행해 모두 컴파일 후 테스트 실패로 검출했다. 생존 0, 컴파일 불가 0이며 변형 전후 원본 테스트가 통과했다. 대상은 식별자 길이·문자 집합, 두 식별자 보존, envelope 구분, OAuth와 읽기 원인, HTTP 상태 비교, 대기 보존, 엔드포인트·상태 전달이다. 자동 설정의 대상은 domain·order-execution·application이며 broker-toss는 대상 밖이므로 새 코드의 손 변형을 사용했다. 최종 전체 검사 결과는 구현 보고에 기록한다. 실제 API·실제 키·실제 계좌 자료는 사용하지 않는다.

## 인계와 사용자 확인 대상

- T-02: token-revoked는 재발급 1회 이후에 매퍼로 최종 실패를 전달한다. OAuth는 대기 중인 조회의 ReadEndpoint로 귀속한다. A-01에 Token 엔드포인트가 없어 독립 토큰 발급 진단 엔드포인트는 후속 포트 계약으로 정한다.
- T-03: Retry-After, 본문 대기 힌트의 시간 파싱·검증과 재시도 상한을 구현하고 RetryDelay로 주입한다. 매퍼는 머리말이나 힌트 원문을 보존하지 않는다.
- T-15·T-16·O-19: 위 주문 코드 28개의 실제 주문 분류·상태 대조를 구현한다. 조회 매퍼의 Validation을 주문 분류로 재사용하지 않는다.
- A-06·A-19: application PortError에는 식별자 필드가 없으므로, 진단 기록에 필요하면 TossMappedError의 DiagnosticIds를 잃기 전에 어댑터 진단 경계에서 보존할 계약이 필요하다. application은 수정하지 않았다.
- 사용자 확인: 미지 코드를 Schema 버그로 처리해 대기를 만들지 않는 정책, 식별자 128바이트·ASCII 문자 집합 상한을 확인한다.
- 사용자 확인: R-01·ADR 0063에 개별 행이 없는 invalid_grant와 unauthorized_client는 대기를 만들지 않고 키 확인 조치로 정했다. 실제 client credentials 발급에서의 발생 의미는 실측하지 않았다.
- 프로젝트 신뢰와 Codex 훅 신뢰 상태는 제공된 도구로 확인하지 못했다. 실제 Git 훅 실행과 check 출력을 검증 증거로 사용한다.
