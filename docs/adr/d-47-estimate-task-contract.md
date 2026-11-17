# D-47: 추정 과제 발급·배정·소비와 제출 기록

- 상태: 초안
- 기준: a8d20344ebdcf8d1044189a97cce531b8b41c0af
- 근거: ACC-9, AI-6, ADR 0027·0046·0051·0071, A-12 정의 대기 분석

## 결정

하나의 저장 원장을 EstimateAssignments로 표현한다. issue만 IssuedEstimateTaskId를 만든다. ID의 외부 표현은 기존 CalibrationEstimateId이며 lookup은 그 외부 값을 원장과 대조한다. 외부 값 파싱은 발급 사실을 만들지 않는다. issue는 코드가 선택한 CostEligibleTask와 실행만 받는다. ID의 유일성은 원장 전체에서 검사한다. 저장소는 원장 스냅샷의 이전 버전과 다음 버전을 비교해 원자적으로 교체해야 한다. 순수 함수의 호출만으로 동시성이나 영속성이 보장되지는 않는다.

실행과 추론 강도는 각각 Run, Effort 제네릭으로 둔다. 이미 application에 있는 AiRunId와 ReasoningEffort를 그대로 사용하기 위한 것으로 domain에서 같은 타입을 재정의하거나 application에 역의존하지 않는다. 실제 응답 모델은 SignalSubmission의 CalibrationKey.model에 보존한다. 요청 모델이나 라우팅 사건으로 대신 채우지 않는 연결 책임은 A-12·AI-04에 있다.

소비는 이전 원장을 바꾸지 않고 새 원장을 반환한다. 배치 크기가 배정 수를 넘으면 AssignmentExceeded, 그 외 미지 값 UnknownTask, 다른 실행 WrongRun, 같은 과제의 재소비 AlreadyConsumed다. 배치 중간의 실패도 원래 원장을 소비하지 않는다. 빈 배치는 같은 상태다. 배정은 발급과 함께 고정하며 재배정 API는 없다.

제출 기록은 기존 SignalSubmission을 통째로 보존하며 근거와 반론은 UntrustedText다. 발급된 과제와 제출 과제의 값이 다르면 거부한다. 기록 생성은 소비나 저장 성공을 뜻하지 않으며 거래 조건도 받지 않는다. 도구 참조만 있거나 출처가 없으면 Missing이다. 이는 한국어 UI의 "출처 없음"에 대응한다. 실제 표시는 E-06의 messages.ts가 담당한다. Web·News·Disclosure는 외부 출처이며 모든 원본 참조를 보존한다.

프롬프트 해시는 64자리 ASCII 16진수로 제한한다. 해시 계산은 경계 책임이다. CLI 버전과 프롬프트 버전은 기존 ModelIdentityPart의 검증된 불투명 문자열을 재사용한다. 증거 묶음은 EvidenceBundleId로 항목 ID와 구분한다. 공개 필드를 가진 입력 묶음은 기록 생성 때 소유권을 넘기므로 생성 뒤 기록을 수정하지 못한다.

## 기존 타입 대응

| 사실 | 기존 타입 / 결합 |
|---|---|
| 사건·기준가·시각·변동성 | CostEligibleTask → EstimateTask (D-21·D-22) |
| 원래 확률·출처·제출 시각 | SignalSubmission (AiProbability, Evidence, Timestamp) |
| 실제 모델·기간 | SignalSubmission → CalibrationKey → CalibrationModel / Horizon |
| 외부 제출 식별 표현 | CalibrationEstimateId |
| 실행 ID / 추론 강도 | application::AiRunId / ReasoningEffort를 Run / Effort에 대입 |
| 근거·반론 | UntrustedText |
| 프롬프트 ID | ScorecardPromptId |
| CLI·프롬프트 버전 | ModelIdentityPart |
| 프롬프트 해시 / 증거 묶음 ID | PromptHash / EvidenceBundleId (신규 의미) |

## A-12의 아홉 기준

| 기준 | 계약 준비 | 남은 실행 책임 |
|---|---|---|
| 미지 task_id 거부 | lookup / consume | A-12가 저장 원장을 읽고 호출 |
| 다른 실행 거부 | WrongRun | 신뢰한 실행 컨텍스트 전달 |
| 배정 수 이하 | 배정 집합·한 번 소비 | 원자적 상태 교체 |
| 중복·동시 중복 한 번 | AlreadyConsumed·순수 전이 | S-18의 비교 교체와 기록 추가 단일 트랜잭션 |
| 출처 없음 | source_status Missing | E-06 messages.ts 표시 |
| 모든 제출 기록 | EstimateSubmissionRecord | S-18 누락 없는 저장, A-15 판정 연결 |
| AI-6 | 완전한 필수 필드 | 실제 관측 모델·CLI·프롬프트·묶음 연결 |
| 기록 뒤 이벤트 한 번 | 기존 DomainEvent 재사용 가능 | A-12 저장 성공 뒤 발행 |
| 저장 실패 이벤트 없음 | 순수 전이로 이전 상태 보존 가능 | A-12 실패 경로·동시성 통합 테스트 |

## 인계

- A-12: Run=AiRunId, Effort=ReasoningEffort로 연결한다. 실제 응답 모델을 CalibrationKey에 넣는다. 기록 생성·소비·저장을 함께 처리하고 성공 뒤 InstrumentId 이벤트를 한 번 발행한다.
- S-18: 스냅샷 비교 교체와 완전한 기록 추가를 단일 트랜잭션으로 적용한다. 실패는 소비하지 않는다. 원장 전체 ID 유일성과 재시작 복원을 보장한다. 역직렬화를 domain에 추가하지 않는다.
- E-01·E-02: 발급 함수는 코드 경로만 호출한다. AI 입력은 lookup과 소비의 외부 키로만 사용한다.
- E-06: Missing을 "출처 없음"으로 표시한다. A-13·A-15: 기록의 기존 SignalSubmission·EstimateTask를 사용한다.

## 검증

프로젝트 신뢰·훅 정의 신뢰·현재 세션 강제 적용은 확인할 수 없었다. 지시문의 계속 진행 예외를 적용했다. frozen install·lefthook install 성공. 실제 계좌·키·API 접근 없음. 새 의존성·기존 테스트 수정 없음. 테스트 우선 작성 순서는 지키지 못했으며 실행 근거를 별도로 기록한다.

## 손 변형

실행 후 표를 추가한다.
