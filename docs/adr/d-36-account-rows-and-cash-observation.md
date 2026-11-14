# D-36 계좌 행과 현금 관측

- 상태: 초안
- 최초 구현: f7011cc30c128b2304172a2ad6544a431293f1db
- 리뷰 수정 기준: master 792a35b5da51e9a8fff146793a172d1c2e210fc1을 bc450f0에서 병합. D-24·DS-03·H-43 포함.
- 근거: ADR 0022·0038·0043·0045·0047·0048·0054·0055·0057, design 2.5·UX-4, architecture 3.3, OpenAPI 1.2.28.

## 계좌 응답 경계

Account의 원시 필드는 accountSeq(int64), accountType(string), accountNo(string)이다. 조회 시각은 어댑터가 공급한다. BrokerAccountRow::from_broker_account_result는 응답 한 행의 AccountId·BrokerAccountTypeCode·BrokerAccountNumberInput·Timestamp를 받아 내부에서 번호를 검증·가리고 유형을 해석한다. 이미 가린 MaskedAccountNumber나 해석된 BrokerAccountKind를 받지 않는다.

원시 타입 규칙에 따라 원문 번호 문자열은 BrokerAccountNumberInput 경계 newtype으로 전달한다. 이 입력은 원문 String을 이동해 받고, Clone·공개 접근자·Display가 없으며 Debug도 원문을 숨긴다. 행 생성자가 입력을 소비하고 뒤 네 자리만 행에 보관한다. 원시 문자열을 임의로 만드는 것은 어댑터의 신뢰 경계이며 IO 진위를 타입으로 인증하지 않는다. 호출 위치를 실제 계좌 응답 어댑터로 제한하는 것은 하네스 후속이다. 가린 번호 객체를 반환하던 masked_number는 제거했다. 표시에는 계좌 키와 번호가 묶인 display_fact만 제공한다.

번호 스키마는 문자열과 예시 12345678901만 명시하며 길이·패턴 제약은 없다. ADR 0022의 임시 ASCII 숫자 8~20자리 검증을 재사용하고 하이픈을 정규화하지 않는다. 뒤 네 자리만으로 입력을 다시 만들어도 이 검증을 통과하지 않는다.

| accountType | BrokerAccountKind |
|---|---|
| BROKERAGE | Brokerage: 종합매매 |
| OVERSEAS_DERIVATIVES | OverseasDerivatives |
| PENSION_SAVINGS | PensionSavings |
| RESHORING_INVESTMENT | ReshoringInvestment |
| 그 밖의 원문 | Unknown(봉인된 UnknownBrokerAccountKind), 원문 보존 |

AccountLookupDisplayRow::from_broker_account_result와 AccountDisplayFact::from_account_lookup_result는 crate 내부로 닫았다. 기존 domain 테스트의 내부 사실 생성은 유지한다. 메인이 승인한 application/tests/ports.rs의 account()와 필요한 import만 새 행 경로로 변경했고 기존 단언은 유지했다. 승인된 test-diff 예외는 이 변경에 한정한다. D-33이 수정하는 다른 도우미와 본문은 변경하지 않는다.

BrokerAccountRows::try_new는 같은 AccountId의 중복을 거부하고 응답 순서를 보존한다. 두 선택 함수는 이 검증된 목록만 받는다. select_brokerage_account는 design의 첫 종합매매 계좌와 ADR 0055의 서버 순서 보존에 따라 첫 Brokerage를 고른다(Selected/NoBrokerage/Empty). select_brokerage_account_by_id는 목록 안의 Brokerage만 고른다(Selected/Empty/NotFound/NotBrokerage). Unknown은 자동·명시 선택 모두 거부한다.

## 현금 실패와 A-01 대응

이름은 ADR 0055의 CashObservationFailure로 통일하고 CashLookupFailure 이름을 제거한다. CashObservation은 Observed(BuyingPower) / Failed { currency, reason, observed_at }다. 실패에는 금액이 없고 관측 시각은 실패 관측 시각이다. 원문·금액·서버 본문은 오류에 없다. A-06은 PortError 전체(엔드포인트·HTTP 상태 포함)를 별도로 보존하면서 kind를 아래와 같이 옮길 수 있다. domain은 application에 의존하지 않는다.

| A-01 PortErrorKind | CashObservationFailure | 세부 대응·안내 구분 |
|---|---|---|
| AccessDenied(AccessDenial) | AccessDenied(CashAccessDenial) | InvalidCredentials, IpNotAllowed, TokenRevokedRepeated, AccountNotFound, Forbidden, LoginUserNotFound를 같은 이름으로 일대일 보존. 사용자 조치 |
| Unavailable { cause, retry } | Unavailable { cause: CashUnavailableCause, retry: CashRetryDelay } | RateLimited, Maintenance, ServerError, Network, ExpiredToken을 같은 이름으로 보존. 속도 제한·점검·서버·네트워크·토큰 만료를 구분 |
| RetryDelay::Unknown / Known(TimeSpan) | CashRetryDelay::Unknown / Known(TimeSpan) | 대기 시간 전체를 그대로 보존. 임의 기본값 없음 |
| NotFound(MissingData) | NotFound(CashMissingData) | Stock, ExchangeRate, Sector를 같은 이름으로 일대일 보존 |
| InvalidResponse(ResponseMismatch) | InvalidResponse(CashResponseMismatch) | Schema, Validation 보존. 응답 오류 |
| Unsupported(BrokerCapability) | Unsupported(CashReadCapability) | DailyCandles, MinuteCandles, AdjustedHistory, RawHistory 보존 |
| InvalidRequest(RequestRejection) | InvalidRequest(CashRequestRejection) | MalformedRequest, MissingAccountHeader, InvalidToken, EdgeBlocked, UnsupportedQuery, UnsupportedContentType, BatchLimitExceeded 보존. 요청 오류 |

