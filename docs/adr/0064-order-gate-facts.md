# ADR 0064: D-34 주문 게이트 사실

- 상태: 확정 (2026-11-14)
- 최초 기준: 6a33bc1b1ec4ef9dd055b60f10c0f93aba350a20
- 리뷰 수정 병합 기준: master 9b7c74285cca8ac83d914e609962695aad58ad8b

## 입력과 봉인

SellableQuantityFact는 계좌 키, D-33 주문용 종목, 매도가능수량, 조회 시각을 봉인한다. 국내 소수 조회 수량을 거부한다. check_sellable_quantity는 다른 계좌·종목이면 DifferentFact, 국내 소수 요청이면 FractionalDomesticRequest, 한도 초과이면 Exceeded, 그 밖에는 Allowed다. 정규화한 수학적 값으로 판단해 10.0은 정수다. Allowed는 매도가능수량 한도만 충족하며 보유 수량·장 시간·승인까지 인증하지 않는다.

BrokerOrderRow는 어댑터 입력이며 계좌 키를 포함한다. BrokerOrder의 비공개 필드에 봉인하고 account_id, order_id, instrument, side, status, size 및 아래 개별 접근자만 제공한다. 봉인된 입력 행을 다시 꺼내는 row()는 없다. BrokerOrderSet::from_open_lookup_result는 Vec<BrokerOrderRow>만 받는다. 봉인된 주문이나 주문 목록을 다른 계좌 집합의 입력으로 받지 않는다. 행 계좌와 집합 계좌가 다르면 AccountMismatch다. 중복 주문 ID, 행과 목록의 조회 시각 불일치도 거부한다. BrokerOrderId는 1~256 ASCII 표시 문자로 제한해 공백·제어 문자를 거부한다. 오류는 원문을 담지 않고 사실·입력 행·금액과 가격을 포함한 새 타입의 Debug는 원문을 가린다.

## 수량 주문과 금액 주문

| 원 응답 | BrokerOrderSize | 미체결 부분 |
|---|---|---|
| orderAmount 없음 | Quantity(quantity) | 주문 수량 − 누적 filledQuantity를 기존 Amount 정밀 뺄셈으로 계산한 UnfilledQuantity::Known. 초과나 정밀도 손실은 오류 |
| orderAmount 있음 | Amount { reported_quantity, order_amount } | UnfilledQuantity::UnknownAmountOrder. 조회 수량과 체결 수량을 대조하지 않음 |

Amount는 스키마가 명시한 미국·MARKET·BUY·USD 양수 금액에 한정한다. reported_quantity는 스키마 quantity를 그대로 보존하며 0이어도 체결 수량 0.5를 거부하지 않는다. order_amount()는 BrokerOrderMoney::Known, 수량 주문은 Absent다. quantity()는 응답이 제공한 수량이지 금액 주문의 잔량이 아니다. T-15는 실제 금액 주문에서 quantity와 filledQuantity가 어떻게 내려오는지 실측한다. 실측 전 임의로 수량을 합성하지 않는다.

Order에는 미체결 수량 필드가 없다. Known도 취소·종료 뒤에는 취소된 부분을 포함할 수 있으므로 활성 잔량이나 새 주문 가능 수량으로 쓰지 않는다. 반대 방향 미체결 판정은 두 변형 모두 주문 상태로 보수적으로 한다.

## OPEN 조회의 완전성과 상태

BrokerOrderSet은 계좌별 OPEN 조회용이다. coverage()와 observed_at()을 제공한다. Complete에서 허용하는 알려진 상태는 PENDING, PENDING_CANCEL, PENDING_REPLACE, PARTIAL_FILLED뿐이다. FILLED, CANCELED, REJECTED, CANCEL_REJECTED, REPLACE_REJECTED, REPLACED가 섞이면 ClosedInOpenResult다. Unknown은 원문 MasterCode를 보존하고 허용하며 미체결 가능 상태로 다룬다. Incomplete는 받은 종료 상태도 보존하되 주문 없음의 증거가 되지 않는다.

opposite_open_order는 먼저 계좌를 확인하므로 미국·다른 계좌도 DifferentAccount다. 같은 계좌 미국은 NotApplicable이다. 국내 같은 종목 반대 방향에서 위 네 진행 상태 또는 Unknown이 있으면 Present다. Unknown은 수량 주문 잔량이 0이어도 차단한다. 없으면 Complete는 Absent, Incomplete는 Incomplete다. 취소·정정 거부는 별도 종료 레코드이고 원주문이 이전 상태로 복귀한다. PARTIAL_FILLED는 CLOSED 그룹에도 나타날 수 있으므로 상태만으로 CLOSED 조회의 완전성을 주장하지 않는다.

## 체결 금액과 주문 메타데이터

| 스키마 필드 | 보존·접근자 |
|---|---|
| execution.filledQuantity | filled_quantity(), 누적 체결 수량 |
| execution.averageFilledPrice | average_filled_price(), nullable 평균가 |
| execution.filledAmount | filled_amount(), BrokerOrderMoney::Absent / Known(Money). 종목 거래 통화와 다르거나 음수이면 거부 |
| execution.filledAt | filled_at(), nullable 최종 체결 시각 |
| orderedAt | ordered_at(), 주문 시각 |
| price | price(), BrokerOrderPrice::Absent / Known(Price). 평균가와 별도로 통화 일치 검사 |
| orderType | order_type(), BrokerOrderType::Limit / Market / Unknown(MasterCode) |
| timeInForce | time_in_force(), BrokerTimeInForce::Day / Cls / Opg / Unknown(MasterCode) |
| canceledAt | canceled_at(), BrokerOrderTime::Absent / Known(Timestamp) |
| 조회 시각 | observed_at(), 위 원 응답 시각과 독립 |

