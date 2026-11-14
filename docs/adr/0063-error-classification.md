# ADR 0063: A-05 오류 분류

- 상태: 확정 (2026-11-14)

## 결정과 근거

최초 작업 기준은 6a33bc1b1ec4ef9dd055b60f10c0f93aba350a20이며, 후속 검증 기준으로 로컬 master 9b7c74285cca8ac83d914e609962695aad58ad8b를 병합했다. architecture 3.3·3.8, design 2.4·2.6·API-1~3, R-01 Q4·Q7과 ADR 0055의 닫힌 조회 원인을 따른다. 분류는 포트가 반환한 최종 실패에 적용한다. 사용자 문장, 화면 표시 우선순위, 대기 실행, 자동 재발급, 주문 전송은 하지 않는다.

ErrorClass는 UserAction(UserAction), Wait(RetryDelay), Bug(BugDiagnostic), NoData(MissingData)의 닫힌 합 타입이다. architecture 3.8의 세 오류 분류와 R-01 Q7의 별도 데이터 없음을 함께 반환한다. classify_port_error는 PortErrorKind와 모든 하위 오류 원인을 와일드카드 없이 소진한다. 오류 원인이 늘면 컴파일이 실패한다. BrokerCapability는 모두 같은 호출 계약 위반 분류이므로 값을 보존하는 한 갈래로 받으며 capability 추가만으로 분류 파일을 바꿀 필요는 없다. HTTP 상태가 아니라 정규화된 원인이 분류의 기준이다. 상태와 원인의 올바른 결합은 T-04 계약 테스트 책임이다.

## 변형별 분류표

| PortErrorKind와 하위 원인 | ErrorClass | 딸린 닫힌 값 |
|---|---|---|
| AccessDenied(InvalidCredentials) | UserAction | CheckCredentials |
| AccessDenied(IpNotAllowed) | UserAction | CheckIpAllowlist |
| AccessDenied(TokenRevokedRepeated) | UserAction | StopCompetingKeyUse |
| AccessDenied(AccountNotFound) | UserAction | CheckAccount |
| AccessDenied(Forbidden) | UserAction | CheckPermissions |
| AccessDenied(LoginUserNotFound) | UserAction | CheckCredentials |
| Unavailable(RateLimited, retry) | Wait | retry 원값 |
| Unavailable(Maintenance, retry) | Wait | retry 원값 |
| Unavailable(ServerError, retry) | Wait | retry 원값 |
| Unavailable(Network, retry) | Wait | retry 원값 |
| Unavailable(ExpiredToken, retry) | Wait | retry 원값 |
| NotFound(Stock) | NoData | Stock |
| NotFound(ExchangeRate) | NoData | ExchangeRate |
| NotFound(Sector) | NoData | Sector |
| InvalidResponse(Schema) | Bug | Response(Schema) |
| InvalidResponse(Validation) | Bug | Response(Validation) |
| Unsupported(DailyCandles) | Bug | UnsupportedCapability(DailyCandles) |
| Unsupported(MinuteCandles) | Bug | UnsupportedCapability(MinuteCandles) |
| Unsupported(AdjustedHistory) | Bug | UnsupportedCapability(AdjustedHistory) |
| Unsupported(RawHistory) | Bug | UnsupportedCapability(RawHistory) |
| InvalidRequest(MalformedRequest) | Bug | Request(MalformedRequest) |
| InvalidRequest(MissingAccountHeader) | Bug | Request(MissingAccountHeader) |
| InvalidRequest(InvalidToken) | Bug | Request(InvalidToken) |
| InvalidRequest(EdgeBlocked) | Bug | Request(EdgeBlocked) |
| InvalidRequest(UnsupportedQuery) | Bug | Request(UnsupportedQuery) |
| InvalidRequest(UnsupportedContentType) | Bug | Request(UnsupportedContentType) |
| InvalidRequest(BatchLimitExceeded) | Bug | Request(BatchLimitExceeded) |

