# ADR 0071: A-03 정보·AI 포트

- 상태: 확정 (2026-11-15)

- 최초 기준: 257bd4c94999034745f75fb7df2c1fb545138919
- 리뷰 대상: 6df4493096c9de956657ab6fe5b3a2ae8f57d346
- 수정 시 병합한 master: cb38274a4152d4171928b36c65e3f3a8407a65d7
- 최초 병합 시도: 472a45c31df167b45cd1608b7767e5b16a87fa44 (작업 중 H-45가 추가되어 최신 끝으로 재구성)
- 관련: A-03, ADR 0024·0055·0063, design 4.1·4.2·4.4·5.1·5.4, architecture 2.2·2.4·2.6·3.2·3.3·3.8

## 확정한 수정 결정

이 회차는 메인의 A-03 리뷰 수정 지시를 구현한다. 아래 결정은 사용자 확인 대기가 아니다. 어댑터와 공통 가짜가 같은 단순한 계약을 구현하게 하는 것이 목적이다. application의 의존성은 domain뿐이며 외부 의존성을 추가하지 않는다. 기존 master 테스트와 기존 분류 impl은 수정하지 않는다. application 포트는 Send + Sync, IO 메서드는 Send Future이고 조립은 제네릭으로 한다.

| 지적 | 결정과 근거 |
|---|---|
| B1 | CollectedEntry = News(domain::NewsItem) / Disclosure(domain::Disclosure). D-15의 공시 객체와 DisclosureKey를 통째로 보존한다(design 5.4). Web 변형은 없으며 웹 항목은 AI 수집 실행 소유다(architecture 2.2). |
| I1 | CollectionQuery는 CodeTopics와 UserMessage뿐이다. CodeTopics의 종목은 Symbol 목록, 주제는 CompanyNews/Earnings/Disclosures의 닫힌 목록이다. 임의 문자열 주제나 프롬프트를 받지 않는다(architecture 2.2, SEC-9). |
| I2 | Analysis에서 assignments를 제거한다. 과제 전달은 MCP get_estimate_task이며 실행 연결은 AiRunId 기준으로 A-12·S-09가 맡는다(architecture 2.4·2.6). |
| I3 | RunSpec은 id/input/model/effort/purpose를 봉인한다. model은 Default/Highest/Fast/Named(ModelName)이다. Highest/Fast는 design 4.1의 최상위/빠른 모델, Default는 P-4의 기본 설정이다. effort는 Default/Low/Medium/High/XHigh/Max이며 R-02의 low/medium/high/xhigh/max와 대응한다. CLI 이름과 옵션 변환은 어댑터 책임이다. |
| I4 | resume(spec, history, message)는 앱이 저장한 순서 있는 사용자/비서 메시지 전체와 새 메시지를 받는다(design 4.1·4.2). CLI 세션 ID나 --resume 옵션은 받지 않는다. |
| I5 | AiUsage = Unknown/Available(Utilization)/Waiting(ResetTime). Utilization은 이미 0~1 검증을 마친 domain::Probability를 봉인하며 포트에서 비율을 계산하지 않는다. 재개 시각은 ResetTime으로 통일한다(AI-2, AI-05). |
| I6 | NewsFeed/Fundamentals/MacroSeries 모두 Result<DataAvailability<…>, InformationError>이다. NotConfigured와 NoData는 실패가 아니다(P-5). |
| I7 | InvalidCredential(CredentialSource::Dart/Fred)는 사용자 조치다. 식별자 생성 실패는 IdentifierError이며 IO 오류에 들어가지 않는다. 토스 RequestRejection을 빌려 쓰지 않는다. 뉴스 소스 오류는 고급 > 상태에만 기록한다(design 2.6, A-19). |
| I8 | verify_isolation은 별도 메서드로 유지하되 run/resume의 첫 사건을 IsolationChecked로 고정한다. Rejected 뒤 사건은 없다. 사유는 design 4.1의 점검 항목과 design 2.6의 로그인/설치 상태를 닫힌 IsolationFailure로 보존한다. 이 순서 계약은 A-11과 어댑터 계약 테스트가 강제한다. |
| I9 | 게시·수집 시각, 보고 기간·행 기준일, 종목을 서로 다른 값으로 테스트한다. 식별자는 ASCII 전체 허용·거부 표와 멀티바이트 입력·길이 경계로 검사한다. 리뷰 생존 변형을 포함해 다시 손 변형한다. |
| m1 | trait 이름은 NewsFeed다. source() -> FeedSource로 자기 소스를 알린다. 다른 소스의 항목을 반환하면 계약 위반이다. architecture 3.3도 맞춘다. |
| m2 | FinancialFact/FinancialSource/FinancialUnit와 MacroFact/MacroSource/MacroUnit를 분리한다. 거시 출처에는 Dart가 없고 거시 단위에는 KRW가 없다. |
| m3 | 분석용 AnalysisPrompt는 코드·프롬프트 로더가 만든 신뢰 텍스트의 별도 봉인 타입이다. Debug는 내용을 출력하지 않는다. |
| m4 | TokenUsage는 input/output/cache_read/cache_creation을 분리한다. ActualModel(ModelName) 사건으로 실제 응답 모델을 알린다(AI-6). |
| m5 | new_items(since: FeedCursor)는 항목 목록과 다음 커서를 반환한다. 최초는 Start이며 Continue의 FeedPosition은 비신뢰 텍스트를 봉인한 불투명 어댑터 값이다. Debug에 커서 원문이 나오지 않는다. |
| m6 | 테스트 안에서 이름을 다시 매핑하는 동어반복 기대 함수는 제거했다. 제품의 생성자·접근자·trait 서명을 호출하고 독립적인 구체 값을 단언한다. |
| m7 | 이 문서를 현재 계약으로 다시 작성하고 AI-05·AI-06·N-03·N-05·C-01·C-09·A-19 인계를 추가했다. |

