# D-30: 현금 대조와 날짜별 외부 흐름 (리뷰 수정 초안)

- 상태: 초안. 최초 대상 2f306d760e7e03a327642c338cc5c6283a7988be. 로컬 master e7a2064afc584dd464cad73f845d35fb6669d76e 병합 후 적대적 리뷰 수정.
- 근거: I-05, R-01 Q5·Q14, R-11 Q1·Q4·Q5와 위험 1·2, ADR 0025·0028·0029·0034·0038·0044.

## 현금 효과

계산은 D-08 active_settled_for_calculation만 읽는다. 취소·사라짐·미결제 행은 현금 합에 넣지 않는다. 완전성·취소 후보의 검사에만 원본 이력을 읽는다.

| 종류 | 현금 효과 |
|---|---|
| BUY, WITHDRAWAL, EXCHANGE_OUT, FEE | -netAmount |
| SELL, DEPOSIT, EXCHANGE_IN, DIVIDEND, INTEREST | +netAmount |
| SECURITIES_TRANSFER_IN/OUT | 거래 대금은 0. 명시된 commission·tax의 합만 유출. 둘 다 0이면 0 |
| 미지 종류·미지 상품, 필요한 정산 금액 누락 | 사용 불가 |

R-01과 Transaction 스키마의 netAmount는 절댓값이며 “수수료와 세금을 반영한 정산 금액”, “계산하지 말고 이 값을 사용”이다. 일반 거래에서는 commission·tax를 다시 차감하지 않는다. 증권 입출고는 스키마상 amount·netAmount가 null이다. commission은 입출고를 포함한 모든 유형의 수수료, tax는 기록된 세금 합계이므로 이 예외에서는 명시 비용만 정확히 뺀다. 입출고에 amount·netAmount가 채워진 모순된 입력은 TransferCashUnknown이며 추정하지 않는다. 채권 이자 세금 WITHDRAWAL의 amount·tax 중복도 netAmount 한 번으로 처리한다.

## 봉인된 완전 응답 증거

LedgerEvidence는 비공개 필드의 CompleteTransactionResponse를 가진다. from_complete_response만 공개한다. 전체 종류 필터가 아니면 증거를 만들지 못한다. 재구성은 증거의 계좌·기간 포함 관계·활성 결제 멀티셋과 응답 결제 멀티셋의 정확한 일치를 검사한다. 미동기화·다른 계좌·좁은 창·빠진 행·증거 없음은 IncompleteResponse다. 빈 이력은 실제 빈 완전 응답 증거가 있어야만 일치 상태를 만들 수 있다. 맨 Complete 열거형·복원·Default 경로는 없다.

이는 D-08처럼 어댑터가 마지막 페이지·필터·격리 없음의 사실을 검증한다는 경계다. 도메인이 네트워크 사실을 인증하지 않는다. 미지 통화 격리나 중단 응답에서 T-06은 완전 응답 자체를 만들지 않고 증거 없음으로 넘긴다.

시작 잔고는 기간 첫 결제일 직전 값이다. 아직 저장된 마지막 일치 잔액을 복원하는 S 포트가 없으므로 값의 실제 획득 경로는 미해결이다. A/S는 같은 계좌·통화·연속 기간의 마지막 일치 잔액과 그 시점을 저장하고, 다음 기간 시작 경계와 일치할 때만 opening을 공급해야 한다. 증거 없이 최초 잔고 0을 가정하지 않는다.

대조값은 같은 계좌·통화·기간 종료 시점의 토스 BuyingPower다. 이 값은 예수금이 아니며 미체결 주문·미결제 매도 대금·환전의 반영 방식은 T-13 실측 대상이다. 대조값 observed_at이 저장 이력의 해당 통화 마지막 처리 시각보다 앞서면 StaleReportedCash다. 같은 시각은 허용한다. 처리 시각이 없으면 선후를 증명할 수 없어 MissingSettlementTime이다. 시작 잔고의 출처·기간 경계와 BuyingPower의 계좌 연결은 A/S에서 검증하며 현재 Money·BuyingPower가 인증하지 않는다.

## 대조용 값과 상태