문서 표에서 독립적으로 작성한 expected_class와 원인별 기대 함수는 와일드카드 없는 match로 PortErrorKind, AccessDenial, UnavailableCause, MissingData, ResponseMismatch, RequestRejection의 모든 잎을 소진한다. 새 오류 원인을 더하면 구현을 보완했더라도 기대 함수가 컴파일되지 않아 테스트 기대값 검토를 강제한다. capability는 원인이 아니라 보존할 진단 값이므로 그대로 받는다. 기존 표 기반 예제와 기대 함수가 일치하는지도 every_leaf_matches_the_decision_table에서 확인한다. 알려진/알 수 없는 대기, 모든 엔드포인트·상태와의 교차, trait 호출을 함께 검사한다. 테스트는 ADR 파일 이름이나 본문을 읽지 않는다.

## 대기·인증·빈 상태의 경계

RetryDelay는 Unknown 또는 Known(domain::TimeSpan)이다. 값이 있으면 0초와 소수초도 그대로 보존한다. 없으면 0초·기본 백오프를 발명하지 않는다. 현재 포트는 절대 재시도 시각을 받지 않으므로 시계를 읽거나 시간을 계산하지 않는다. 분류는 재시도의 허가가 아니며 주문 재전송의 허가도 아니다.

R-01 Q7과 ADR 0055대로 ExpiredToken은 대기다. T-02의 자동 재발급 뒤에도 남는 만료는 대기로 분류한다. 이 앱은 client credentials로 토큰을 발급하므로 사용자 재인증 조치를 만들지 않는다. 반복 token-revoked는 T-02가 재발급 1회 뒤 TokenRevokedRepeated로 넘기며 같은 키를 쓰는 다른 도구 확인 조치를 유지한다. InvalidToken은 요청 계약 버그다. LoginUserNotFound는 키가 가리키는 사용자를 찾을 수 없다는 사용자 조치이며 CheckCredentials로 전달한다.

R-01 Q7은 stock-not-found·exchange-rate-not-found·sector-not-found를 데이터 없음으로 따로 분류한다. architecture 3.8이 그 표를 따르도록 하므로 NotFound는 NoData이며 사용자 조치가 아니다. design 2.6의 사용자가 할 일이 있는 문제만 상태 띠에 올린다는 원칙에 따라 데이터 없음은 상태 띠에 올리지 않고 해당 조회의 빈 결과로 다룬다. A-08·A-19가 화면 처리를 맡는다. 성공한 빈 Vec 역시 오류로 승격하지 않는다. ADR 0055의 NotFound 사용자 조치 인계보다 R-01 Q7 및 이번 architecture 3.8의 명시적 보완을 따른다. 지원 없음의 정상 판정은 CapabilitySupport이고 지원 밖 호출의 Unsupported는 버그다.

재시도 횟수·상한 인계: 429의 Retry-After, API-1의 응답 한도·60% 제한, API-2의 백오프 및 재시도 상한은 T-03 책임이다. 포트 호출 밖에서 다시 시도하는 동기화·예약 유스케이스는 A-06과 스케줄러 작업이 각각 횟수·상한을 정한다. 분류 함수는 재시도 루프나 무한 반복을 만들지 않는다.

## 비밀과 본문이 없는 오류

기존 PortError의 필드는 ReadEndpoint, HttpStatus, PortErrorKind뿐이다. 모든 원인 필드를 따라가면 닫힌 열거형과 RetryDelay의 TimeSpan만 있다. 새 ErrorClass·UserAction·BugDiagnostic도 닫힌 값만 담는다. 문자열, 헤더, 토큰, 계좌 식별자, 요청·응답 본문, 외부 오류 객체를 담는 필드가 없다. PortError와 ErrorClass 모두 Display를 구현하지 않고 사용자 문장을 만들지 않는다.