## 최종 메서드 계약

| 포트 | 메서드 | 성공 값 | 오류 |
|---|---|---|---|
| NewsFeed | source() | FeedSource::News(domain::NewsSource) / Disclosure(domain::DisclosureSource) | 없음 |
| NewsFeed | metadata() | NewsMetadata | 없음 |
| NewsFeed | new_items(since: FeedCursor) | DataAvailability<(Vec<CollectedObservation>, FeedCursor)> | InformationError |
| Fundamentals | statements(symbol, period_end) | DataAvailability<Vec<FinancialStatement>> | InformationError |
| MacroSeries | daily_values(series, from, through) | DataAvailability<Vec<MacroObservation>> | InformationError |
| AiProvider | run(RunSpec) | Self::Stream: AiEventStream | InformationError |
| AiProvider | resume(RunSpec, ChatHistory, UntrustedText) | Self::Stream: AiEventStream | InformationError |
| AiProvider | usage() | AiUsage | InformationError |
| AiProvider | interrupt(&AiRunId) | () | InformationError |
| AiProvider | verify_isolation(RunKind) | IsolationCheck | InformationError |
| AiEventStream | next() | Option<AiEvent> (`None`은 종료) | InformationError |

FeedCursor는 같은 피드에만 돌려준다. 어댑터가 ETag·수집 위치·접수 번호 등의 표현을 정하고 application은 해석하지 않는다. Available(empty, next_cursor)와 NotConfigured/NoData를 구분한다. 반환 항목의 소스는 source()와 같아야 한다. 페이지·304·요청 간격·실제 키 유무는 어댑터 안의 일이다. macro 시작일과 종료일은 양 끝을 포함하며 역전 기간과 반환 계열·기간의 일치는 어댑터 계약 테스트가 확인한다.

## 실행 입력과 목적

실행 종류는 Collection/Analysis 두 개다(design 4.4). Chat과 예약 작업은 목적이며 새 격리 종류가 아니다. kind는 RunInput에서 유도해 종류와 입력이 어긋나지 않는다.

CodeTopics는 Symbol과 닫힌 CollectionTopic만 받는다. 최소 주제 목록 CompanyNews/Earnings/Disclosures는 design 4.3·5.1의 뉴스·실적·공시 조사 범위를 코드 항목으로 만든 것이다. 주제 확장은 N-18에서 한다. 실제 보유·관심·후보 목록에 종목이 있는지와 검색어 검사는 N-18·N-19가 수행한다. UserMessage는 저장 ID와 원문을 가진 StoredUserMessage를 받는다. from_store_result는 저장 어댑터의 생성 경계이며 일반 UntrustedText의 From/Deref나 문자열 주제 생성자를 제공하지 않는다. 공개 생성자는 저장 사실의 진위를 인증하지 않는다. 실제 호출 위치는 저장 어댑터와 테스트로 제한해야 한다.