CashLedgerState는 Reconciled(ComparisonCash) 또는 Unavailable(reason)이다. CashComparison은 Matches·Different(CashDifference)·Unavailable이다. 차액은 재구성-토스 값이며 정확 비교한다. 원화 1원·달러 1센트와 그보다 작은 소수 차이도 숨기지 않는다. 정확한 Money checked 합·차만 사용하며 넘침·표현 손실은 사용 불가다. 정렬로 합산 순서와 첫 오류를 고정한다. 서로 다른 오류 두 행의 순열 회귀가 정렬 제거를 검출한다.

ComparisonCash·CashDifference는 봉인된 별도 타입이며 Money·표시 금액·BuyingPower로 공개 변환되지 않는다. 낮은 신뢰 상태에는 잔액이 없다. Debug는 금액을 가린다. 숫자 표시 문자열을 직접 내는 메서드는 없지만 Debug라는 공개 문자열 표현 자체는 존재하므로 “공개 문자열 API가 없다”고 주장하지 않는다. 비교 연산의 반복 질의로 값을 복원할 수 있는 성질은 받아들인다.

## 증권 입출고 분류와 평가

TransferClassification은 원본 Transaction과 동일 행 멀티셋 내 TransferOccurrence, 비공개 닫힌 효과를 가진다. 공개 읽기 분류는 InternalCorporateAction·ExternalInflow·ExternalOutflow·Unclassified다.

- 내부 경로: D-28의 CorporateDelivery를 그대로 받는다. Received이고 실제 SecuritiesTransferIn 입고가 연결되어야 한다. D-28의 LotAssumption::ReceiptMatch를 중복 정의하지 않는다. Pending·NotReported·입고 없음은 내부 증거가 아니다.
- 외부 경로: UserTransferConfirmation은 비공개 필드의 확인 증거 자리다. 지금 공개 생성자는 없다. 소비 메서드가 crate 내부 from_user_confirmation을 호출한다. A의 실제 확인 증거로만 이 타입을 발급하는 어댑터는 후속으로 남긴다. 단순 불린·공개 enum으로 사용자 확인을 위조하는 생성 경로는 없다.
- 평가: R-11의 과거 구간 재구성 규칙에 따라 결제일의 수정 전 확정 일봉 종가×실제 입출고 수량을 거래 통화로 계산한다. Daily·Finalized·Raw·같은 날짜·같은 통화를 검사한다. 금액을 모르거나 종가 자료가 없으면 Unclassified로 남긴다. 원화 합산에는 R-11 Q4의 해당 날짜 Mid 환율이 필요하며 D-09가 D-04로 환산한다. transactionTime·exchangeRate 원본 사실은 ExternalFlow에 그대로 보존한다. 거래 환율을 Mid로 가장하지 않는다.
- R-11은 외부 이관의 일중 공정가치 시각·비거래일 가격 대체·채권/옵션 평가 소스를 완결하지 않았다. 이 회차는 확정 일봉 근거가 있는 주식의 일별 재구성만 선택한다. 자료·동일 종목 대응이 확인되지 않으면 A는 확인 증거를 발급하지 않는다. 임의 가격·취득 원가·0원을 외부 흐름 평가로 쓰지 않는다.
- 옵션 만기 출고는 현재 D-28의 기업 행사 입고 연결 증거가 아니므로 내부로 추측하지 않는다. 그 결제일만 미분류로 남고 이후 기간은 막지 않는다. A-07의 실제 만기 식별 자료와 D-28의 만기 행사 증거 계약이 추가되기 전에는 해당 날짜의 계좌 수익률을 보류한다.
- D-08은 거래 ID가 없으므로 동일 행의 멀티셋 개수도 보존한다. 사용자 분류는 동일 행의 0부터 시작하는 발생 번호에 묶는다. D-28은 유일한 입고를 연결하므로 내부 분류는 발생 번호 0이다. 같은 행·발생 번호에 복수 분류가 있으면 ConflictingClassification이다.

## 날짜별 결과와 취소

현금이 일치한 CashLedger에서만 external_flows를 얻는다. ExternalFlows는 Reconciled(FlowCalendar) 또는 Unavailable(CashUnavailable)이다. FlowCalendar의 day는 Confirmed(봉인된 ConfirmedFlowDay) 또는 Uncertain(reason)이다. 원시 부분 흐름 목록을 공통 접근자로 노출하지 않는다. 미분류 입출고가 있는 날은 그 날 전체가 Uncertain이며 다른 날짜의 확정 상태를 지우지 않는다.

