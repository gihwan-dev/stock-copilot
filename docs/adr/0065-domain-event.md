# ADR 0065: D-18 DomainEvent

- 상태: 확정 (2026-11-14)
- 기준: 적대적 리뷰 대상 `4a10b75ee253cebf4f44160d95ae9bb132224734`, 먼저 병합한 master `9b7c74285cca8ac83d914e609962695aad58ad8b`
- 관련 작업: D-18, A-10, U-02, U-05, N-12, O-04, O-19

## 결정

메인 리뷰 결정에 따라 이벤트의 ID를 개별 사건 ID로 한정하지 않는다. 종목 키로 현재 신호·추정·판정·뉴스 묶음 목록을, 계좌 키로 평가·주문 목록을 다시 읽는다. 이 앱의 주문 전송 기록은 기존 ClientOrderId로 다시 읽는다. 페이로드는 기존 식별자와 닫힌 종류뿐이며 원문·금액·수량·가격·오류 본문·AI 글이 없다. event kind는 와일드카드 없는 match로 일대일 대응한다. DomainEventKind에 PartialOrd·Ord를 제공해 A-10의 BTreeMap 구독 등록표에 쓸 수 있다.

InformationSource는 Web·News(NewsSource)·Disclosure(DisclosureSource)의 기존 닫힌 종류이므로 CollectionFinished로 해당 종류의 수집 결과 목록을 다시 읽는다. 개별 실행 완료 신호가 아니므로 AI 실행 후속 단계의 인증·지정에 사용하지 않는다. FxPair는 기존 닫힌 통화쌍 종류이며 FxRateUpdated의 구독자는 그 쌍의 모든 환율 종류를 다시 읽는다. NewsClusterGrew도 design 5.4의 같은 종목 묶음 목록을 다시 읽는다.

토스 연결은 한 상태 변경에 BrokerConnectionChanged 하나를 낸다. 차단·만료·사용자 조치 필요·차단 해제·재발급 성공을 모두 포함한다. 같은 사건에 SelfCheckChanged를 함께 내지 않으며 후자는 토스 연결 밖의 자체 점검 항목에만 쓴다. 동기화 결과 저장은 PortfolioSyncRecorded·TransactionsSyncRecorded로 성공·일부 실패·실패·접근 차단 모두 하나씩 알리고 결과 조회가 그 차이를 보존한다.

## 옛 이름과 새 이름

| 옛 이름 | 새 이름 | 의미 |
|---|---|---|
| PortfolioSynced, HoldingsSyncFailed, BuyingPowerSyncFailed | PortfolioSyncRecorded | 계좌 동기화 결과 저장 단위 |
| TransactionsSynced | TransactionsSyncRecorded | 거래내역 대조 결과 저장 단위 |
| BrokerAccessBlocked, BrokerTokenExpired, BrokerUserActionRequired | BrokerConnectionChanged | 토스 연결 상태 변경과 회복 |
| AiQuotaExhausted | AiBudgetChanged | 예약 비율 도달·한도 초과·재개를 전역 예산 변경으로 통합 |

SignalRaised·SignalCleared·EstimateSubmitted·EstimateResolved는 InstrumentId, ValuationSnapshotTaken은 AccountId, OrderRejected·OrderOutcomeResolved는 ClientOrderId, OrderFilled·OrderCancelled·OrderModified는 AccountId로 확정한다. 뒤의 세 이벤트는 외부 앱 주문도 포함한다. OrderOutcomeResolved는 O-19의 대조 확정 또는 확인 불가 확정 모두다. 확인 불가를 성공으로 바꾸거나 새 주문 차단을 푸는 의미가 아니다.

계좌 선택 변경은 design 2.5·8장과 U-22에 따라 AccountSelectionChanged로 모든 계좌 조회를 무효화한다. StoredDataChanged는 U-15 전체 삭제와 S-11 복구 절차의 저장소 교체 완료에만 적용하며 새 복원 화면이나 사용자 절차를 추가하지 않는다. 단순 백업 생성에는 내지 않는다. SettingsChanged 구독자는 설정을 전부 다시 읽는다. AiBudgetChanged는 design 4.3·AI-2·REL-4의 전역 예산 조회를 무효화하며 주 화면 알림을 만들지 않는다.

