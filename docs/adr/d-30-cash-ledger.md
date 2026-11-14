# D-30: 현금 원장과 외부 흐름 (초안)

- 상태: 초안. 구현 기준 4c46c32e9239384b24ca0e13869b118fe2a3a3c7.
- 근거: I-05, design get_portfolio·ACC-6·ACC-7, R-01 Q5·Q14, R-11 Q1, ADR 0025·0028·0029·0034·0038.

## 현금 효과의 닫힌 표

R-01과 로컬 OpenAPI Transaction.netAmount는 “수수료와 세금을 반영한 정산 금액”, “계산하지 말고 이 값을 사용”이라고 정한다. R-11 Q1도 netAmount 누적을 지정한다. 아래는 모두 결제 완료·활성·취소 아님인 계산용 행에만 적용한다. 원금·수량·단가에서 재계산하거나 commission·tax를 다시 차감하지 않는다.

| TransactionKind | 현금 효과 | 외부 흐름 |
|---|---|---|
| Buy | -netAmount | 내부 |
| Sell | +netAmount | 내부 |
| Deposit, assetClass 없음 | +netAmount | 외부 유입 |
| Withdrawal, assetClass 없음 | -netAmount | 외부 유출 |
| Deposit, assetClass 있음 | +netAmount | 내부. 채권 만기 원리금 포함 |
| Withdrawal, assetClass 있음 | -netAmount | 내부. 채권 이자 세금 포함 |
| ExchangeIn | +netAmount, 받은 통화 | 내부 |
| ExchangeOut | -netAmount, 낸 통화 | 내부 |
| Dividend, Interest | +netAmount | 내부 수익 |
| Fee | -netAmount | 내부 비용 |
| SecuritiesTransferIn, SecuritiesTransferOut | 미확정. 0으로 누적하지 않고 사용 불가 | 판단 필요. 외부 흐름 전체 사용 불가 |
| Unknown 종류 또는 Unknown 상품 | 분류 불가 | 분류 불가 |

R-11은 “배당·이자·수수료는 포트폴리오 안의 수익과 비용”, 상품 있는 입출금은 채권 만기·세금에 따른 내부 흐름이라고 정한다. 채권 세금이 amount와 tax에 함께 있어도 중복 차감하지 않는다. 독립 TAX 유형은 현재 스키마에 없으므로 Unknown이며 사용 불가다. 증권 입출고는 “타사 이관이면 외부 흐름이고 주식배당이면 내부 수익”이고 금액도 null이다. D-28의 로트 분류를 선점하는 새 분류 타입·사용자 절차를 만들지 않는다. 분류 및 평가 근거를 붙이는 후속 계약 전에는 보수적으로 현금 대조와 계좌 성과 흐름을 보류한다.

## 입력과 신뢰 상태

I-05는 “화면에는 매수가능금액으로 표시”, 재구성은 “대조용”으로 확정했다. 단계 이름과 하락 조건의 상세 표는 현재 문서에 없으므로 다음 닫힌 상태를 초안으로 정한다.

| 상태 | 의미 | 값 접근 |
|---|---|---|
| Reconciled | 시작 잔고 있음, 조회 근거 Complete, 전 행 분류·산술 성공, 토스 제공값과 정확히 일치 | ComparisonCash의 compare_to만 가능 |
| Unavailable(reason) | 아래 조건 중 하나라도 해당 | 잔액을 보유하지 않음 |

사유는 UnknownOpening, IncompleteResponse, UnknownCurrency, UnconfirmedCancellation, UnclassifiedRow, MissingNetAmount, CurrencyMismatch, MissingReportedCash, InvalidReportedCash, ReportedDifference, Arithmetic(MoneyError)로 닫는다. 여러 사유가 겹치면 입력 근거 → 시작 잔고 → 정렬된 행 → 제공값 → 대조 순서로 첫 사유를 반환한다. 미지 통화는 Currency를 늘리거나 치환하지 않고 LedgerEvidence::UnknownCurrency로 전체 근거를 거부한다. 미동기화 집합은 Complete 증언이 있어도 불완전이다.

LedgerEvidence는 호출자의 명시적 증언이다. Complete는 해당 계좌의 지정 결제일 기간에 모든 종류·모든 페이지가 수신됐고, 미지 통화 격리가 없으며 미확인 취소가 없다는 뜻이다. SettledRowSet에는 과거 조회 창의 완전성 증명이 없으므로 마지막 동기화만으로 이를 추측하지 않는다. 어댑터·유스케이스는 실패 시 Incomplete·UnknownCurrency·UnconfirmedCancellation을 넘겨야 한다. 시작 잔고는 기간 첫날 직전 잔고다. 토스 대조값은 같은 계좌·기간 끝에 수집한 BuyingPower여야 한다. 이 외부 사실을 도메인 타입이 인증하지는 않는다.

CashLedger는 통화별 독립 결과다. 불완전 근거·미지 통화는 전체 기간의 문제이므로 두 통화를 모두 막는다. 다른 통화의 알려진 행은 해당 통화 원장에만 영향을 준다. 현금 변화는 SettledRowSet::active_settled_for_calculation만 읽는다. 사라진 원거래와 취소 행은 둘 다 계산에서 빠진다. 창 밖 원거래의 미확인 취소는 LedgerEvidence로 전달해야 한다.

## 대조와 타입 경계

CashComparison은 Matches·Different(CashDifference)·Unavailable이다. CashDifference는 잔액 ComparisonCash와 별도 봉인 타입이다. 차액으로 Reconciled 변형을 만들 수 없다. 차액은 재구성 - 토스 제공값이며 부호를 유지한다. 허용 오차는 없다. 원화 1원·달러 0.01달러 차이도 Different이며, 더 작은 소수 차이도 반올림해 숨기지 않는다. ADR 0038의 평가액 허용 오차를 현금에 적용하지 않는다.