AnalysisPrompt는 빈 값과 65,536바이트 초과를 거부하고, 명시적 as_str로만 로더·어댑터에 전달한다. ChatHistory는 ChatMessage의 순서 있는 목록이며 역할은 User/Assistant뿐이다. 원문은 UntrustedText로 유지한다. 재개는 이미 저장한 대화 전체를 원래 순서로 주입한다. 현재 살아 있는 채팅 프로세스 재사용과 새 프로세스 재주입의 선택은 어댑터의 일이다.

| RunPurpose | 문서 근거 |
|---|---|
| Chat | design 4.2 |
| MorningBriefing, IntradayMonitoring, ClosingReview, UsMarketCheck, CandidateDiscovery, WeeklyReview | design 4.3 예약 작업 표 |
| Estimate, DeepResearch | design 4.1 모델 기본값, 4.3 심층 연구, 6.1 |
| NewsClassification, NewsSummary, WebCollection | design 4.1 빠른 모델, 4.4·5.4 |
| ThesisReview, ClaimVerification | design 4.3 감시·심층 연구 단계 4 |
| PairedComparison | design AI-5 |

AiRunId/MacroSeriesId/ModelName/UserMessageId는 1~128바이트 ASCII 영숫자와 _, ., -만 허용한다. 이는 내부 식별자 정책이며 자유 텍스트나 CLI 인자를 받는 타입이 아니다. Named 모델과 실제 응답 모델은 같은 봉인 타입을 쓰고, 요청한 모델과 실제 응답 모델을 동일하다고 간주하지 않는다.

## 스트림과 격리 점검

run/resume는 사전 점검 메서드 호출 여부와 관계없이 첫 사건으로 IsolationChecked(Passed 또는 Rejected(IsolationFailure))를 낸다. Rejected 이후 next는 None이며 실제 프롬프트를 보내거나 후속 사건을 내지 않는다. 점검 전에 프로세스를 시작할 수조차 없는 IO 실패는 스트림 생성의 Err다. verify_isolation 결과만 저장해 과거 증거로 실행하지 않는다. 실제 실행마다 해당 프로세스에서 점검한다.

IsolationFailure는 Tools/McpServers/Plugins/Skills/SlashCommands/PermissionMode/ApiKeySource/CliVersion/LoginRequired/NotInstalled다. 앞의 여덟 개는 design 4.1의 init 점검·버전 기록 및 REL-8에, 마지막 둘은 design 2.6의 자체 점검에 대응한다. 세부 외부 오류나 원문을 사유로 넣지 않는다. 포트 trait만으로 어댑터의 사건 순서를 증명하지는 않는다. 테스트 대본은 계약 예시이며 실제 강제 증거는 A-11·AI-02·AI-11이 제공한다.

기존 사건 TextDelta/ToolCall/WebSearch/Quota/RateLimited/ModelRerouted/McpUnavailable/Unknown을 보존하고 IsolationChecked와 ActualModel을 추가한다. TokenUsage는 네 토큰 수를 구별한다. 도구 인자·결과·검색·응답·Unknown 원문은 UntrustedText다. 한도 초과는 오류가 아닌 AiUsage::Waiting 또는 RateLimited 사건이며 둘 다 ResetTime을 사용한다.

## 사실과 오류의 경계

CollectedEntry는 D-15 NewsItem/Disclosure를 보존하므로 제목·본문·출처·원문 주소와 공시 키를 잃지 않는다. 게시 시각이 없으면 NotProvided, 수집 시각은 필수 Timestamp다. 웹 항목을 피드에 넣거나 필수 원문 주소를 발명하지 않는다. NewsMetadata의 네 정책 필드는 닫힌 타입이며 실제 소스별 정책과 본문 저장 금지는 N-17·M-06·저장 어댑터가 적용한다.

FinancialFact와 MacroFact는 값/단위/기준일/각자의 출처를 봉인한다. MetricValue는 domain::Amount의 정확한 숫자 표현만 사용하고 금액·비율 계산을 하지 않는다. 재무 단위는 Currency(KRW/USD)/Percent/Multiple/Count, 출처는 Dart다. 거시 단위는 Percent/IndexPoints/Usd, 출처는 UsTreasury/Fred/TossEtf(Symbol)다. 금융·회계 계정 이름은 외부 UntrustedText이며 코드로 승격하지 않는다. 실제 계열별 단위와 응답의 진위는 N-08·N-16이 검증한다.