공시 원시 도착 DisclosureArrived는 UI만 구독한다. 긴급·높음·보유 여부·정정 한 단계 하향은 N-12 한 곳에서 판정하고 기준 충족을 저장한 뒤 DisclosureTriggerRecorded를 낸다. 구독자가 등급을 다시 계산하지 않고 판정 기록의 스케줄러·알림 대상을 따른다. 알림은 기록된 긴급만이다. 가격·뉴스와 같은 판정/발행 구조다.

ProposalApproved는 감사 사실이지 재사용할 승인 증거가 아니다. 비동기 구독으로 주문을 전송하거나 승인 상태를 저장하지 않는다. UserPresence::Unavailable을 사람의 거절로 기록하지 않는다. 주문 스위치는 기존 ORD-5만 반영한다.

## UI 경계

domain 이벤트에는 AccountId(accountSeq)가 있지만 UI DTO에서는 제외한다. UI는 단일 선택 계좌를 다시 읽는다. accountSeq를 문자열로 바꿔 보내는 것도 금지한다. UI DTO에는 종류와 계좌 키가 아닌 ID의 문자열 표현만 허용한다. 이는 ADR 0022의 계좌 키를 화면이나 AI에 보낼 필요가 없다는 경계, ADR 0016의 i64 내보내기 거부를 따른다. 닫힌 종류의 DTO 표현·타입 생성·직렬화는 U-02에 인계하며 domain에는 serde/specta를 추가하지 않는다.

## 검사

테스트 전용 파일 로더가 실제 DomainEvent 선언의 변형별 필드 타입(전역은 없음)을 architecture 3.5의 페이로드 열과 대조한다. DomainEventKind의 이름·무페이로드와 테스트 기대값 목록도 대조한다. 전체 행 수와 이름 집합 크기를 비교해 중복 행을 막고 정의 대기 이름·소유 작업 집합을 고정한다. 리뷰의 다섯 생존 변형을 영속 회귀로 둔다: 확정 행 중복, 문서 페이로드 변경, 정의 대기 행 삭제, 전역 변형에 ID 부여, 코드 필드 타입 변경. 마지막은 테스트 인자를 맞추어도 코드 선언과 문서가 달라 실패한다.

## 정의 대기와 인계

| 정의 대기 | 소유 작업 | 키 재조회를 쓰지 않은 이유 | tasks.md에 추가할 완료 기준 문장 |
|---|---|---|---|
| AiJobFinished | AI-17 | AI-7의 동시 채팅·예약 실행을 하나의 현재 작업 키로 구분할 수 없다. 후속 단계가 어느 실행의 완료인지 지정할 실행 ID와 조회 계약이 필요하다 | 실행 ID 타입과 종료 기록 조회 계약을 확정하고 AiJobFinished의 ID 페이로드·발행 조건·구독자를 architecture 3.5와 코드에 반영한다. |
| BriefingFinished | J-03 | 현재·지난 브리핑이 함께 남는다. 최신 카드만 읽으면 완료 알림의 기록을 특정하지 못한다 | 브리핑 ID와 완료 기록 조회 계약을 확정하고 BriefingFinished의 UI 갱신·UX-3 알림을 architecture 3.5와 코드에 반영한다. |
| SourceStatusChanged | N-01 | InformationSource는 웹·뉴스·공시만 구분하며 재무·미 재무부·FRED까지 포함한 전체 소스 키가 아니다 | 전체 소스를 식별하는 닫힌 종류와 상태 조회 계약을 확정하고 SourceStatusChanged의 페이로드·고급 상태 갱신을 architecture 3.5와 코드에 반영한다. |

## 미해결·후속

