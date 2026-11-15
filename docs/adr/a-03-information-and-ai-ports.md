# A-03 정보 수집과 AI 실행 포트

- 상태: 초안
- 기준: 257bd4c94999034745f75fb7df2c1fb545138919
- 관련: A-03, ADR 0024, ADR 0055, ADR 0063, design 4.1·4.4·5.1, architecture 3.2·3.3·3.8

## 결정

application은 domain에만 의존한다. 네 포트는 Send + Sync이며 IO 메서드는 Send Future다. dyn 주입은 지원하지 않는다. 실행 환경·키·PATH·CLI 경로는 포트 입력에 없고 어댑터 조립에 속한다. 어댑터와 testkit 구현은 이번 범위에 없다. 테스트의 Contract는 메서드 서명을 컴파일하는 즉시 응답 대본이다.

## 메서드 계약 표

| architecture 3.3 문장 | 포트와 메서드 | 입력 | 출력 |
|---|---|---|---|
| 한 소스의 새 항목 | NewsSource::new_items | 없음, 한 인스턴스가 한 소스 | Vec<CollectedObservation> |
| retention, allow_ai_read, supports_conditional_get, requires_user_agent | NewsSource::metadata | 없음 | NewsMetadata |
| 재무제표 | Fundamentals::statements | Symbol, period_end | DataAvailability<Vec<FinancialStatement>> |
| 미국 금리·지수의 일별 값 | MacroSeries::daily_values | MacroSeriesId, 시작일, 종료일(양 끝 포함) | DataAvailability<Vec<MacroObservation>> |
| run(RunSpec) | AiProvider::run | RunSpec | 연결된 AiEventStream |
| resume | AiProvider::resume | 앱 기록으로 다시 만든 RunSpec, 사용자 메시지 | 연결된 AiEventStream |
| usage | AiProvider::usage | 없음 | AiUsage |
| interrupt | AiProvider::interrupt | AiRunId | () |
| verify_isolation | AiProvider::verify_isolation | RunKind | IsolationCheck |

메서드 이름이 명시된 AI 다섯 개는 그대로 보존했다. 다른 세 포트는 문서의 역할을 최소 메서드 이름으로 구체화한 초안이다. 뉴스 metadata는 IO 없이 설정된 사실을 돌려준다. 모든 IO 메서드와 스트림 next의 오류는 InformationError이고 ClassifiedError를 직접 구현한다. 브로커 PortError로 변환하지 않는다.

## 정보와 메타데이터

CollectedObservation은 D-15의 CollectedItem을 그대로 봉인하며 게시 시각은 Known(Timestamp)/NotProvided, 수집 시각은 Timestamp다. 출처와 원문 주소는 CollectedItem 내부의 InformationSource와 NormalizedUrl에 있다. D-15가 주소를 필수로 요구하므로 주소 없는 항목의 가짜 주소를 만들지 않는다. 파싱할 수 없는 필수 필드는 InvalidResponse(Validation)이며 원문을 오류에 넣지 않는다. 시간 필드는 D-15 ADR의 미해결을 domain 수정 없이 application 봉투로 보완한다.

본문·제목·프롬프트·응답·도구 인자·알 수 없는 사건의 원문은 UntrustedText다. Debug는 byte_len만 보여 주는 domain 정책을 상속한다. 저장 금지·본문 읽기 허용은 타입의 존재만으로 강제되지 않는다. N-17·M-06·저장 어댑터가 정책을 적용한다. metadata에는 보존 기간/보관 안 함, 제목만/제목과 요약/본문 허용, 조건부 GET 지원/미지원, 기본/브라우저/연락처 User-Agent 요구가 있다. 새 사용자 설정은 만들지 않는다.

MetricFact는 MetricValue(숫자 Amount), MetricUnit, 기준일, MetricSource를 묶는다. Money나 Ratio로 수치를 대신하지 않고 Amount의 정확한 십진 표현만 재사용한다. Currency(KRW/USD), Percent(퍼센트 숫자 그대로), IndexPoints, Multiple, Count를 구별한다. Dart·UsTreasury·Fred와 TossEtf(Symbol)를 보존하므로 공식 값과 ETF 근사를 소비자가 구분한다. 변환·비율 계산·집계 로직은 없다. 재무 행의 외부 계정명은 UntrustedText이며 회계 계정 코드로 승격하지 않는다.

DataAvailability는 Available(T), NotConfigured, NoData다. 선택 키가 없음을 오류나 전체 연결 실패로 바꾸지 않으며 정상 빈 목록도 보존할 수 있다. macro 조회의 역전 기간 거부와 요청·응답의 계열/기간 일치는 실제 어댑터 계약 테스트에서 확인한다. 봉투 생성자가 공급자 사실의 진위를 인증하지는 않는다.

## 실행 종류와 사건

실행 종류는 Collection(수집 실행), Analysis(분석 실행) 두 가지다. 근거는 design 4.4의 실행 표와 architecture 3.3의 RunSpec 실행 종류다. 채팅과 예약 작업은 작업 목적이며 세 번째 격리 종류가 아니다. design 4.2는 채팅이 분석 실행이라고 명시한다.

RunSpec은 실행 ID와 RunInput을 봉인한다. RunInput의 Collection에는 비신뢰 주제 입력만 있고 Analysis에는 프롬프트와 코드가 만든 EstimateTask 목록이 있다. kind는 이 합 타입에서 유도해 종류와 입력이 어긋나는 중복 상태를 없앤다. Collection 필드에 EstimateTask나 계좌 타입을 넣는 경로는 없다. 자유 텍스트의 비밀 부재까지 이 타입이 증명하지 않으므로 N-19와 저장 사용자 메시지 경로가 검증해야 한다.