D-38은 토스의 filledAmount를 사용하며 averageFilledPrice × filledQuantity로 대신하지 않는다. 총액이 없으면 부족한 증거로 처리한다. filledAt은 최종 체결 시각이고 총액·평균가·수량은 누적값이다. 여러 평가 구간에 걸친 부분 체결을 한 구간의 개별 체결처럼 배분할 수 없다. 이때 불완전으로 처리하거나 별도 체결 증거를 확보하며 결제일에서 역산하지 않는다.

O-11과 O-19가 쓸 주문 시각·가격·유형·유효조건·취소 시각을 보존한다. Order 스키마에 없는 clientOrderId나 승인 확인 필드를 발명하지 않는다. O-19의 같은 내용 대조는 저장된 요청·감사 기록과 결합해야 한다. 미지 유형·유효조건은 과거 수량 주문에서 원문 보존하지만 새 주문 허가나 같은 내용 확인으로 추측하지 않는다. 수수료·세금·결제일 등 이번 소비 계약 밖 필드는 O-11/O-26 및 A-20·T-15 후속에서 필요성을 확정한다.

## 유의사항

StockWarnings는 종목·조회 시각과 중복 없는 집합이다. 알려진 여덟 값 LIQUIDATION_TRADING, OVERHEATED, INVESTMENT_WARNING, INVESTMENT_RISK, VI_STATIC_AND_DYNAMIC, VI_STATIC, VI_DYNAMIC, STOCK_WARRANTS와 원문 Unknown을 보존한다. check_stock_warnings는 주문 방향을 받는다. 매도에는 NotApplicable, 매수의 다른 종목에는 DifferentInstrument, 같은 종목의 빈 집합은 Clear, 하나라도 있으면 Blocked다. 정리매매 종목의 청산 매도를 이 게이트가 막지 않는다.

스키마는 활성 항목만 반환한다. 거래소·기간으로 별도 우회 허용이나 날짜 재판정을 하지 않는다. design 6.4와 스키마에는 안내 전용 구분 근거가 없어 매수는 알려진 여덟 종류와 Unknown 모두 차단하는 보수적 초안이다.

## 어댑터 의무와 후속 작업

SellableQuantityFact·StockWarnings와 BrokerOrderRow의 원시 응답 생성은 어댑터 신뢰 경계다. 생성자는 네트워크 진위나 요청과 응답의 실제 대응을 인증하지 않는다. T-15·T-11·A-20은 다음 세 의무를 넘겨받는다.

1. 계좌·종목은 실제 요청에 사용한 값 및 응답의 종목과 결합한다. 계좌가 없는 종목 경고 응답은 요청 종목을 반드시 결합한다.
2. 식별하지 못한 행·검증 실패·격리가 있으면 버리고 Complete로 만들지 않고 Incomplete로 남긴다. 과거 상장폐지·정지·미지 유형 때문에 주문용 식별을 만들 수 없는 주문도 이에 포함한다. 후속 읽기 식별 계약이 필요하다.
3. 종목·기간 필터를 건 OPEN 조회를 Complete로 선언하지 않는다. 실패한 조회를 빈 성공 집합으로 대체하지 않는다.

| 소유 작업 | tasks.md에 반영할 후속 계약 |
|---|---|
| D-38 | filledAmount로 순매수 체결 금액 계산. 평균가 × 수량 대체 금지. 누적 체결의 구간 배분 불가·총액 부재는 불완전 처리. T-15의 CLOSED 완전성 증거 소비 |
| T-15 | ClosedOrderQuery(계좌·조회 범위·필터), ClosedOrderRead와 CompleteClosedOrders / IncompleteClosedOrders 등 CLOSED 페이지 종료·격리 없음·범위 대응 증거를 정의하고 공급. D-38과 계약 확정. 금액 주문의 응답 수량 실측 |
| T-15·A-20 | 위 계좌·종목 결합, 식별 불가 행의 Incomplete, 필터 OPEN 조회의 Complete 금지 의무를 포트·계약 테스트로 연결. 주문 메타데이터를 O-11·O-19에 전달 |
| 하네스 후속 | 조회 사실 생성 함수 호출 위치를 실제 응답 어댑터와 허용된 테스트 픽스처로 제한. 원시 입력 재조립·재수출 경로도 검사 |

CLOSED 완전성 타입 구현은 이번 범위 밖이며 T-15가 공급 타입·페이지 검증을 소유하고 D-38이 필요한 평가 구간 소비 계약을 확정한다. CLOSED의 from/to는 orderedAt 기준이므로 평가 구간 filledAt 필터로 바꿔 해석하지 않는다. 오래 전에 낸 주문의 현재 체결도 포함할 수 있는 조회 범위를 증명해야 한다. API가 지원하지 않는 호가 유형은 OPEN·CLOSED·상세에 나오지 않는 스키마 한계도 T-15 실측·수용 과제다.

미해결: ID 256바이트와 MasterCode 64바이트는 내부 보수 상한이다. 초과 행은 절삭하지 않고 실패·불완전 처리한다. 조회 최신성 허용 간격과 승인 직전 재조회는 O-01/O-04 책임이다. 실제 API·계좌·키·주문 전송은 이번 범위에 없다. 세션의 프로젝트·훅 신뢰 적용 자체는 확정할 증거가 없어 직접 검사 및 Git 훅 결과를 근거로 한다.

master 병합 커밋은 훅이 merge-base 기준으로 이미 승인된 D-36 테스트 변경 세 건을 다시 거부하여 메인의 명시 승인으로 훅을 건너뛰었다. 이 기준 처리 한계는 H-44에서 고친다. 수정 커밋은 정상 훅으로 검사한다.
