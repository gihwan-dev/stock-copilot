# ADR 초안: D-18 도메인 이벤트 닫힌 목록

- 상태: 초안
- 기준: `6a33bc1b1ec4ef9dd055b60f10c0f93aba350a20`
- 관련 작업: D-18, A-02, A-10, U-02, U-05, U-14

## 결정

architecture 3.5 표를 도메인 변경 알림의 닫힌 목록으로 삼는다. 코드로 확정할 수 있는 행과 식별자 정의 대기 행을 같은 표에서 구분한다. 정의 대기 행은 DomainEvent와 DomainEventKind에 포함하지 않는다. Unknown이나 자유 문자열 종류는 없다. kind는 와일드카드 없는 match로 변형과 일대일 대응한다.

페이로드는 기존 AccountId, InstrumentId, DisclosureKey, ProposalId, ClientOrderId와 닫힌 시장 종류 KnownMarket만 사용한다. 사건 내용과 계산 결과, 오류 본문, AI 글은 재조회한다. ClientOrderId는 앱이 낸 전송·결과 미확정 기록에만 쓰며 토스에서 따로 낸 주문의 ID로 간주하지 않는다. AccountId는 accountSeq 키이며 계좌번호가 아니다(ADR 0022). Debug에는 이 키들만 나온다.

단일 앱의 전역 설정·원칙·관심 목록·주문 스위치·토큰·자체 점검은 종류만 전한다. 이 자원들은 기존 문서가 앱 전체 하나 또는 단일 설정 화면으로 정의하므로 임의 ID를 발급하지 않는다. 종류로 무효화할 전역 조회가 정해진다. 토스 연결 오류는 계좌 선택 전에도 발생하므로 계좌 ID를 요구하지 않는다. 다중 연결·다중 설정 프로필을 추가하면 이 계약을 다시 정해야 한다.

기존 예시 18개 중 식별자가 있는 PortfolioSynced, PriceMoved, DisclosureArrived, ProposalCreated, ProposalExpired, OrderSubmitted, OrderOutcomeUnknown, BrokerAccessBlocked를 코드에 남긴다. 나머지 예시는 표에 정의 대기로 보존한다. NewsClusterGrew는 사건 묶음 갱신이며 design 5.3의 종목별 뉴스 급증과 같지 않다. 후자는 InstrumentId를 쓰는 NewsActivityIncreased로 추가한다. 브리핑 알림도 모든 AI 작업 완료와 구별해 BriefingFinished로 정의 대기에 둔다.

추가 근거는 architecture 표의 근거 열에 작업·요구사항별로 적는다. 보유·매수가능금액 실패를 분리해 일부 실패를 전체 성공으로 감추지 않는다. 일반 시세 갱신은 PriceUpdated로, AI 재추정 기준 충족은 PriceMoved로 구분한다. MarketSessionChanged는 KnownMarket이라는 닫힌 종류만 전달한다. 임계값·공시 등급 판단과 중복 실행 방지는 구독자가 저장 기록을 다시 읽고 한다.

ProposalApproved는 본인 확인을 통과했다는 감사 사실이지 재사용할 승인 토큰이 아니다. 승인 상태를 저장하거나 이벤트로 주문 전송을 예약하지 않는다. ProposalRejected는 사람의 거절이며 ProposalSkipped는 카드 넘기기다. 주문 스위치 변경은 ORD-5의 기존 제어만 알린다. 별도 킬 스위치 기능이나 앱의 정정·취소 전송 포트는 만들지 않는다. OrderCancelled/OrderModified는 외부 주문 변경 관측만 뜻한다.

모든 이벤트는 변경 저장 이후 발행한다. 이벤트를 놓쳐도 다음 조회로 회복한다. 구독자의 재조회·알림 필터·중복 방지·이벤트 버스 구현은 A-10 이후다. domain 기존 규칙에 맞춰 serde/specta 파생과 새 의존성은 없다. IPC 변환은 architecture 3.6에 따라 U-02에서 한다.

## 검사

문서와 domain 코드의 일치 검사를 검색했으나 선례를 찾지 못했다. domain 테스트에서 테스트 전용 std::fs 로더로 architecture 3.5만 읽고 정의 대기 여부가 아니오인 행의 이름 집합을 추출한다. DomainEvent·DomainEventKind의 실제 선언 이름 집합, 각 변형의 독립 기대값 목록과 비교한다. 제품 코드에는 파일 접근이 없다. kind 구현과 테스트의 Debug match 모두 전 변형을 열거하므로 변형 추가 누락은 컴파일 또는 집합 비교에서 실패한다.

## 미해결·후속

- 정의 대기인 평가 스냅샷, 뉴스 묶음, 수집 실행, 과제·제출·판정, 신호, 증권사 주문, AI 작업·한도 상태, 브리핑, 수집 소스의 영속 식별자와 조회 계약을 E/S-18, AI/J/N, A-20/T-17에서 확정해야 한다. D-03에는 해당 타입이 없다. 계좌+날짜 등을 임의로 스냅샷 키라고 확정하지 않는다.
- D-24의 SignalTaskKey는 EstimateTask 전체를 담는다. 식별자 전용 이벤트로 전송하지 않는다. CalibrationEstimateId도 보정 이력용이며 제출 ID·판정 조회 키를 대신한다고 가정하지 않는다.
- 전역 변경의 세부 무효화 범위와 중복 알림 방지는 A-10/U-05에서 정한다. 현재 이벤트는 영속 이벤트 로그나 재생 계약을 제공하지 않는다. 전역 설정 알림은 보수적으로 관련 조회 전체를 무효화한다.
- 주문 전송 실패·승인 거절의 원인 구분은 저장된 감사 기록을 조회한다. UserPresence::Unavailable을 사람의 거절로 발행하지 않는다. 자체 점검과 실패 기록으로 표시한다.
- 프로젝트 설정의 hooks=true와 lefthook 설치는 확인하지만 이번 세션의 프로젝트 신뢰·Codex 훅 적용 자체를 입증하지 못한다. 실제 직접 검사와 Git 커밋 훅을 증거로 사용한다. 실제 키·계좌 데이터는 쓰지 않는다.
