# A-19 자체 점검 판정

- 상태: 초안

## 결정

기준 HEAD와 master는 54668ffd6a0cd0e49338067a4b9ef54ded752fc5다. design 2.6, P-7, UX-5, REL-5와 ADR 0063·0065·0071·0073·0074를 따른다.

순수 함수 evaluate_self_check는 design 순서의 관측 8개를 받아 최초 사용자 조치 문제 하나 또는 없음, 순서가 고정된 전체 점검 기록 8개, 별도의 소스별 뉴스·공시 기록을 반환한다. 화면 문장, IP 문자열, 키, 원문 오류를 만들거나 보관하지 않는다. 관측은 닫힌 열거형이고 포트 오류는 A-05의 ErrorClass를 받는다. 뉴스 오류도 InformationError로 받는다. 대기 재개 시각은 ResetTime을 원값 그대로 기록한다. 띠에 오르는 조치는 즉시 할 일이며 재개 시각은 NotProvided다.

판정은 IO 없는 순수 규칙이나 금융 계산은 아니다. architecture 3.2에서 domain은 application을 의존할 수 없고, 3.4의 domain 계산은 금융 값·확률·수량·가격 규칙이다. 이미 application이 소유한 ErrorClass, ResetTime, IsolationFailure, FeedSource를 재정의하거나 domain으로 옮기지 않고 application의 운영 상태 유스케이스 규칙으로 둔다. 외부 의존성·개발 의존성·허용 목록 추가는 없다.

배열은 길이 8로 누락·중복 슬롯을 막으며 반환 기록에는 닫힌 점검 종류를 붙인다. 행과 맞지 않는 조치 관측은 원값을 기록하되 띠에 올리지 않는다. 조치가 해당 행과 맞는 경우만 허용한다. CheckAccount와 CheckPermissions는 design 2.6에서 사용자 행동 문장이 확정되지 않았으므로 새 행이나 안내 절차를 발명하지 않고 기록한다. LoginUserNotFound의 CheckCredentials는 1번 키 확인으로 대응한다. 주문 사전 요건은 위험 안내 확인, 거래소 연동, 달러 매수 가능 금액 마련의 세 닫힌 종류로 받는다. T-04의 조회 오류 Bug를 주문 오류로 재해석하지 않는다.

## 점검과 띠 기준

모든 행에서 정상·대기·버그·데이터 없음은 띠 대상이 아니다. design 2.6의 “사용자가 할 일이 있는 문제만”에 따라 버그에는 사용자 조치를 발명하지 않는다. NoData는 architecture 3.8과 ADR 0063의 빈 조회 결과 원칙을 따른다.

| 순서 | 점검 | 띠 대상 | design 2.6 근거 문장과 조치 |
|---|---|---|---|
| 1 | 토스 키 | 해당 사용자 조치만 | “키를 다시 복사해 붙여 넣으세요.” → ReplaceTossKey |
| 2 | 허용 IP | 해당 사용자 조치만 | “WTS의 허용 IP 관리에 추가하세요.” → RegisterIp |
| 3 | 같은 키의 중복 사용 | 해당 사용자 조치만 | “같은 토스 키를 쓰는 다른 프로그램을 끄면 다시 연결됩니다.” → StopOtherProgram |
| 4 | Claude Code 설치 | 해당 사용자 조치만 | “Claude Code를 설치하면 AI 기능이 켜집니다.” → InstallClaudeCode |
| 5 | Claude Code 로그인 | 해당 사용자 조치만 | “Claude Code에 로그인하면 AI 기능이 켜집니다.” → LoginClaudeCode |
| 6 | AI 실행 환경 | 없음 | “주 화면에 띄우지 않는다. 고급 > 상태에 사유를 남긴다” |
| 7 | AI 사용 한도 | 없음 | “주 화면에 띄우지 않는다. 고급 > 상태에 재개 시각을 남긴다” |
| 8 | 주문 사전 요건 | 해당 사용자 조치만 | “오류별 한 문장”, “위험 안내를 먼저 확인해야 주문할 수 있습니다.” → ReviewRiskNotice / IntegrateExchange / ProvideDollarBuyingPower |

