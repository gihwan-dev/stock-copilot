# A-05: 포트 오류의 세 분류

- 상태: 초안

## 결정과 근거

최초 작업 기준은 6a33bc1b1ec4ef9dd055b60f10c0f93aba350a20이며, 후속 검증 기준으로 로컬 master 9b7c74285cca8ac83d914e609962695aad58ad8b를 병합했다. architecture 3.3·3.8, design 2.4·2.6·API-1~3, R-01 Q4·Q7과 ADR 0055의 닫힌 조회 원인을 따른다. 분류는 포트가 반환한 최종 실패에 적용한다. 사용자 문장, 화면 표시 우선순위, 대기 실행, 자동 재발급, 주문 전송은 하지 않는다.

ErrorClass는 UserAction(UserAction), Wait(RetryDelay), Bug(BugDiagnostic)의 닫힌 합 타입이다. classify_port_error는 PortErrorKind와 모든 하위 원인을 와일드카드 없이 소진한다. 오류 종류가 늘면 컴파일이 실패한다. HTTP 상태가 아니라 정규화된 원인이 분류의 기준이다. 상태와 원인의 올바른 결합은 T-04 계약 테스트 책임이다.

## 변형별 분류표

| PortErrorKind와 하위 원인 | ErrorClass | 딸린 닫힌 값 |
|---|---|---|
| AccessDenied(InvalidCredentials) | UserAction | CheckCredentials |
| AccessDenied(IpNotAllowed) | UserAction | CheckIpAllowlist |
| AccessDenied(TokenRevokedRepeated) | UserAction | StopCompetingKeyUse |
| AccessDenied(AccountNotFound) | UserAction | CheckAccount |
| AccessDenied(Forbidden) | UserAction | CheckPermissions |
| AccessDenied(LoginUserNotFound) | UserAction | Reauthenticate |
| Unavailable(RateLimited, retry) | Wait | retry 원값 |
| Unavailable(Maintenance, retry) | Wait | retry 원값 |
| Unavailable(ServerError, retry) | Wait | retry 원값 |
| Unavailable(Network, retry) | Wait | retry 원값 |
| Unavailable(ExpiredToken, retry) | UserAction | Reauthenticate |
| NotFound(Stock) | UserAction | CheckData(Stock) |
| NotFound(ExchangeRate) | UserAction | CheckData(ExchangeRate) |
| NotFound(Sector) | UserAction | CheckData(Sector) |
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

위 표의 27개 원인에 각각 기대값을 둔 every_leaf_matches_the_decision_table이 있다. 대기 원인은 Known과 Unknown을 각각 검증한다. endpoint_and_status_do_not_override_the_cause는 현재 모든 엔드포인트·상태와 표의 사례를 교차한다. 테스트는 ADR 파일 이름이나 본문을 읽지 않는다. 표를 바꿀 때 독립 기대값 테스트도 함께 검토한다.

## 대기·인증·빈 상태의 경계

RetryDelay는 Unknown 또는 Known(domain::TimeSpan)이다. 값이 있으면 0초와 소수초도 그대로 보존한다. 없으면 0초·기본 백오프를 발명하지 않는다. 현재 포트는 절대 재시도 시각을 받지 않으므로 시계를 읽거나 시간을 계산하지 않는다. T-03이 Retry-After를 경계에서 기간으로 검증하고, API-1의 응답 한도·60% 제한과 API-2의 지수 백오프를 수행한다. 분류가 재시도나 주문 재전송의 허가는 아니다.

토큰 폐기는 T-02가 재발급을 1회 시도한 뒤 반복 실패를 TokenRevokedRepeated로 전달한다. 만료도 자동 재발급 1회 뒤 여전히 실패한 최종 ExpiredToken을 Reauthenticate로 처리한다. ADR 0055는 Unavailable(ExpiredToken)을 대기로 분류한 초안이지만, 이번 사용자 지시에 따라 이 ADR에서는 재발급 뒤 최종 실패를 사용자 조치로 분류한다. 기존 ADR 0055의 보호된 본문은 수정하지 않는다. InvalidToken은 ADR 0055의 요청 계약 버그 분류를 유지한다.