- ProposalExpired의 시간·가격 범위 만료 조건은 design 6.5에 있으나 발행 주체가 없다. 메인 지시대로 예약 점검이 만료를 기록할 때 발행하는 보수적 초안이다. O-04는 예약 점검의 등록·중복 방지·만료 기록/발행을 구현하고 승인 요청 때도 최신 시각·가격을 다시 검증한다.
- A-10은 기록 저장 이후 발행, 같은 연결 사건의 중복 방지, 전역·키별 재조회 계약을 연결한다. U-02는 계좌 키를 제외한 DTO 변환을, U-05는 선택 계좌·전체 저장 데이터 변경의 쿼리 무효화를 검증한다. DTO 자체는 이 작업에서 구현하지 않는다.
- 이벤트를 놓쳐도 다음 조회로 회복한다. 현재 타입은 영속 이벤트 로그·정확히 한 번 전달·실행 인증 계약이 아니다. 전역 자원을 다중 연결·다중 설정으로 바꾸면 키 계약을 다시 정해야 한다.
- 프로젝트 hooks=true 설정과 실제 세션의 프로젝트·훅 신뢰 적용 자체를 구별한다. 직접 검사·Git 훅 결과를 검증 증거로 사용한다. 실제 키·계좌 데이터·새 의존성·검사 끄기는 없다.

## 그 밖의 tasks.md 인계 문장

| 작업 행 | 추가할 완료 기준 문장 |
|---|---|
| A-06, A-07 | 성공·일부 실패·실패·접근 차단 결과를 저장하면 각각 PortfolioSyncRecorded(AccountId)·TransactionsSyncRecorded(AccountId)를 한 번 내고 구독자는 저장 결과를 다시 읽는다. |
| A-12, A-13, A-15, A-16 | 추정 제출·신호 발생/해제·추정 판정은 InstrumentId로, 일별 평가 기록은 AccountId로 architecture 3.5의 해당 이벤트를 내고 키에 속한 현재 상태·기록 목록을 다시 읽는다. |
| A-10, T-02 | 토스 연결의 차단·만료·사용자 조치·회복 한 사건에는 BrokerConnectionChanged 하나만 내며 SelfCheckChanged와 중복 발행하지 않는다. 구독 등록표는 DomainEventKind의 순서 비교를 쓴다. |
| T-17, O-19 | 계좌 주문 관측 변화는 외부 앱 주문을 포함해 OrderFilled/OrderCancelled/OrderModified(AccountId)로, 이 앱 주문의 거절과 미확정 결과의 확정·확인 불가 확정은 OrderRejected/OrderOutcomeResolved(ClientOrderId)로 알린다. |
| N-09, N-12 | 종목별 묶음 증가는 NewsClusterGrew(InstrumentId)로 알린다. 공시 원시 도착은 UI만 갱신하고 N-12가 트리거 판정을 저장한 뒤 DisclosureTriggerRecorded(DisclosureKey)를 내며 구독자가 등급을 다시 계산하지 않는다. |
| AI-14, AI-06 | 수집 결과 저장은 CollectionFinished(InformationSource)로 소스 종류별 재조회를 알리고, 예약 비율 도달·구독 한도 초과·재개는 AiBudgetChanged 하나로 전역 예산을 다시 읽는다. |
| U-22, U-15, S-11, U-05 | 계좌 선택 변경은 AccountSelectionChanged로 모든 계좌 조회를, 전체 삭제·백업 복구의 저장소 교체는 StoredDataChanged로 저장 데이터 조회 전체를 무효화한다. SettingsChanged는 설정을 전부 다시 읽는다. |
| T-08, S-04 | 환율 저장 뒤 FxRateUpdated(FxPair)를 내고 해당 통화쌍의 모든 환율 종류와 관련 평가 조회를 무효화한다. |
| U-02 | UI 이벤트 DTO에는 종류와 계좌 키가 아닌 ID의 문자열 표현만 싣고 AccountId/accountSeq는 문자열 변환까지 포함해 제외하는 테스트를 둔다. |
| O-04 | 예약 점검이 시간·가격 범위 만료를 기록할 때 ProposalExpired(ProposalId)를 한 번 내며 승인 요청 때도 만료·가격을 다시 검증한다. |