뉴스·공시 실패는 “고급 > 상태에만 남긴다.” 선택 소스 자격 오류는 ADR 0071대로 사용자 조치 분류지만 띠 대상이 아니다. 각 관측과 소스 기록을 그대로 돌려주어 상태 화면이 사유를 표시할 수 있다.

가림은 앞 순서의 띠 대상 하나가 뒤의 모든 문제를 가린다. 가려진 기록은 삭제하지 않는다. 6·7번과 뉴스 실패는 뒤의 8번 조치를 가리지도 않는다.

## 이벤트

self_check_difference는 전체 결과의 동등성을 비교하여 Same/Changed를 반환한다. self_check_change는 발행 판단만 한다. BrokerConnection 원인은 회복을 포함해 항상 Unchanged(발행 안 함)다. OtherCheck는 토스 밖 4~8번 기록이 바뀐 경우에만 Publish다. 토스 관측만 바뀌어 띠가 달라져도 SelfCheckChanged를 겹쳐 내지 않는다. 띠에 가려진 4~8번 기록의 변화도 Publish다. 뉴스만 변한 경우는 N-01의 소스 이벤트 책임이다. 두 함수는 실제 이벤트를 내거나 저장하지 않는다.

## 검사

새 테스트만 추가했다. 최초 실행은 미구현 공개 타입·함수 부재로 컴파일 실패했고, 구현 뒤 5개 테스트가 통과했다. 8개 점검 × 모든 관측 변형·조치 표, 256개 활성 문제 부분집합, 4,096회 임의 관측 조합을 독립 exhaustive 기대 함수와 대조한다. 6·7번의 임의 조치도 차단하고 뉴스 실패가 있는 조합도 확인한다. 숨은 기록, 서로 다른 시각·관측·뉴스 필드 보존과 연결 사건 억제를 검사한다. application Clippy, source-check, raw-types를 실행한다. 전체 검사는 최종 정상 커밋 훅의 just check 결과를 따른다. 자동 변이 검사는 지시대로 실행하지 않는다.

프로젝트·세션 훅 신뢰를 인증하는 도구는 없으므로 신뢰 적용을 추정하지 않는다. pnpm frozen install 및 lefthook install을 수행했으며 정상 Git 훅의 실제 실행을 증거로 삼는다.

## 인계

- T-02: 토스 키·IP·중복 사용 관측을 최종 A-05 분류에서 만들고 한 연결 사건을 저장한 뒤 BrokerConnectionChanged만 발행한다. 같은 사건의 자체 점검 호출에는 BrokerConnection 원인을 넘긴다.
- AI-13·AI-16·AI-05: 설치·로그인·실행 환경과 사용 한도 관측을 만든다. 재개 시각·격리 실패의 닫힌 사유를 보존한다.
- T-16·O-01·O-09: 주문 거절을 실제 주문 경로에서 세 사전 요건 조치로 정규화한다. 조회 매퍼의 Bug를 사용자 조치로 바꾸지 않는다.
- U-21·U-27: 닫힌 조치의 한국어 문장은 messages.ts에서 만든다. 띠는 0~1개, 상태 화면은 전체 기록과 재개 시각을 표시한다. 최신 관측 수집·저장·주기 실행·저장 뒤 실제 발행은 조립 유스케이스에서 맡는다.
- N-01: 소스별 마지막 성공 시각과 상태 저장·SourceStatusChanged 계약은 별도 구현한다. 이번 결과의 뉴스 기록은 전달받은 현재 상태만 보존한다.
- A-05·U-21: CheckAccount·CheckPermissions의 구체 안내는 메인이 제품 문장을 확정할 때 연결한다. 현재는 기록만 남긴다.
- S-11: 백업·복구와 StoredDataChanged는 자체 점검 이벤트로 대체하지 않는다.

## 손 변형 결과