port_and_classification_types_are_copy는 PortError·분류 타입과 중첩 필드 타입들의 Copy를 명시적으로 요구한다. classification_fields_are_closed_enums_or_time_span은 모든 분류·조치·진단·대기 필드를 와일드카드 없는 패턴으로 열고, 각 필드가 지정한 닫힌 열거형 또는 TimeSpan 타입임을 컴파일로 고정한다. 문서 기반 기대 함수도 원본 PortErrorKind의 필드를 빠짐없이 연다. PortError의 비공개 필드는 생성자·접근자 반환 타입과 정확한 Debug 필드 구조로 고정한다. 이 검사는 구조의 경계이며 비밀 표식을 입력할 수 없는 타입에 임의 문자열 부재를 반복 단언하지 않는다. 원시 타입 금지와 실제 필드 검토도 함께 적용한다.

격리 거래 원문은 성공/불완전 조회 자료이고 오류가 아니므로 기존 observations 계약을 수정하지 않는다. 실제 키·계좌·API는 사용하지 않는다.

## 새 포트 인계와 하네스 후속

ClassifiedError는 fn class(&self) -> ErrorClass 하나만 요구한다. PortError가 이를 구현하며 자유 함수 classify_port_error와 같은 결과를 돌려준다. A-02·A-03은 저장소·비밀 저장·AI·뉴스 등 각자의 닫힌 오류 타입을 정의하고 이 trait을 구현한다. 토스 전용 PortError로 변환하거나 브로커 조치 종류를 다른 포트에 억지로 사용하지 않는다. 현재 키·IP·중복 키·계좌·권한 조치는 PortError 분류에서만 사용하며, 다른 포트에 필요한 조치나 진단은 그 작업에서 닫힌 변형을 추가한다. 테스트의 OtherPortError는 PortError 변환 없이 trait만 구현한다.

하네스 후속 arch-check: application의 공개 포트 trait 메서드가 반환하는 오류 타입은 ClassifiedError를 구현해야 한다는 규칙을 검사한다. trait을 정의한 것만으로 앞으로 선언할 임의 포트의 채택을 강제했다고 주장하지 않는다.

하네스 후속 변이 범위: 현재 harness 6.4와 보호된 .cargo/mutants.toml은 application의 order_gate만 대상으로 하므로 error_class.rs를 포함하도록 후속 작업에서 범위를 확장한다. 이번 작업은 보호 설정을 수정하지 않고 고친 분기를 겨냥한 손 변형 16개 이상으로 검증한다.

D-36 CashObservationFailure와의 대응은 A-06이 담당하며 여기서는 domain을 수정하지 않는다.

## 미해결

- LoginUserNotFound는 R-01 Q7의 사용자 조치와 CheckCredentials로 대응하지만 design 2.6 점검표에는 대응 행이 없다. A-19·UI 작업에서 키 확인 안내의 적용 행을 정한다.
- Forbidden의 실제 필요한 권한은 R-01 Q2에도 미확인이다. CheckPermissions로 보존하며 특정 권한 설정 절차를 발명하지 않는다.
- A-08·A-19는 NoData를 상태 띠에 올리지 않고 해당 조회의 빈 결과로 처리한다.
- A-02·A-03의 개별 닫힌 오류 타입, 필요한 조치·진단 추가와 ClassifiedError 채택 검사는 후속 작업이다.
- arch-check의 오류 trait 규칙과 error_class.rs의 자동 변이 범위 등록은 보호된 하네스 후속이다.
- tasks.md A-05 완료 기준의 "모든 포트 오류가 셋 중 하나로 분류"는 "모든 포트 오류가 사용자 조치·대기·버그 또는 R-01 Q7의 데이터 없음으로 분류되며, 데이터 없음은 상태 띠에 올리지 않고 해당 조회의 빈 결과로 처리"로 메인이 정정해야 한다. 이번 작업은 tasks.md를 수정하지 않는다.
