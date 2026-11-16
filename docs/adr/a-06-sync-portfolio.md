# A-06 포트폴리오 동기화

- 상태: 초안

## 결정

ADR 0065의 이벤트 단위는 계좌다. SyncPortfolio는 명시적인 선택 계좌 하나를 받아 accounts의 종합매매 계좌 여부를 확인하고 holdings와 원화·달러 매수가능금액을 각각 한 번 조회한다. 여러 계좌를 처리하는 호출자는 계좌마다 실행하므로 한 계좌의 실패가 다른 계좌 기록을 지우지 않는다. 계좌 선택 자체는 기존 domain 선택 함수에 위임한다. 재시도는 포트 어댑터 책임이며 유스케이스는 반복 재시도하지 않는다.

PortfolioRepository와 SyncRecord/SyncOutcome이 이미 있으므로 새 저장 포트를 만들지 않는다. domain의 portfolio_from_cash_observations와 with_reported만 사용한다. 보유·현금 금액·평가·손익을 application에서 계산하지 않는다. 보유 조회 성공과 일부 현금 조회 실패는 Partial이며 받은 보유와 보고값, 성공 통화, 실패 통화의 닫힌 원인을 보존한다. 보유 조회 실패는 Failed, 접근 거부는 AccessBlocked가 우선한다. 잘못된 계좌 선택은 AccessBlocked다. 조립 검증 실패는 Failed, BuyingPower 자체 오류는 Partial이다. 실패를 0원·빈 성공으로 바꾸지 않는다.

기록 시각은 마지막 Clock 관측값이다. 현금 실패의 관측 시각도 주입한 Clock에서 받는다. 저장 성공 이후 PortfolioSyncRecorded(account)를 정확히 한 번 publish한다. 저장 실패는 원래 SystemPortError를 반환하며 publish하지 않는다. 구독자 실패는 이미 저장된 기록을 되돌리거나 재발행하지 않으며 DeliveryReport로 호출자에게 보존한다. 이벤트 버스의 프로세스 내 호출 계약이며 프로세스 종료·취소를 가로지르는 영속 발행 보장은 만들지 않는다.

## 정의 대기

CashObservationFailure는 PortErrorKind보다 좁다. NotFound(Order) 및 확장 BrokerCapability의 정확한 domain 현금 오류 표현은 정의 대기다. 이 오류로 새 domain 타입을 만들거나 다른 원인을 발명하지 않는다. 현재는 Failed로 저장하고 반환 PortfolioSyncReport.failures에 원 PortError를 보존한다. 기존 SyncRecord에는 실패 세부 원인 저장 필드가 없다. 영속 상세 오류 조회는 별도 계약 확정 대상이다.

## 인계

- A-08: SyncRecord 결과와 기존 보유·현금 실패 사실을 표시하고 오류를 성공이나 0원으로 해석하지 않는다.
- A-19/U-08: 반환한 원 포트 오류와 DeliveryReport를 자체 점검·기록에 연결한다. 사용자 재시도 절차를 추가하지 않는다.
- D-36/A-05: Order 없음·확장 capability의 현금 오류 표현과 영속 실패 상세 계약을 메인이 평가한다.
- T-05/T-11: 보유·매수가능금액 응답의 실제 요청 계좌 결합은 어댑터 계약이다. 이 응답 타입에는 계좌 키가 없어 application이 진위를 다시 인증하지 않는다.
- O-01/O-04/O-17/A-14: A-04 주문 요약·본인 확인 실행 검증은 주문 경로에서 수행한다. A-06에는 주문 승인 경로가 없다.

## 검증 경계

실제 키·실제 계좌·실제 API·주문 전송 없음. 프로젝트 신뢰와 세션 훅 런타임 적용은 확인 불가다. Git 훅 설치는 확인했다. 홈 설정·인증 파일 변경 없음. 기존 테스트 변경과 새 검사 끄기, 외부·내부 의존성 추가 없음.