mktemp -d의 별도 저장소 복사본에서 아래 25개를 각각 컴파일한 뒤 실제 테스트를 실행했다. 25개 모두 검출, 생존 0, 컴파일 불가 0이다. 실험 복사본에만 cargo-mutants 방식의 cap-lints를 사용했다. 변형 전과 원복 후 같은 테스트가 통과했다. 원본 소스는 변형하지 않았다. 상세 실행 로그는 작업 사본 target/A-19에 남긴다.

| 번호 | 바꾼 것 | 컴파일 | 결과 | 잡은 테스트 예시 |
|---|---|---|---|---|
| 1 | 관측 필드 교체 TossKey, key | 성공 | 검출 | `tests::self_check_change_compares_hidden_status_and_suppresses_broker_events` |
| 2 | 관측 필드 교체 AllowedIp, ip | 성공 | 검출 | `tests::self_check_change_compares_hidden_status_and_suppresses_broker_events` |
| 3 | 관측 필드 교체 DuplicateKeyUse, duplicate | 성공 | 검출 | `tests::self_check_masked_records_and_news_are_preserved` |
| 4 | 관측 필드 교체 ClaudeCodeInstallation, installation | 성공 | 검출 | `tests::self_check_masked_records_and_news_are_preserved` |
| 5 | 관측 필드 교체 ClaudeCodeLogin, login | 성공 | 검출 | `tests::self_check_masked_records_and_news_are_preserved` |
| 6 | 관측 필드 교체 AiEnvironment, environment | 성공 | 검출 | `tests::self_check_change_compares_hidden_status_and_suppresses_broker_events` |
| 7 | 관측 필드 교체 AiUsageLimit, usage | 성공 | 검출 | `tests::self_check_decision_table_and_order` |
| 8 | 관측 필드 교체 OrderPrerequisites, order | 성공 | 검출 | `tests::self_check_every_actionable_subset_picks_first` |
| 9 | 마지막 문제 우선 | 성공 | 검출 | `tests::self_check_masked_records_and_news_are_preserved` |
| 10 | 띠 전체 누락 | 성공 | 검출 | `tests::self_check_decision_table_and_order` |
| 11 | 뉴스 기록 유실 | 성공 | 검출 | `tests::self_check_masked_records_and_news_are_preserved` |
| 12 | 키 조치 누락 | 성공 | 검출 | `tests::self_check_decision_table_and_order` |
| 13 | IP 조치 누락 | 성공 | 검출 | `tests::self_check_decision_table_and_order` |
| 14 | 중복 키 조치 누락 | 성공 | 검출 | `tests::self_check_decision_table_and_order` |
| 15 | 설치 | 성공 | 검출 | `tests::self_check_every_actionable_subset_picks_first` |
| 16 | 로그인 | 성공 | 검출 | `tests::self_check_every_actionable_subset_picks_first` |
| 17 | 환경 노출 | 성공 | 검출 | `tests::self_check_every_actionable_subset_picks_first` |
| 18 | 한도 노출 | 성공 | 검출 | `tests::self_check_every_actionable_subset_picks_first` |
| 19 | 띠 점검 종류 교체 | 성공 | 검출 | `tests::self_check_every_actionable_subset_picks_first` |
| 20 | 띠 조치 교체 | 성공 | 검출 | `tests::self_check_every_actionable_subset_picks_first` |
| 21 | 연결 중복 이벤트 허용 | 성공 | 검출 | `tests::self_check_change_compares_hidden_status_and_suppresses_broker_events` |
| 22 | 비교 연산자 반전 | 성공 | 검출 | `tests::self_check_change_compares_hidden_status_and_suppresses_broker_events` |
| 23 | 숨은 기록 변경 무시 | 성공 | 검출 | `tests::self_check_change_compares_hidden_status_and_suppresses_broker_events` |
| 24 | 전체 변화 비교 반전 | 성공 | 검출 | `tests::self_check_change_compares_hidden_status_and_suppresses_broker_events` |
| 25 | 토스 기록도 자체 이벤트 | 성공 | 검출 | `tests::self_check_change_compares_hidden_status_and_suppresses_broker_events` |
