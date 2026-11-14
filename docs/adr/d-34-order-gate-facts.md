# D-34 주문 게이트 조회 사실

- 상태: 초안
- 기준: 6a33bc1b1ec4ef9dd055b60f10c0f93aba350a20

## 생성과 봉인

매도가능수량은 계좌 키, D-33의 주문용 종목, 수량, 조회 시각을 묶는다. 국내 소수 수량은 거부한다. 판정은 양수 SellOrderQuantity와 계좌·종목을 모두 대조한다. Allowed는 매도가능수량 한도만 충족하며 보유 수량·장 시간·승인까지 인증하지 않는다. D-26과 O-01이 나머지 게이트를 결합한다.

주문 행은 BrokerOrderRow로 읽기 어댑터에서 입력하고 BrokerOrder의 비공개 필드에 봉인한다. BrokerOrderId는 1~256 ASCII 표시 문자로 제한한다. 오류는 입력 원문을 담지 않으며 새 사실과 입력 행의 Debug는 내용을 가린다. 생성자는 네트워크 진위나 계좌와 응답의 실제 대응을 인증하지 않는다. T-15가 요청 계좌와 각 행을 결합해야 한다. 주문용 종목은 마스터에서만 만들며 과거 상장폐지·정지·미지 유형 때문에 주문용 식별 생성이 안 되는 주문의 보존은 후속 읽기 식별 계약이 필요하다. 이런 행을 버리고 완전 목록으로 표시하면 안 된다.

BrokerOrderSet은 한 계좌의 OPEN 조회 결과다. 중복 주문 ID와 행·목록 조회 시각 불일치를 거부하며 ID 순으로 보존한다. Complete는 종목·기간 필터 없는 OPEN 응답 전체를 받은 어댑터의 선언이다. 부분 조회·페이지 중단·격리 행이 있으면 Incomplete를 써야 한다. CLOSED 조회나 임의 필터 결과를 Complete로 선언해서는 안 된다. API가 지원하지 않는 호가 유형 주문은 목록·상세에 나오지 않는다는 스키마 제한도 T-15/O-01의 실측·수용 과제다.

국내 같은 종목 반대 방향에서 PENDING, PENDING_CANCEL, PENDING_REPLACE, PARTIAL_FILLED와 Unknown은 미체결 가능 상태다. 미지 상태는 잔량이 0이어도 차단한다. FILLED, CANCELED, REJECTED, CANCEL_REJECTED, REPLACE_REJECTED, REPLACED는 스키마의 종료 상태다. 취소·정정 거부는 별도 레코드이고 원주문이 이전 상태로 복귀한다. 국내에서 가능 주문이 있으면 Present, 없고 완전하면 Absent, 불완전하면 Incomplete다. 다른 계좌는 DifferentAccount, 미국은 NotApplicable이다. PARTIAL_FILLED는 CLOSED 그룹에도 나타날 수 있으므로 이 상태만으로 종료를 추측하지 않는다.

## 체결과 유의사항

Order에는 미체결 수량 필드가 없다. unfilled_quantity는 주문 수량에서 누적 filledQuantity를 정확히 뺀 미체결 부분이며 종료 뒤에도 취소된 수량을 포함할 수 있다. 활성 잔량이나 새 주문 가능 수량으로 사용하지 않는다. 체결 수량 초과와 평균 가격 통화 불일치를 거부한다. 잔량은 기존 Amount의 정밀 뺄셈을 사용하며 표현 불가능한 결과를 반올림하지 않고 Arithmetic 오류로 남긴다. nullable 평균 가격과 filledAt은 독립 보존한다. D-38용 접근자는 filled_quantity, average_filled_price, filled_at이며 조회 시각은 observed_at이다.

filledAt은 최종 체결 시각이고 averageFilledPrice는 누적 평균이다. 여러 평가 구간에 걸친 부분 체결을 개별 체결처럼 구간에 배분할 수 없다. D-38은 이 경우 불완전으로 처리하거나 별도 체결 증거를 확보해야 하며 결제일에서 역산하지 않는다. 금액 주문에서 수량 0 등 조회 행의 허용 조합은 실측 전 임의로 제한하지 않는다.

StockWarnings는 종목·조회 시각과 중복 없는 유의사항 집합이다. LIQUIDATION_TRADING, OVERHEATED, INVESTMENT_WARNING, INVESTMENT_RISK, VI_STATIC_AND_DYNAMIC, VI_STATIC, VI_DYNAMIC, STOCK_WARRANTS 및 원문 MasterCode를 담은 Unknown을 보존한다. 스키마는 활성 항목만 반환하며 거래소와 기간은 이 집합의 게이트 소비 계약에 포함하지 않는다. 거래소별 우회 허용이나 날짜 재판정은 하지 않는다. 비어 있으면 Clear, 하나라도 있으면 Blocked, 다른 종목이면 DifferentInstrument다. 이 함수는 매수 게이트 전용이다.

## 미해결과 후속

- design 6.4와 스키마에는 안내 전용 유의사항 구분이 없다. 가장 보수적으로 알려진 여덟 종류와 Unknown 전부 매수를 막는 초안이다. 안내만 하는 값은 현재 없다.
- ID 256바이트, 미지 코드 MasterCode의 64바이트 제한은 내부 보수 상한이다. 넘는 행은 조회 실패·불완전으로 취급하며 절삭하지 않는다.
- 조회 시각의 최신성 허용 간격은 O-01/O-04가 Clock과 승인 직전 재조회로 정한다. 시각 보존만으로 최신성을 주장하지 않는다.
- A-20은 읽기 포트에 이 타입들을 연결한다. 실제 API 호출, 키, 계좌 자료, 주문 전송은 이번 범위에 없다.
- 프로젝트 hooks=true 및 Git 훅 설치를 확인했다. 현재 세션의 프로젝트·훅 신뢰 적용 자체는 확정할 증거가 없어 직접 just check 및 커밋 훅의 결과를 보고한다.
