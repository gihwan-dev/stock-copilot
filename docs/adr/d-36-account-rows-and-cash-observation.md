# D-36 계좌 행과 현금 관측

- 상태: 초안
- 기준: e31ab9d0ce070a9911c9e6402c9651184081e2c3
- 근거: ADR 0022·0038·0043·0045·0047·0048·0055, architecture 3.3, design 2.5·UX-4, 로컬 OpenAPI 1.2.28.

## 계좌 사실과 선택

Account 스키마의 필수 필드는 accountSeq(int64), accountNo(string), accountType이다. 추가 필드는 없다. 번호는 예시 12345678901만 있고 길이·패턴 제약은 없다. ADR 0022의 임시 ASCII 숫자 8~20자리 검증과 뒤 네 자리 저장을 재사용한다. 전체 번호를 새 타입에 보관하지 않는다. 조회 시각은 서버 응답 필드가 아니라 어댑터가 공급한다.

| 스키마 값 | BrokerAccountKind |
|---|---|
| BROKERAGE | Brokerage |
| OVERSEAS_DERIVATIVES | OverseasDerivatives |
| PENSION_SAVINGS | PensionSavings |
| RESHORING_INVESTMENT | ReshoringInvestment |
| 그 밖의 원문 | Unknown(봉인된 UnknownBrokerAccountKind) |

BROKERAGE는 스키마가 명시한 종합매매다. 현재 accounts는 이것만 반환하지만 향후 다른 유형과 미지 값을 버리지 않는다. BrokerAccountTypeCode는 문자열에 별도 형식 제약을 발명하지 않고 원문을 그대로 보존하는 경계 newtype이다. 알려진 값을 Unknown으로 직접 만드는 경로는 없다.

BrokerAccountRow는 키·유형·가린 번호·조회 시각을 비공개 필드에 가진다. from_broker_account_result는 T-05의 응답 한 행 변환 경계다. IO 진위를 타입만으로 인증하지 않는다. 공개 접근자와 display_fact는 읽기만 제공한다. display_fact는 추가 키·번호를 받지 않아 행 이후 다른 번호를 붙일 수 없다. Debug도 이미 잘린 번호만 가진다.

select_brokerage_account는 ADR 0055의 서버 순서 보존과 design의 첫 종합매매 계좌 규칙대로 응답 순서의 첫 Brokerage를 빌린다. 키 정렬·기본값·Unknown 선택은 없다. 결과는 Selected / NoBrokerage / Empty다.

기존 AccountLookupDisplayRow::from_broker_account_result는 application/tests/ports.rs를 포함한 master 테스트가 crate 밖에서 사용한다. 기존 테스트를 바꾸지 않기 위해 남긴다. AccountDisplayFact::from_account_lookup_result도 그대로다. **A-20·T-05가 옮긴 뒤 닫는다.** 따라서 이번 변경은 새 권장 경로에서 행 결합을 보장하지만 모든 공개 경로가 행만 받는다는 전역 봉인을 완성하지 않는다. A-20은 AccountObservation의 독립 키·표시 조합을 BrokerAccountRow로 교체한다. T-05는 응답 한 행에서만 이 행을 만들고 U-22는 선택 함수를 쓴다. O-04·O-07은 선택된 행의 display_fact를 받는다.

## 조회 실패한 현금

CashObservation은 Observed(BuyingPower) / Failed { currency, reason, observed_at }다. 실패에는 금액이 없다. 이유는 Unavailable / AccessDenied / InvalidResponse의 닫힌 종류다. 구체적인 PortError는 A-06에서 이 종류와 별도로 보존하며 서버 본문을 domain 오류에 넣지 않는다. 실패 observed_at은 실패 관측 시각이다.

새 자유 함수 portfolio_from_cash_observations는 Portfolio를 반환한다. 기존 Portfolio::new·BuyingPower::new와 모든 기존 impl·테스트 본문은 변경하지 않았다. portfolio.rs에는 PortfolioError::CashLookupFailed와 비공개 변환 자유 함수만 추가했다. 봉인된 BuyingPower 내부 필드에 실패를 넣기 위한 최소 연결이다. 실패는 money의 Err이며 0원·음수 가짜 값이 아니다. 기존 평가기가 해당 현금만 제외하고 Total::Incomplete를 유지한다. 실패 이유·통화·시각은 Portfolio와 평가 결과 buying_power 접근자로 읽고 평가 오류 목록에도 보존된다. 성공·음수 관측 오류는 그대로다.

보유 평가·원금·손익·통화별 합은 현금 실패와 독립적이다. 남은 통화 현금만 불완전 총자산의 부분합에 더한다. buying-power는 currency와 cashBuyingPower 필수이며 현금 기반 주문 가능액이다. 예수금이나 현금 원장의 대조 잔액으로 취급하지 않는다. 보유/현금 독립 조회와 조립은 A-06 책임, 표시 시각 정합은 A-08 책임이다.

D-06 총자산 비중과 D-12 원칙은 기존 오류 경로로 차단하고 보유 구성비는 유지한다. 기존 AllocationError::NegativeCash가 모든 buying_power.money 오류에 쓰이므로 조회 실패에도 이 이름이 나온다. 이번에는 기존 allocation.rs를 고치지 않는다. 현금이 음수였다고 사용자에게 번역해서는 안 되며 A-08은 CashLookupFailed 원인을 확인해야 한다. 오류명 세분화는 후속이다.

## 검증과 미해결

새 테스트는 구현 전 컴파일 실패를 확인했다. 독립 기대값의 4,096회 속성 검사로 선택 결정성과 응답 순서, 한 통화 실패 시 다른 현금·보유 평가 보존과 실패 금액의 완전 합계 제외를 검증한다. D-06 구성비 유지와 D-12 현재 검사·알려진 후보의 매수 차단을 확인한다.

외부 crate 컴파일 실험: row.display_fact() 성공, 다른 suffix/AccountId 인자 전달 실패(E0061), 비공개 suffix 덮기 실패(E0616), Unknown 생성 실패(E0603). 기존 D-32 공개 생성자는 성공해 호환성 예외를 확인했다. 최초 응답 경계에서 임의 키·번호를 함께 공급하는 것까지 막는 IO 인증은 아니다.

로컬 master에는 H-43 구현과 ADR 0054가 없어서 자동 변이 대신 새 분기의 손 변형 14개 이상을 실행한다. 기존 프로세스의 임시 폴더는 건드리지 않는다. 실제 손 변형 18개 중 16개를 테스트가 검출했고 2개는 컴파일 단계에서 거부되었다. 생존 0이며 모두 복원했다.

D-24 SignalDecision은 기준 코드에 없어 직접 매수 신호 연동 테스트는 미실행이다. 현재 D-12 매수 차단을 검증했으며 A-13·D-24는 이 결과를 우회해 신호를 내지 않도록 연결 후 확인해야 한다. 실패 이유의 PortError 상세 대응, 계좌 번호 실측 형식, 어댑터 생성 호출 위치 강제, 기존 공개 생성자 폐쇄는 후속이다. 제품 규칙이나 사용자 절차는 추가하지 않았다.

새 의존성·검사 끄기는 없다. 프로젝트 설정 hooks=true와 Git 훅 설치는 확인했지만 이 세션의 프로젝트·훅 신뢰 적용은 증명하지 못했다. 직접 just check와 커밋 훅 실행 결과를 근거로 한다. master 이동·푸시·PR·실제 계좌·키 접근은 없다.