| InformationError | ClassifiedError 결과 | 보존·처리 |
|---|---|---|
| InvalidCredential(Dart/Fred) | UserAction(CheckCredentials) | 원본 오류에 자격 소스 보존. A-19는 고급 > 상태에만 기록 |
| InvalidResponse(Schema/Validation) | Bug(Response(해당 원인)) | 요청·응답 본문 없음 |
| Unavailable(ResetTime) | Wait(RetryDelay::Unknown) | 원본 오류에 절대 재개 시각 보존 |

A-05 ErrorClass::Wait는 상대 TimeSpan만 받는다. 현재 시각 없이 절대 재개 시각에서 상대 대기 시간을 계산할 수 없으므로 Unknown을 반환한다. 0초나 임의 지연을 만들지 않는다. 절대 시각을 사용하는 호출자는 원본 오류를 보존하며, A-05·AI-05·A-19가 Clock을 사용하는 소비 경계에서 변환 계약을 정한다. 포트에는 Clock 읽기나 새 상대 대기 필드를 더하지 않는다. 선택 키 없음은 NotConfigured, 식별자 생성 실패는 IdentifierError다.

## 인계

| 받는 작업 | 남은 책임 |
|---|---|
| A-04, A-11 | 새 포트 서명과 커서를 사용하는 가짜. run/resume 첫 사건 점검, 거절 뒤 종료, 소스 일치, 사용자 메시지 원문·대화 순서 검사 |
| AI-01, AI-02, AI-11 | 녹화 사건 매핑, 네 토큰 수·실제 모델 파싱, 실행별 격리 점검과 첫 사건 계약 강제 |
| AI-05 | resetsAt을 절대 ResetTime으로 보존하고 재개. 상대 대기 변환은 Clock을 받는 소비 경계에서 결정 |
| AI-06, AI-08 | Utilization으로 예약 작업 기본 70% 정책 적용, 한도별 사용량과 입력·출력·캐시 토큰 기록 |
| AI-04, C-01, C-09 | history 전체와 새 메시지의 순서·원문을 보존하고, 살아 있는/미리 띄운 채팅 실행과 재시작을 검증 |
| A-12, S-09 | AiRunId 기준 실행과 과제 연결, get_estimate_task 전달, 중복·다른 실행 제출 거부 |
| N-03, N-05, N-07 | CollectedEntry::Disclosure의 원래 DisclosureKey 유지. 출처 간 접수 번호를 합치지 않음 |
| N-01~N-07, N-17 | source()와 반환 항목 일치, 선택 키·연락처 없음, 불투명 커서와 304·보존·본문 읽기 정책 |
| N-18, N-19, S-09 | 알려진 종목 목록·닫힌 주제 확장, 검색어 검사, StoredUserMessage의 저장 결과 생성 경계 |
| N-08, N-16, A-09, D-37 후속 | 재무·거시 사실의 domain 편입, 계정·보고 기간·단위 계약, 공식/ETF 표시 |
| A-19 | 자격 오류의 사용자 조치 분류를 연결 상태 띠로 올리지 않고 소스별 고급 > 상태에 기록. 절대 재개 시각 보존 |
| AI-07, AI-17 | 프롬프트 ID/버전/해시·CLI 버전·증거 묶음 메타데이터의 실행 기록 연결 |

## 검증과 앞 회차 보고 정정

앞 회차의 손 변형 20개 검출은 그때 선택한 변형에만 해당했다. 독립 리뷰의 21개 중 8개 생존은 이전 검증이 필드 뒤바꿈·고정 기호·문자 집합 확장을 충분히 확인하지 못했음을 보여 준다. 앞 회차의 생존 0을 이 수정의 증거로 사용하지 않는다.

이번 테스트는 서로 다른 게시·수집 시각(11/99), 보고 기간·행 기준일(15/10), MSFT/NVDA와 QQQ/VT를 사용한다. 식별자는 ASCII 0~127 전체와 명시적 +/@/: 거부, 멀티바이트·빈 값·128/129바이트 경계를 검사한다. 테스트 안 이름 매핑 함수를 검증하는 대신 제품 생성자·접근자와 포트 메서드를 호출한다. 실제 외부 API는 호출하지 않는다. 손 변형은 전용 임시 복사본에서만 적용하고 컴파일 실패를 검출 수에 세지 않는다.

