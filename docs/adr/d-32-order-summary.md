# D-32: 실행 토큰에서 유도하는 주문 요약과 고액 확인

상태: 초안. 기준 af70b46fc6c62a0d1a7b036c9977b3d2f3d47366. 사용자 후속 결정 반영.

## 생성 경계와 표시 사실

OrderSummary::from_execution은 ExecutionToken과 봉인된 표시 사실만 받는다. 코드·시장·방향·수량·지정가·금액·주문 키를 따로 받는 생성자는 없다. 필드는 architecture 4.4 순서 5의 종목명, 코드, 시장, 방향, 수량, 지정가, 예상 금액, 예상 수수료, 계좌 뒤 네 자리, 키 앞 여덟 자리 순서다. 구조 분해 테스트는 나머지 패턴 없이 열 필드를 전부 사용한다.

InstrumentNameFact는 T-09 종목 마스터 결과의 ResolvedOrderInstrument와 DisplayInstrumentName을 봉인한다. 이름은 빈 값과 256바이트 초과, 제어 문자를 거부한다. 이 상한은 자원 제한 초안이다. 요약은 코드·시장을 토큰과 대조한다. 조회 결과 생성자는 IO 사실을 인증하지 않으며 어댑터가 정확한 응답을 전달할 책임이 있다.

AccountDisplayFact는 계좌 조회 어댑터가 AccountId와 MaskedAccountNumber로 만든다. accountSeq는 계좌번호가 아니다. 계좌번호 전체를 보관하지 않고 기존 MaskedAccountNumber의 네 자리만 보관한다. Debug는 계좌 키를 숨긴다. 요약 생성 시 계좌 키가 토큰과 일치해야 한다. 실제 계좌번호 형식 확인은 ADR 0022에서 어댑터로 넘긴 T 작업 몫이다. D-32는 기존 마스킹 타입의 입력 정책을 변경하지 않는다.

EstimatedOrderFee는 Unknown 또는 Confirmed(ConfirmedOrderFee)다. ConfirmedOrderFee의 튜플 생성자는 crate 내부에서만 접근하며 주문 키와 Money를 함께 봉인한다. D-22 TradeCost가 공개 생성 경로를 정하고 그 유일한 생산자가 되어야 한다. 요약은 주문 키·주문 통화 일치와 음수 아님을 검사한다. 표시 계층은 Unknown을 '미확인'으로 보여야 하며 0원으로 표시하지 않는다. 확인된 금액도 예상 수수료이며 체결 확정값이 아니다. 고액 판정에는 수수료를 넣지 않는다.

A-17/O-03과 네이티브 확인 창은 이 요약만 사용한다. 표시 사실을 불러오거나 화면에서 주문 내용을 재조립하지 않는다. A-17은 기존 tasks에서 위험 설정 확인 역할도 있으므로 주문 요약 소비 지시는 주문 확인 경로에만 적용한다. 순서 7의 지연 후 재확인은 동일 토큰의 recheck로 하고, 요약을 재계산한 다른 주문으로 바꿔 보내지 않는다.

## 금액과 고액 판정

지정가 금액은 Price::checked_mul의 정확 곱이며 Overflow·PrecisionLoss는 요약 생성 오류, 주문 불가다. 시장가 수량 주문은 토큰 execution_observation의 가격으로 추정하며 Estimated를 보존한다. 미국 금액 지정 매수는 지정 금액 그대로다. 금액 주문의 수량과 시장가의 지정가는 닫힌 Absent 변형이다. 키가 여덟 자보다 짧으면 있는 만큼 표시한다.

매수·매도 모두 원화 금액 80,000,000원 이상이면 Required다. design 6.5는 별도 금액 한도를 두지 않으므로 1억 원 초과 자체를 거부하지 않는다. 매수가능금액·매도가능수량 검증은 O-01 책임이다.

달러 주문은 Buy(rate)·Mid(midRate)가 모두 USD/KRW 방향이고 토큰 checked_at 시각에 유효해야 한다. 시작 시각은 포함, 종료 시각은 제외한다. 둘 중 큰 값을 사용하며 같은 값이면 rate를 보존한다. 환산은 기존 D-04 정확 곱을 사용하되 경제적 환산 결과는 Approximate이고 선택 환율·출처·유효 시각이 남는다. 주문 금액·수량 산술로 환산액을 재사용하지 않는다.

환율 부재, 종류·방향 불일치, 미래·만료 시각, 환산 Overflow·PrecisionLoss는 Unavailable에 이유를 남기며 반드시 Required다. 작은 유효 환율 하나만으로 판정을 낮추지 않도록 두 환율 모두 검사한다. 판정은 Required/NotRequired의 닫힌 열거형이다.

## 검증과 후속

속성 테스트는 Config::with_cases(4096)를 명시하며 KRW 임계값과 USD 두 환율 선택·곱을 독립 정수 계산과 대조한다. 외부 crate 실험으로 맨 OrderIntent 입력, Summary 필드 위조, 확인된 수수료 위조를 거부하는지 확인한다. 새 의존성과 검사 끄기는 없다. 기존 master 테스트 본문은 변경하지 않는다.

OrderIntent Debug를 가리는 변경은 기존 master 테스트가 AccountId 전체 출력을 요구하여 적용하지 않는다. 요약과 AccountDisplayFact의 Debug에는 전체 계좌번호·계좌 키가 없다. 별도 사용자 결정으로 기존 테스트 계약을 바꿔야 OrderIntent Debug도 바꿀 수 있다.

미해결: 이름 상한 실측, T-09·계좌 조회 어댑터 연결, D-22의 확인된 수수료 공개 생성 경로, 토스의 실제 고액 판정 환율(S-01), 주문 시 환율 유효 기간의 어댑터 정책. 현재는 공급된 두 유효 구간의 교집합만 허용하고 부적합하면 고액 확인을 요구한다. 전송까지 지연되면 O-18이 token.recheck를 호출해야 하며 과거 checked_at의 환율 판정을 현재 유효성으로 간주하지 않는다.