resume는 CLI 세션 ID를 받지 않고 앱이 저장한 대화를 RunSpec으로 재구성한다(design 4.1). CLI --resume를 요구하지 않는다. AiRunId와 MacroSeriesId는 1~128바이트 ASCII 영숫자 및 _.− 중 ASCII 하이픈만 허용하는 내부 식별자다. 외부 제공자 식별자 전체의 규격을 확정했다는 뜻은 아니다.

AiEvent는 architecture 3.3의 TextDelta, ToolCall, WebSearch, TokenUsage, Quota, RateLimited, ModelRerouted, McpUnavailable, Unknown을 닫힌 열거형으로 둔다. AiEventStream::next는 None으로 종료를 표현하며 Vec로 전체 실행을 기다리지 않는다. RateLimited는 오류가 아닌 사건이고 재개 시각은 Known/NotProvided다. IsolationCheck는 Passed/Rejected/LoginRequired/NotInstalled를 보존한다. 이 결과만으로 실제 격리가 증명되지는 않는다.

## 오류 분류

| InformationError | ErrorClass |
|---|---|
| Unavailable(retry) | Wait(retry), 알려진 대기 원값 보존 |
| InvalidResponse(Schema/Validation) | Bug(Response(해당 원인)) |
| InvalidIdentifier | Bug(Request(MalformedRequest)) |

비밀·본문·외부 오류 객체가 오류 값에 들어가지 않는다. 선택 키 없음은 DataAvailability::NotConfigured, AI 미설치·미로그인은 사전 점검 결과다. 한도 초과는 AiEvent/AiUsage의 대기이며 실패 오류로 바꾸지 않는다. 추가 오류 조치가 필요하면 ADR 0063의 인계에 따라 별도 닫힌 원인과 분류 확장이 필요하다.

## 정의 대기와 인계

| 받는 작업 | 정의 대기 | 타입 초안·귀속 |
|---|---|---|
| D-15 후속, N-01 | 게시·수집 시각의 domain 편입 | CollectedObservation을 domain으로 옮길지 결정 |
| D-37, N-08, N-16, A-09 | 수치 타입의 domain 편입, 계정 코드·공시 기간·연결/별도 구분, 계열별 단위 목록 | MetricFact·FinancialStatement·MacroObservation. 현재 application 임시 봉투 |
| N-01~N-07, N-17 | 소스별 metadata 실제 값, 조건부 요청 커서, 304·부분 수집·빈 목록의 계약 | NewsSource. 현재 메서드에 HTTP 헤더·페이지를 노출하지 않음 |
| N-08, N-16 | 실제 응답 단위·요청 기간·출처 일치 검증 | Fundamentals·MacroSeries 계약 테스트 |
| AI-01, AI-02, AI-08 | 두 제공자 녹화 사건 매핑, 도구 중간 결과 유무, 입력/출력/캐시 토큰·한도 수치 | AiEvent·TokenCount·AiUsage의 최소 모델 확장. 현재 Available은 수치 사용률을 제공하지 않음 |
| AI-04, AI-07, A-12, AI-17 | 모델·추론 강도·프롬프트 ID/버전/해시, 배정 과제 ID와 실행 ID 결합·중복 제출 검사 | RunSpec·RunInput 확장. EstimateTask 값만 전달하며 배정의 진위나 제출 권한을 인증하지 않음 |
| AI-11, AI-12, AI-13 | 실제 사전 점검 진단·로그인·설치 상태, 실행과 통과 증거의 결합 | AiProvider. 이번 포트만으로 사전 점검 강제를 주장하지 않음 |
| A-04, A-11 | 공통 포트 가짜·녹화 CLI | 현재 trait과 스트림 계약을 사용 |
| 하네스 후속 | application 새 파일의 자동 변이 범위 등록 | 보호된 mutants 설정은 이번에 수정하지 않음 |

## 검증

계약 표의 모든 메서드를 부르는 대본 테스트는 Send Future와 반환 타입을 컴파일로 대조한다. 수집 정보, 필드 보존, 메타데이터 조합, 식별자 경계, 실행 종류, 사건 목록, Debug 비노출, ClassifiedError의 소진 기대 함수를 검사한다. 실제 어댑터나 실제 API에 대한 증거는 아니다. 기존 테스트 본문과 기존 impl, 의존성, tasks.md·progress.md는 수정하지 않는다.

자동 변이 설정은 새 application 포트 파일을 포함하지 않으므로 임시 복사본에서 새 코드의 조건·경계·반환 값을 손 변형해 검증한다. 컴파일 실패는 검출 수에 포함하지 않는다.

## 미해결과 사용자 확인 대상

위 표의 임시 수치 모델을 domain으로 옮길 시점, 최초 세 포트의 메서드 명명, 식별자 128바이트 상한은 초안이다. 정해지지 않은 회계 규격과 AI 제공자별 사용량 의미를 이번에 발명하지 않는다. 모델·과제 ID·이벤트 세부는 담당 작업이 녹화 자료와 소비자 계약으로 확정해야 한다. 화면 문구나 설정을 추가하지 않는다.

검증 실측: application 전체 25개 통과(기존 14개와 신규 11개), clippy·raw-types·source-check 통과. 손 변형 24개 중 컴파일 가능한 20개 검출, 생존 0개. 컴파일 불가 4개는 제외했다. 유효 변형은 두 식별자의 빈 값·129 허용·128 거부·숫자 거부·구분자 거부 각 5개, 실행 종류 교환 2개, 게시 시각과 소스 정책 반환 변경 5개, 단위·출처 반환 변경 2개, 토큰 수 반환 변경 1개다. 실제 테스트 실패(exit 100)를 확인했다. 코드 계산이나 외부 API를 검증한 결과가 아니다.