T-02·T-04 필수 인계: 최초 만료(자동 재발급 대상)와 재발급 1회 뒤 최종 실패는 서로 다른 PortError 원인으로 전달해야 한다. 현재 UnavailableCause::ExpiredToken은 후자의 최종 실패 전용이다. 최초 만료에 같은 원인을 재사용하면 갱신 전에 재인증을 요구하게 되므로 금지한다. 최초 만료를 포트 밖으로 전달하는 계약에는 대기로 분류할 별도의 닫힌 원인을 후속 작업에서 추가하고 A-05의 exhaustive match와 변형별 테스트도 함께 확장해야 한다. 자동 재발급이 진행되는 동안 T-02 내부에서 처리하는 상태와 포트가 반환하는 최종 실패를 혼동하지 않는다.

자료 없음은 ADR 0055대로 사용자 조치 범주의 CheckData이며 계좌 접근 실패와 구별한다. 성공한 빈 Vec는 오류가 아니며 분류하지 않는다. CheckData는 누락을 숨기지 않는 구조화 값이고 사용자가 자료를 복구할 수 있다는 보장이 아니다. A-06·A-08과 U-09가 부분 실패·빈 상태를 구분하고 A-19가 P-7의 상태 띠 정책을 적용한다. 지원 없음의 정상 판정은 CapabilitySupport이고, 그 판정을 무시하고 포트를 호출한 Unsupported는 버그다. 스키마·검증 실패는 원문 없는 진단 종류다.

## 비밀과 본문이 없는 오류

기존 PortError의 필드는 ReadEndpoint, HttpStatus, PortErrorKind뿐이다. 모든 원인 필드를 따라가면 닫힌 열거형과 RetryDelay의 TimeSpan만 있다. 문자열, 헤더, 토큰, 계좌 식별자, 요청·응답 본문, 외부 오류 객체를 담는 필드가 없다. 새 ErrorClass·UserAction·BugDiagnostic도 동일한 닫힌 값만 담는다. 사용자 문장을 만들지 않으며 Display는 UserAction / Wait / Bug의 진단용 분류 이름만 출력한다. 세부 진단은 닫힌 Debug 값으로 확인한다.

formatting_contains_only_the_closed_fields는 모든 예제의 PortError Debug 구조와 분류 값 Display를 확인하고 가짜 비밀·Bearer·계좌번호·본문·헤더 표식의 부재를 확인한다. PortError에는 원래 Display 구현이 없으며 추가하지 않는다. 비밀을 받아 지우는 방식이 아니라 입력 필드 자체가 없다. 격리 거래 원문은 성공/불완전 조회 자료이고 오류가 아니므로 기존 observations 계약을 수정하지 않는다. 실제 키·계좌·API는 사용하지 않는다.

## 새 포트 인계와 한계

현재 application의 BrokerRead·MarketInfo와 종목별 QuoteObservation 오류는 모두 PortError다. 별도 application 포트 오류 타입은 없다. 새 FalliblePort는 type Error: Into<PortError>를 요구한다. A-02·A-03의 실패 가능한 포트는 이 supertrait를 채택하고 Result의 오류를 Self::Error로 선언한다. 어댑터 전용 오류가 필요하면 닫힌 PortError로 변환하는 구현 없이는 이 경계를 구현할 수 없다. 소비자는 변환 후 같은 classify_port_error를 사용한다. 테스트의 FuturePort가 이 경계를 사용한다.

Rust는 앞으로 선언할 임의의 trait에 supertrait 채택을 강제로 삽입할 수 없다. 기존 lib.rs의 trait 본문 변경 금지 범위 안에서 제공한 장치이며, A-02·A-03이 경계를 무시한 별도 오류 포트를 선언하는 것까지 이 작업만으로 금지하지는 못한다. 후속 포트 리뷰에서 채택을 확인해야 한다. 전역 AST 검사 추가는 보호된 하네스 작업으로 인계한다.

D-36 CashObservationFailure와의 대응은 A-06이 담당하며 여기서는 domain을 수정하지 않는다.

## 미해결

- Forbidden의 실제 필요한 권한은 R-01 Q2에도 미확인이다. CheckPermissions로 보존하며 특정 권한 설정 절차를 발명하지 않는다.
- CheckData의 대상별 실제 사용자 행동과 화면 문장은 A-08·U 그룹에서 정한다. 성공한 빈 목록이나 누락 자료를 계좌 연결 오류로 승격하지 않는다.
- T-02·T-04는 최초 만료와 재발급 뒤 최종 실패에 서로 다른 PortError 원인을 사용해야 한다. 현재 ExpiredToken은 최종 실패 전용이며, 최초 만료 전달용 닫힌 원인 추가와 대기 분류 확장은 후속 계약 작업이다.
- A-02·A-03의 FalliblePort 채택과 하네스 차원의 전역 강제는 후속 작업에서 확인한다.