ComparisonCash는 봉인된 필드와 비교 연산만 갖는다. Money·Amount·Decimal·표시 문자열을 반환하거나 BuyingPower·DisplayMoney로 변환하는 공개 API가 없다. 상태 공통 잔액 접근자도 없다. 차이가 있으면 원장 상태에는 잔액이 없고 대조 결과에 차액만 남는다. 표시·총자산·주문 가능 금액은 기존 토스 BuyingPower를 계속 사용한다.

행을 정렬한 뒤 정확한 Money checked_add/checked_sub만 적용한다. 표현 손실·넘침은 사용 불가이며 부분 합을 반환하지 않는다. 정렬은 입력 순서에 따른 결과·첫 오류 변화를 없앤다. 최종 합이 표현 가능해도 중간 합이 넘치는 경우는 보수적으로 거부한다.

## ExternalFlow와 시간

ExternalFlow는 봉인된 생성 경로에서 결제일 CalendarDate, 통화, Inflow/Outflow, 양의 절댓값 netAmount를 보존한다. 0 금액도 원본 관측으로 유지한다. R-01의 settlementDate는 “KST 기준 결제일”, 비매매 결제 행은 “거래가 처리된 날짜”다. 일별 TWR·MWR 입력은 이 날짜를 사용하며 transactionTime을 주문·체결 시각으로 해석하지 않는다. 흐름 방향은 포트폴리오 관점이다. D-09는 MWR 투자자 관점에서 입금 부호를 반대로 바꿔야 한다.

ExternalFlows는 Available(Vec<ExternalFlow>) 또는 Unavailable(reason)이다. 분류 불가·증권 입출고·외부 입출금의 누락 netAmount가 있으면 외부 흐름 부분 목록을 내보내지 않는다. 기간 밖·미결제·사라짐·취소는 제외한다. 정렬된 원본 순서와 중복 개수를 유지한다.

## 미해결과 경계

- I-05에 단계 명칭이 없으므로 본 초안의 명칭·증언 계약은 메인 평가 대상이다.
- 증권 입출고의 타사 이관·주식배당·기업 행위 판별 및 평가액은 D-28·D-09에서 근거를 붙여야 한다. 자동 외부 흐름 처리 없음.
- 토스 cashBuyingPower는 예수금이 아니다. 일치는 예수금 정확성이나 계좌 TWR의 정답 증명이 아니다.
- 이력 완전성·시각·계좌 일치의 네트워크 검증은 T-06·A-07 책임이다. 현재 타입은 증언을 요구하며 사실 인증은 못 한다.
- 프로젝트 설정 hooks=true와 Git 훅은 확인했다. 세션의 프로젝트 신뢰·훅 신뢰 적용은 증명하지 못해 직접 just check와 커밋 훅을 증거로 사용한다.
- 새 의존성·새 검사 끄기·실제 계좌 접근 없음. 기존 테스트·병렬 작업 파일·progress.md는 변경하지 않는다.

## 완료 기준과 검증

| 기준 | 테스트 |
|---|---|
| 종류별 netAmount 방향·중복 비용 차감 없음·통화 격리 | cash_kind_table_uses_net_once_and_keeps_currencies_separate |
| 낮은 신뢰에 잔액 없음·정확 차액 | cash_low_trust_never_contains_a_value_and_exact_difference_is_preserved, incomplete_inputs_never_expose_cash |
| 미분류·입출고·누락 금액은 0 아님 | unknown_missing_and_transfer_cash_are_not_silently_zero |
| 은행 입출금만 외부·채권과 환전은 내부·결제일 | external_flows_use_settlement_date_and_bank_deposits_only |
| 순서·통화별 독립 합·취소 행 추가 불변 | independent_integer_sum_order_and_cancelled_row_invariance (Config::with_cases(256)) |
| 소수 금액의 독립 정수 기준·현금과 흐름 순서 불변 | fractional_cash_matches_independent_minor_unit_sum (Config::with_cases(256)) |
| 미결제·사라진 행·취소·기간 밖 제외 | outside_window_pending_disappeared_and_cancellation_rows_do_not_contribute |
| 양쪽 기간 경계·미지 상품·외부 흐름 누락 금액 | unknown_assets_missing_external_amount_and_both_window_edges_are_checked |
| 넘침·정밀도 손실·대조 차액 넘침 | arithmetic_failure_and_missing_reference_are_unavailable, exact_arithmetic_errors_never_expose_partial_cash |
| 비교값의 부호·통화 오류 | comparison_only_values_preserve_signed_delta_and_currency_errors |

새 테스트를 먼저 연결해 타입 부재 컴파일 실패를 확인했다. 이 최초 실행에는 테스트 보조 함수의 NumericInput 생성자 오기도 포함돼 있었고 이후 기존 try_new API로 바로잡았다. 외부 crate rustc 실험은 비교 연산 호출 1건 성공, BuyingPower::new에 ComparisonCash 직접 전달(E0308), Money/DisplayMoney로 into(E0277), money/value 접근자 호출(E0599), 필드 접근(E0616), 생성자 위조(E0423), 차액으로 신뢰된 잔액 변형 생성(E0308)의 8건 실패를 확인했다. 임시 디렉터리는 실험 직후 삭제했다. 실제 키·계좌와 UI는 검증 범위가 아니다.

테스트 전용 파일을 src에 두면 하네스가 제품 파일로 집계해 미측정·미도달 테스트 분기를 보고하므로, 검사를 유지하고 새 테스트만 crates/domain/tests/cash_ledger_tests.rs로 옮겼다. lib.rs에는 제품 mod·pub use 줄만 추가한다.