## 수정 후 손 변형 결과

동일한 수정 소스의 임시 복사본에서 `cargo test --locked -p application --test ports_a03 --test ports_a03_contract`를 실행했다. 기준 테스트 통과 후 각 변형을 하나씩 적용하고 원복했다. 43개 모두 컴파일 후 테스트 실패로 검출됐으며 생존 0, 컴파일 불가 0, 시간 초과 0이다. 게시/수집 뒤바꿈·첫 행 기준일·SPY/AAPL 고정·+/@/: 문자 확장 반례를 포함한다.

| # | 파일 | 손 변형 | 결과 | 실제 실패 테스트 중 하나 |
|---|---|---|---|---|
| 1 | news_source.rs | 게시 시각을 수집 시각으로 | 검출(exit 101) | missing_publication_does_not_replace_collection_time |
| 2 | news_source.rs | 수집 시각을 게시 시각으로 | 검출(exit 101) | news_and_disclosures_preserve_identity_and_distinct_times |
| 3 | fundamentals.rs | 보고 기준일을 첫 행 기준일로 | 검출(exit 101) | financial_period_symbol_and_row_date_remain_independent |
| 4 | fundamentals.rs | 재무 종목을 AAPL로 고정 | 검출(exit 101) | financial_period_symbol_and_row_date_remain_independent |
| 5 | macro_series.rs | ETF 기호를 SPY로 고정 | 검출(exit 101) | macro_facts_preserve_etf_symbol_and_official_provenance |
| 6 | ai_request.rs | AiRunId + 허용 | 검출(exit 101) | identifiers_check_every_ascii_character_and_length_boundaries |
| 7 | ai_request.rs | AiRunId @ 허용 | 검출(exit 101) | identifiers_check_every_ascii_character_and_length_boundaries |
| 8 | ai_request.rs | AiRunId : 허용 | 검출(exit 101) | identifiers_check_every_ascii_character_and_length_boundaries |
| 9 | ai_request.rs | AiRunId 빈 ID 허용 | 검출(exit 101) | identifiers_check_every_ascii_character_and_length_boundaries |
| 10 | ai_request.rs | AiRunId 128 거절 | 검출(exit 101) | identifiers_check_every_ascii_character_and_length_boundaries |
| 11 | ai_request.rs | AiRunId 129 허용 | 검출(exit 101) | identifiers_check_every_ascii_character_and_length_boundaries |
| 12 | ai_request.rs | AiRunId 숫자 거절 | 검출(exit 101) | identifiers_check_every_ascii_character_and_length_boundaries |
| 13 | macro_series.rs | MacroSeriesId + 허용 | 검출(exit 101) | identifiers_check_every_ascii_character_and_length_boundaries |
| 14 | macro_series.rs | MacroSeriesId @ 허용 | 검출(exit 101) | identifiers_check_every_ascii_character_and_length_boundaries |
| 15 | macro_series.rs | MacroSeriesId : 허용 | 검출(exit 101) | identifiers_check_every_ascii_character_and_length_boundaries |
| 16 | macro_series.rs | MacroSeriesId 빈 ID 허용 | 검출(exit 101) | identifiers_check_every_ascii_character_and_length_boundaries |
| 17 | macro_series.rs | MacroSeriesId 128 거절 | 검출(exit 101) | identifiers_check_every_ascii_character_and_length_boundaries |
| 18 | macro_series.rs | MacroSeriesId 129 허용 | 검출(exit 101) | identifiers_check_every_ascii_character_and_length_boundaries |
| 19 | macro_series.rs | MacroSeriesId 숫자 거절 | 검출(exit 101) | identifiers_check_every_ascii_character_and_length_boundaries |
| 20 | ai_request.rs | ModelName + 허용 | 검출(exit 101) | identifiers_check_every_ascii_character_and_length_boundaries |
| 21 | ai_request.rs | UserMessageId + 허용 | 검출(exit 101) | identifiers_check_every_ascii_character_and_length_boundaries |
| 22 | news_source.rs | retention 고정 | 검출(exit 101) | source_policies_preserve_all_independent_fields |
| 23 | news_source.rs | allow_ai_read 고정 | 검출(exit 101) | source_policies_preserve_all_independent_fields |
| 24 | news_source.rs | supports_conditional_get 고정 | 검출(exit 101) | source_policies_preserve_all_independent_fields |
| 25 | news_source.rs | requires_user_agent 고정 | 검출(exit 101) | source_policies_preserve_all_independent_fields |
| 26 | ai_provider.rs | 토큰 input 뒤바꿈 | 검출(exit 101) | tokens_and_actual_model_are_distinct_result_facts |
| 27 | ai_provider.rs | 토큰 output 뒤바꿈 | 검출(exit 101) | tokens_and_actual_model_are_distinct_result_facts |
| 28 | ai_provider.rs | 토큰 cache_read 뒤바꿈 | 검출(exit 101) | tokens_and_actual_model_are_distinct_result_facts |
| 29 | ai_provider.rs | 토큰 cache_creation 뒤바꿈 | 검출(exit 101) | tokens_and_actual_model_are_distinct_result_facts |
| 30 | fundamentals.rs | 재무 단위 KRW 고정 | 검출(exit 101) | financial_period_symbol_and_row_date_remain_independent |
| 31 | macro_series.rs | 거시 단위 Percent 고정 | 검출(exit 101) | macro_facts_preserve_etf_symbol_and_official_provenance |
| 32 | macro_series.rs | 거시 계열 DGS10 고정 | 검출(exit 101) | macro_facts_preserve_etf_symbol_and_official_provenance |
| 33 | ai_request.rs | 대화 이력 삭제 | 검출(exit 101) | history_keeps_roles_order_and_exact_untrusted_text |
| 34 | ai_request.rs | 역할 Assistant 고정 | 검출(exit 101) | history_keeps_roles_order_and_exact_untrusted_text |
| 35 | ai_request.rs | effort 고정 | 검출(exit 101) | analysis_spec_and_prompt_preserve_model_effort_and_purpose |
| 36 | ai_request.rs | purpose 고정 | 검출(exit 101) | analysis_spec_and_prompt_preserve_model_effort_and_purpose |
| 37 | ai_request.rs | model 고정 | 검출(exit 101) | analysis_spec_and_prompt_preserve_model_effort_and_purpose |
| 38 | ai_request.rs | 수집 종류를 분석으로 | 검출(exit 101) | collection_queries_separate_code_topics_from_stored_user_messages |
| 39 | ai_request.rs | 분석 종류를 수집으로 | 검출(exit 101) | analysis_spec_and_prompt_preserve_model_effort_and_purpose |
| 40 | information_error.rs | 잘못된 자격을 버그로 분류 | 검출(exit 101) | errors_preserve_credential_source_and_absolute_reset_time |
| 41 | information_error.rs | 응답 원인 Schema 고정 | 검출(exit 101) | errors_preserve_credential_source_and_absolute_reset_time |
| 42 | ai_request.rs | 프롬프트 상한 65537 허용 | 검출(exit 101) | analysis_spec_and_prompt_preserve_model_effort_and_purpose |
| 43 | ai_request.rs | 빈 프롬프트 허용 | 검출(exit 101) | analysis_spec_and_prompt_preserve_model_effort_and_purpose |

수정 중 검사: `cargo test -p application` 31개 통과(기존 14개, 수정 테스트 17개), clippy·source-check·raw-types 통과. 고정 Rust 1.99.1을 사용하는 별도 임시 소비자에서 코드 종목/주제 문자열, 수집 원문, 저장 메시지 대신 원문, 비신뢰 분석 프롬프트, assignments 필드, 피드 Web 변형, 거시 Dart/KRW, 원시 실수 사용률, 비공개 프롬프트 직접 생성 등 11개를 검사했다. 모두 의도한 타입/가시성 오류로 거부됐다. 이 컴파일 거부 사례는 손 변형 43개와 별도로 센다.

최신 master 병합의 생성물 충돌을 공식 스크립트로 해결했다. 최종 커밋 전에 시작한 중간 병합 검사 실행은 이번 지시의 전체 검사 1회 원칙에 맞추어 소유 프로세스 트리만 종료했다. 수정은 병합 트리에 함께 담고 정상 커밋 훅 전체 검사로 최종 검증한다.