새 자유 함수 portfolio_from_cash_observations는 기존 Portfolio를 반환한다. 기존 Portfolio::new·BuyingPower::new는 그대로이며 조회 실패는 BuyingPower.money의 PortfolioError::CashLookupFailed로 전달한다. 실패한 현금만 제외하고 총자산은 Total::Incomplete이며 보유·통화별 합·다른 현금은 유지한다. 실패 관측의 통화가 슬롯과 다르면 CurrencyMismatch이며 슬롯 통화로 덮어쓰지 않는다. buying-power는 예수금이 아닌 현금 기반 주문 가능액이고 현금 원장 잔액과 혼동하지 않는다.

CashObservationFailures는 Krw(reason) / Usd(reason) / Both { krw, usd }다. 비어 있는 실패 집합은 생성하지 않는다. D-06은 두 통화의 조회 실패를 먼저 모아 AllocationError::CashObservationFailed로 돌려준다. 실제 음수 현금의 NegativeCash(Currency)와 구별하며 음수 원화 현금도 실패한 달러 조회를 가리지 않는다. 기존 allocation impl은 바꾸지 않고 자유 함수 asset_values의 사전 검사와 오류 변형만 추가했다.

D-12는 PolicyViolation::Allocation에 전체 실패 집합을 그대로 보존한다. D-24는 NoSignalReason::Unavailable(SignalUnavailable::CashObservationFailed)에 전체 집합을 옮기고 실제 음수는 SignalUnavailable::NegativeCash로 구별한다. 보유 구성비는 제공하고 총자산 비중·매수는 차단한다. 매도 판정 규칙은 바꾸지 않는다.

## 검증과 후속

4,096회 속성 검사는 계좌 선택 순서·결정성과 현금 부분 실패 격리·독립 합계를 유지한다. 추가 회귀는 중복 키 거부, 명시 선택, 번호 입력 검증·Debug 가림, 반대 통화 슬롯 거부, 달러 실패/원화 실패/둘 다 실패/실제 음수의 D-06·D-12·D-24 소비자 결과와 재시도 시간·두 이유 보존을 확인한다. 실제 음수 원화와 달러 실패가 섞인 사례도 검사한다.

외부 crate에서 원시 응답 행→표시 사실과 갱신된 AllocationError·SignalUnavailable의 exhaustive match는 컴파일된다. 기존 생성자 두 개와 리뷰어의 행 부품 섞기 재현은 컴파일 거부된다. 저장된 실험 결과와 검사·변이 결과는 구현 보고에 남긴다. H-43 자동 변이는 병합한 master SHA를 기준으로 한 번만 실행하고 20분을 넘으면 소유한 실행만 중단한 뒤 손 변형 16개 이상으로 대신한다.

실패 관측 시각과 보유 조회 시각의 차이·신선도 검사는 A-08 책임이다. A-06의 실제 PortError 변환과 어댑터 호출 위치 강제, 계좌 번호의 실측 형식은 후속이다. 원시 응답의 진위는 여전히 어댑터 신뢰 경계다. 세션의 프로젝트·훅 신뢰 적용 자체는 증명하지 못해 직접 검사와 Git 훅 결과를 근거로 한다.

새 의존성·검사 끄기·사용자 절차는 없다. 승인된 test-diff 예외 외 전체 검사를 확인한 뒤 사용자 지시대로 LEFTHOOK=0으로 커밋하고 커밋 메시지에 사유를 남긴다. master 수정·푸시·PR·실제 계좌·키 접근은 없다.

## 이번 회차 실행 결과

전체 check는 915/915 테스트와 커버리지 580/580·위반 0을 통과하고 승인된 application/tests/ports.rs의 import·account() 변경에 대한 test-diff 3건에서만 종료했다. 이후 단계인 disable-report·disable-report-since·hooks-installed를 별도로 실행해 새 끄기 0건과 훅 설치를 확인했다.

just mutants-diff 792a35b5da51e9a8fff146793a172d1c2e210fc1은 한 번 실행했으나 H-43의 test-diff 전제가 같은 승인 3건을 거부해 변이 생성 전에 끝났다. 20분 시간 초과는 아니며 자동 변이 본체는 미실행이다. 보호된 전제 검사를 바꾸지 않고 가장 가까운 검증으로 손 변형 23개를 실행해 전부 테스트 검출·생존 0·시간 초과 0을 확인했고 모두 복원했다. 실패 통화를 자리 통화로 덮는 원화·달러 두 변형도 각각 검출했다. 승인된 test-diff 예외를 H-43 자동 범위 산정에 전달하는 방법은 하네스 후속이다.