D-09가 받는 기간 토큰은 FlowPeriod::Confirmed(봉인된 ConfirmedFlowPeriod)다. period는 현금 원장이 대조한 기간과 정확히 같은 기간만 받는다. “D-09는 같은 기간의 현금 원장 대조가 일치일 때만 계좌 수익률을 낸다.” 다른 기간을 계산하려면 그 기간의 시작 잔고·토스 대조값으로 새 원장을 먼저 대조해야 한다. 미분류 날짜·취소 불확정 범위를 걸치면 기간 토큰이 나오지 않는다. 따라서 배당 입고 한 건이 모든 미래 기간의 수익률을 막지 않는다.

취소는 호출자의 기억으로 증언하지 않는다. D-08 CancellationIdentity의 종류·시장·통화·종목·상품·ISIN·수량·amount 키를 그대로 사용한다. 활성 취소 개수와 사라진 원거래 개수를 멀티셋으로 상계한다. 남은 취소는 활성 원거래 후보의 최소 결제일부터 취소일 및 후보 최대일까지의 범위를 UnmatchedCancellation으로 둔다. 후보도 없으면 취소 결제일을 막는다. 원거래가 기간 안·취소가 밖이거나 그 반대인 경우도 전 이력을 검사하여 해당 범위와 겹치는 날짜·기간을 막는다. 사라진 취소는 효과가 없다. 불확정 범위는 구간으로 보존하고 수천 년의 날짜를 열거하지 않는다.

## 책임 분담과 후속 인계

| 주체 | 만든다 | 받는다·적용한다 |
|---|---|---|
| T-06·A-07 (D-08 계약) | 페이지·종류·격리를 검증한 CompleteTransactionResponse와 대조된 SettledRowSet | 미지 통화·불완전 응답에는 증거를 발급하지 않음 |
| D-28 | 기업 행사와 원본 입고의 유일한 연결, CorporateDelivery·Received·ReceiptMatch 표지 | 분류를 D-30에 요구하지 않음 |
| A의 사용자 확인 작업 | 거래 행·발생 번호·동일 종목의 일봉 평가 근거를 확인한 UserTransferConfirmation 발급 경로 | D-30의 봉인 입력 계약을 구현. 현재 발급 어댑터는 미구현 |
| D-30 | CorporateDelivery 또는 사용자 확인 증거를 소비해 TransferClassification, 날짜별 불확정, 같은 기간 현금 대조가 일치한 ConfirmedFlowPeriod | 로트 원가·기업 행사 수량을 재계산하지 않음 |
| D-09 | 확정 기간 토큰의 외부 유입·유출로 TWR·MWR 계산, 필요 시 Mid 환산 | 두 통화의 같은 기간 토큰과 평가 스냅샷을 검증. 미분류 날짜·대조 불가를 걸친 계좌 수익률은 내지 않음 |
| S·A의 잔고 저장/복원 | 같은 계좌·통화·기간 경계의 마지막 일치 잔액과 출처·시점 | 연속성이 입증되지 않은 opening을 공급하지 않음 |
| T-13 | BuyingPower의 미체결·미결제·환전 반영과 실제 앱 대조 실측 | 예수금과 같은 값이라는 가정을 확정하지 않음 |

## 검증과 한계

리뷰 반례를 먼저 테스트로 추가한 뒤 구현한다. 속성 테스트는 Config::with_cases(4096)이며 매매·환전·배당·입출금·취소·증권 입출고·두 통화를 섞고 독립 정수 센트 합 및 순열을 대조한다. 기존 master 테스트는 수정하지 않는다. 불필요한 통합 테스트 파일은 제거하고 모듈 단위 테스트로 옮긴다. 단위 테스트의 cfg(test)는 보호된 forbid(clippy::tests_outside_test_module) 때문에 유지한다. 실제 Clippy 실험에서 감싸기 없는 테스트가 이 규칙으로 거부됨을 확인했다.

이번 회차는 사용자 지시에 따라 자동 변이를 중단하고 손 변형 14개 이상으로 검증한다. 새 의존성·검사 끄기·실제 키·계좌 접근은 없다. 남은 제품 범위는 위 표의 명시된 A/S 발급·저장 어댑터, T-13 실측, D-09 수익률과 환산 구현이다. 프로젝트·훅 신뢰 적용을 추측하지 않고 직접 검사와 커밋 훅의 전체 검사 결과를 증거로 사용한다.