## 손 변형 실제 실행

작업 사본 아래 mktemp 복사본에서 기준 통과 후 서로 다른 변형을 하나씩 컴파일·실행했다. 컴파일 성공 21개, 검출 21개, 생존 0개이며 원복 대조군도 통과했다. 로그와 집계는 target/A-06/hand-*.log 및 hand-results.json에 남겼다. 임시 복사본을 제거했다. 자동 변이와 별개다.

| 번호 | 바꾼 것 | 잡은 테스트 |
|---|---|---|
| 1 | CashMissingData Stock | tests::cash_error_reasons_group_0 |
| 2 | CashMissingData ExchangeRate | tests::cash_error_reasons_group_0 |
| 3 | CashMissingData Sector | tests::cash_error_reasons_group_0 |
| 4 | CashResponseMismatch Schema | tests::cash_error_reasons_group_0 |
| 5 | CashResponseMismatch Validation | tests::cash_error_reasons_group_0 |
| 6 | CashRequestRejection MalformedRequest | tests::cash_error_reasons_group_1 |
| 7 | CashRequestRejection MissingAccountHeader | tests::cash_error_reasons_group_1 |
| 8 | CashRequestRejection InvalidToken | tests::cash_error_reasons_group_1 |
| 9 | CashRequestRejection EdgeBlocked | tests::cash_error_reasons_group_1 |
| 10 | CashRequestRejection UnsupportedQuery | tests::cash_error_reasons_group_1 |
| 11 | CashRequestRejection UnsupportedContentType | tests::cash_error_reasons_group_2 |
| 12 | CashRequestRejection BatchLimitExceeded | tests::cash_error_reasons_group_2 |
| 13 | CashUnavailableCause RateLimited | tests::known_retry_delay_is_not_replaced_with_unknown_or_zero, tests::cash_error_reasons_group_2 |
| 14 | CashUnavailableCause Maintenance | tests::cash_error_reasons_group_2 |
| 15 | CashUnavailableCause ServerError | tests::cash_error_reasons_group_2 |
| 16 | CashUnavailableCause Network | tests::cash_error_reasons_group_3, tests::success_partial_failure_blocked_and_storage_failure_record_before_one_event |
| 17 | CashUnavailableCause ExpiredToken | tests::cash_error_reasons_group_3 |
| 18 | 부분 실패를 완전 성공으로 | tests::cash_error_reasons_group_2, tests::cash_error_reasons_group_3, tests::cash_error_reasons_group_1, tests::success_partial_failure_blocked_and_storage_failure_record_before_one_event, tests::known_retry_delay_is_not_replaced_with_unknown_or_zero, tests::cash_error_reasons_group_0 |
| 19 | 보고값 누락 | tests::cash_error_reasons_group_2, tests::cash_error_reasons_group_0, tests::cash_error_reasons_group_1, tests::known_retry_delay_is_not_replaced_with_unknown_or_zero, tests::success_partial_failure_blocked_and_storage_failure_record_before_one_event, tests::cash_error_reasons_group_3 |
| 20 | 보유 누락 | tests::cash_error_reasons_group_0, tests::known_retry_delay_is_not_replaced_with_unknown_or_zero, tests::cash_error_reasons_group_3, tests::success_partial_failure_blocked_and_storage_failure_record_before_one_event, tests::cash_error_reasons_group_1, tests::cash_error_reasons_group_2 |
| 21 | 이벤트 두 번 발행 | tests::account_lookup_errors_and_unselected_accounts_save_without_reading_holdings, tests::cash_error_reasons_group_1, tests::success_partial_failure_blocked_and_storage_failure_record_before_one_event, tests::cash_error_reasons_group_2, tests::known_retry_delay_is_not_replaced_with_unknown_or_zero, tests::cash_error_reasons_group_0, tests::cash_error_reasons_group_3 |
