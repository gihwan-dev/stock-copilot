# ADR: AI-01 실행 이벤트 모델 (`AiEvent`)

- 상태: 초안
- 날짜: 2026-11-24
- 결정한 사람: 구현 담당 초안(메인·사용자 확인 대기)
- 관련 작업: AI-01 (선행 A-03, 병합 묶음 A-11, 연관 AI-02·AI-03·AI-04·AI-05·AI-08·AI-09·AI-11·AI-14·AI-16·AI-17)
- 반영한 문서·코드: `crates/adapters/ai-claude-code/**`, `crates/adapters/ai-codex/**`, `fixtures/ai/*.jsonl`, `harness/arch-check/rules.toml`, `Cargo.lock`

## 맥락

`docs/architecture.md` 3.3과 [ADR 0071](0071-information-and-ai-ports.md)(A-03)은 `application::AiEvent` 11개 변형을 정의하고, AI 실행 스트림의 첫 사건이 반드시 `IsolationChecked(Passed | Rejected(IsolationFailure))`이며 거절 뒤 후속 사건이 없어야 한다고 고정했다. AI-01의 완료 기준은 다음과 같다.

- `architecture.md` 3.3의 `AiEvent` 변형과 모르는 `type`을 담는 `Unknown`으로 두 제공자(Claude Code CLI `stream-json`, Codex CLI `app-server` JSON-RPC 및 `exec --json`)의 녹화 출력을 같은 모델로 표현한다.
- 모르는 이벤트 종류·깨진 줄·필수 필드 누락·순서 위반 입력을 추정하지 않고 구분된 값(`AiEvent::Unknown` 또는 명시적 오류 타입 `StreamDecodeError`)으로 보존하며 조용히 버리지 않는다.
- 실제 Claude·Codex CLI나 네트워크를 호출하지 않고 비식별 합성 고정 자료(`fixtures/ai/*.jsonl`)로 계약 테스트와 속성 테스트를 고정한다.
- 새 외부 패키지를 추가하지 않는다(`deny.toml` 무변경).

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| `application::AiEvent` 11개 변형을 그대로 유지하고, 깨진 줄·필수 필드 누락·타입 불일치·도메인 값 검증 실패·순서 위반은 각 어댑터 크레이트의 명시적 `StreamDecodeError`(`EmptyLine`·`LineTooLong`·`MalformedJson`·`Schema`·`Validation`·`Order`)로 보존한다 | A-03(`ports_a03`·`ports_a03_contract`)의 기존 테스트와 포트 계약을 한 줄도 바꾸지 않으면서, 모르는 정상 JSON 이벤트(`AiEvent::Unknown`)와 파싱·스키마·순서 오류(`StreamDecodeError`)를 섞지 않고 보존한다. `StreamDecodeError::to_information_error()`가 `InformationError::InvalidResponse(Schema|Validation)`로 사상하고 `Display`·`Debug`에서 원문 줄을 노출하지 않는다 | `application::AiEvent`에 오류 변형 추가(기존 A-03 테스트 변경 필요, 정상 이벤트와 프로토콜 오류 혼합); 깨진 줄을 `AiEvent::Unknown`으로 흡수(오류 은닉) | 메인 선택지 A 결정 4, ADR 0071, architecture 3.3·3.8 |
| Claude Code(`ai-claude-code`)는 `ClaudeEventDecoder`, Codex(`ai-codex`)는 `CodexAppServerDecoder`(기본 경로)와 `CodexExecDecoder`(보조 JSONL 경로)를 각각 상태 기계 디코더로 구현한다 | `init` / `(thread/start, mcpServerStatus/list)` 이전의 조기 이벤트, 초기 스냅숏 중복 덮어쓰기, 거절 뒤 후속 이벤트, 중복·고아·미종결 도구 호출, 비허용 도구 실행, 후속 턴 격리 퇴보(추가 서버·추가 도구 등)를 상태 기계로 기계적으로 차단하고 즉시 `TerminatedAfterRejection`으로 닫는다 | 줄 단위 무상태 파서(첫 사건 격리 계약·도구 짝·거절 후 종료를 강제할 수 없음); 후속 턴 격리 퇴보를 `McpUnavailable`로 덮어쓰기 | R-02, R-03, design 4.1, SEC-2, ADR 0071 |
| 사용률(`utilization`·`usedPercent`) 변환은 부동소수점 없이 문자열 십진 이동 + `domain::Amount::parse` + `domain::Probability::try_new`로만 수행한다 | 하네스 `float_arithmetic` 금지와 `domain` 십진 확률 불변식을 지키고 `0..=100` 백분율(예: `7.5` → `0.075`, `100` → `1.00`, `101` 거부)을 정확하게 검증한다 | `f64` 나눗셈(`used_percent / 100.0`, 린트 금지 및 이진 부동소수점 오차) | harness 3.1, architecture 3.4 |
| `CodexAppServerDecoder`는 `sandbox.networkAccess`, `multiAgentMode`, `mcpServerStatus/list` 항목의 `runtimeStatus`(우선) 또는 `status`가 누락·`null`이면 닫힌 실패(`SchemaFailure::MissingField`·`InvalidFieldType`)로 거부하고, `RunKind::Analysis`에서 `sandbox.networkAccess == true`이면 `IsolationFailure::PermissionMode`로 거절한다 | R-03 실측 스키마(`runtimeStatus`, `sandbox.networkAccess`, `multiAgentMode`)를 따르며, 필드 누락을 안전 기본값으로 추정하거나 분석 실행의 네트워크 개방을 통과시키는 열린 실패를 막는다 | 누락 시 `"ready"`·`false` 기본값(열린 실패); `networkAccess` 무시(분석 실행 웹 격리 위반) | R-03 473~474행, design 4.4, SEC-2 |
| `harness/arch-check/rules.toml`에 `ai-claude-code`·`ai-codex`의 `serde_json`·`proptest` 경유 `syn`·`proc-macro2` 최소 예외만 등록한다 | 작업 공간 기능 통합으로 생기는 기존 crate들과 동일한 전이 경로이며 새 외부 패키지 추가(`deny.toml` 변경 및 `Cargo.lock` 외부 패키지 변경)가 없다(`Cargo.lock`에는 작업 공간 crate의 기존 의존 목록만 갱신) | 자체 JSON 파서 작성(불필요한 복잡성과 보안 위험) | 메인 선택지 A 결정 1 |

## 이벤트 종류 대응 표

| `application::AiEvent` 변형 | Claude Code `stream-json` (`ClaudeEventDecoder`) | Codex `app-server` JSON-RPC (`CodexAppServerDecoder`) | Codex `exec --json` (`CodexExecDecoder`) |
|---|---|---|---|
| `IsolationChecked(Passed \| Rejected(IsolationFailure))` | 첫 `system`/`init` 줄의 8항목(`apiKeySource`, `permissionMode`, `skills`, `slash_commands`, `plugins`/`plugin_errors`, `mcp_servers`/`mcp_server_errors`, `tools`, `claude_code_version`) 검사 결과 | `thread/start` 응답(`sandbox`, `instructionSources`, `multiAgentMode`, `cliVersion`) + `mcpServerStatus/list` 응답(`data[]`의 `name`, `runtimeStatus`/`status`, `tools`) 결합 평가(초기 중복 스냅숏은 `DuplicateIsolationSnapshot` 오류); 초기 `unauthorized` RPC 오류 → `LoginRequired`; 초기 `mcpServer/startupStatus/updated` `failed`/`error` → `McpServers` | 사전 점검에서 확정된 `IsolationCheck`를 `take_initial_event()` 또는 첫 `decode_line()` 선두에 1회 방출(`Rejected`는 첫 `decode_line()`에서도 유실 없이 방출 후 종료) |
| `TextDelta(UntrustedText)` | `stream_event`/`content_block_delta`/`text_delta`; 직전 스트림 델타가 없었을 때 `assistant` `text` 블록(`<synthetic>` 합성 오류 메시지는 `TextDelta` 미방출) | `item/agentMessage/delta`; 직전 스트림 델타가 없었을 때 `item/completed` `agentMessage`(각 `agentMessage` 완료마다 델타 플래그 초기화) | `item.completed` `agent_message` |
| `ToolCall { server, tool, args, result, status }` | `assistant` `tool_use`(허용 도구 목록 대조 후 `mcp__<server>__<tool>` 또는 내장 도구 `server: ""` → `Running`), `user` `tool_result`(`is_error: false` → `Succeeded`, `true` → `Failed`), 실패 `result` 시 열린 도구 `Failed`(결과 `"turn_failed"`) 종결 | `item/started` `mcpToolCall`(허용 서버·도구 대조 후 `Running`), `item/completed` `mcpToolCall`(`completed`/`succeeded` → `Succeeded`, `failed`/`error` → `Failed`), 실패 `turn/completed` 시 열린 도구 `Failed`(결과 `"turn_failed"`) 종결 | `item.started` `mcp_tool_call`(`Running`), `item.completed` `mcp_tool_call`(`Succeeded`/`Failed`), `turn.failed` 시 열린 도구 `Failed`(결과 `"turn_failed"`) 종결 |
| `WebSearch { query, action }` | `assistant` `tool_use`(`name == "WebSearch"` → `query`, `action` 기본 `"search"`), 대응 `user` `tool_result`(`"completed"` / `"failed"`), 실패 `result` 시 열린 검색 `"failed"` 종결 | `item/started`·`item/completed` `webSearch`(`query`, `action` 문자열 또는 `action.type`) | `item.started`·`item.completed` `web_search` |
| `TokenUsage(TokenUsage)` | `result.usage`(`input_tokens`, `output_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens`) | `thread/tokenUsage/updated`(`last` 우선 또는 `total`: `inputTokens`, `outputTokens`, `cachedInputTokens`, `cacheWriteInputTokens`) | `turn.completed.usage`(`input_tokens`, `output_tokens`, `cached_input_tokens`, `cache_write_input_tokens`) |
| `Quota(AiUsage)` | `rate_limit_event` `allowed`/`allowed_warning` → `Available(max utilization)` 또는 `Unknown`; `rejected` → `Waiting(resets_at)` | `account/rateLimits/updated` 또는 RPC 결과: `ordinaryUsageAllowed == null`/부재 → `Unknown`, `false` 또는 비어 있지 않은 문자열 `rateLimitReachedType` → `Waiting(resets_at)`, `true` + `primary`/`secondary` `usedPercent` 최댓값 → `Available(usedPercent/100)` | 미제공(R-03: `exec --json`은 한도 이벤트 없음) |
| `RateLimited { resets_at }` | `rate_limit_event` `status == "rejected"`, `assistant` `model == "<synthetic>"` + `error == "rate_limit"`, `result` `is_error == true` + 한도 사유 | `account/rateLimits/updated` 거절/도달, `turn/completed` `status == "failed"` 또는 `error` 알림/RPC 오류의 `codexErrorInfo`가 `usageLimitExceeded`·`rateLimitExceeded`·`usage_limit_reached` | `turn.failed`·`error`의 `code`가 `usage_limit_reached`·`usageLimitExceeded`·`rateLimitExceeded` |
| `ModelRerouted(ModelName)` | `assistant.message.model`이 초기 설정 `model`과 다를 때(`result.modelUsage`는 `assistant` 미관찰·설정 모델 미포함 시에만 보조 키 배제 후 판정) | `model/rerouted`(`toModel`/`model`) 또는 `turn/completed.model`이 초기 `thread/start.model`과 다를 때(직전 실제 모델과 같으면 중복 미방출) | 미제공 |
| `ActualModel(ModelName)` | `assistant.message.model`(`<synthetic>` 제외) 및 `result.modelUsage`의 주 모델 키 | `model/rerouted`, `turn/completed.model` | `turn.completed.model` |
| `McpUnavailable` | 활성 상태 후속 `system`/`init`에서 다른 격리 항목과 허용 도구 부분집합이 유지된 채 **예상 MCP 서버만** 연결 끊김·오류일 때(추가 서버·추가 도구·수집 실행 MCP 출현은 `IsolationRegressed` 오류) | 활성 상태 후속 `mcpServerStatus/list`에서 **예상 MCP 서버만** 연결 끊김이고 허용 도구 부분집합이 유지될 때, 또는 예상 서버의 `mcpServer/startupStatus/updated` `failed`/`error`(낯선 서버·추가 도구는 `IsolationRegressed` 오류) | 미제공 |
| `Unknown(UntrustedText)` | 알 수 없는 최상위 `type`, 알 수 없는 `system.subtype`, `stream_event` 비텍스트 델타, `assistant`/`user` 미지원 블록, `<synthetic>` 비한도 오류, 미지 `rate_limit_info.status`, 비한도 실패 `result`·비 `"success"` `subtype` | 알 수 없는 JSON-RPC `method`/`result`(`model/list` 등)/`error`, 미지원 `item.type`, 비한도 `turn/completed` 실패·미지 `status` | `thread.started`, `turn.started`, 알 수 없는 `type`/미지원 `item.type`/비한도 `turn.failed`·`error` |

### 오류 분류 (`StreamDecodeError`)

- `EmptyLine`, `LineTooLong { byte_len }` (65,536바이트 초과), `MalformedJson { raw: UntrustedText }` (`Debug`/`Display`에서 원문 비노출), `Schema(ExpectedObject | MissingField | InvalidFieldType | UnrecognizedEnvelope)` → `InformationError::InvalidResponse(ResponseMismatch::Schema)`
- `Validation(InvalidModelName | InvalidTimestamp | InvalidUtilization | InvalidToolName | InvalidToolStatus | InvalidTokenCount | FieldTooLong)`, `Order(MissingInitialIsolationCheck | DuplicateIsolationSnapshot | EventAfterRejection | DuplicateToolCallId | UnexpectedToolResult | UnexpectedToolCompletion | UnclosedToolCall | IsolationRegressed)` → `InformationError::InvalidResponse(ResponseMismatch::Validation)`

## 정의 대기

아래 세부는 기존 문서가 확정하지 않았으므로 가장 보수적인 기본값을 적용하고 정의 대기로 남긴다.

| 항목 | 현재 안전한 기본값 | 선택지 | 사용자 영향 |
|---|---|---|---|
| 1. Claude `<synthetic>` 메시지의 `rate_limit` 외 합성 오류 분류 | `error == "rate_limit"`만 `AiEvent::RateLimited`로 매핑하고 나머지는 `AiEvent::Unknown`으로 원문 보존(어떤 경우에도 `TextDelta` 미방출) | A) 현재 유지(`Unknown` 보존, AI-02 로그 기록) / B) `authentication_failed`·`billing_error` 등 추가 합성 오류를 별도 `AiEvent` 또는 `InformationError`로 승격 | A는 알 수 없는 CLI 변경에도 추정을 피하고 오류 문구가 모델 응답으로 섞이지 않는다. B는 향후 로그인 만료 안내를 세분화할 수 있으나 `AiEvent` 또는 `InformationError` 계약 확장이 필요하다 |
| 2. 스트림 중간의 `StreamDecodeError`(`MalformedJson`·`LineTooLong` 등) 처리 정책 | 디코더가 `Err(StreamDecodeError)` 반환(`to_information_error()`로 `InvalidResponse` 변환) | A) AI-02 실행기가 즉시 스트림을 오류로 중단(닫힌 실패) / B) 깨진 줄만 기록·격리하고 스트림 계속 읽기 | A는 잘린 JSON·변조 스트림을 불완전 응답으로 오인하지 않고 안전하게 차단한다. B는 일시적 잡음에 내성이 생기나 누락된 도구 호출이나 텍스트를 놓칠 위험이 있다 |
| 3. `allowed_cli_versions` 미지정(`None`) 시 버전 검증 기본값 | 빈 문자열·공백만 거부하고 허용 집합이 주어졌을 때만 정확 일치 확인 | A) 현재 유지(AI-16이 확인된 버전 목록을 주입해 REL-8 강제) / B) `allowed_cli_versions`를 필수 인자로 만들어 미지정 자체를 금지 | A는 AI-02·AI-11 단위 테스트와 AI-16 버전 재점검을 자연스럽게 분리한다. B는 호출자가 실수로 버전 목록을 비우는 실수를 컴파일 단계에서 막는다 |
| 4. Codex `app-server` 활성 중 `mcpServer/startupStatus/updated` `failed`/`error` 처리 | 초기 대기 중이면 `IsolationChecked(Rejected(McpServers))`, 활성 중 예상 서버면 `AiEvent::McpUnavailable`, 낯선 서버면 `IsolationRegressed(McpServers)` 반환 | A) 현재 유지(초기 실패·낯선 서버는 즉시 종료, 예상 서버 실행 중 장애는 `McpUnavailable`) / B) 활성 중 예상 서버 장애도 즉시 `IsolationRegressed` 오류로 중단 | A는 실행 중 MCP 연결 끊김을 `AiEvent::McpUnavailable`로 표현하는 `architecture.md` 3.3 계약에 맞다. B는 실행 중 장애를 오류 중단으로 일원화한다 |
| 5. Codex `app-server` `turn/completed` `status == "interrupted"` 시 열린 도구 호출 처리 | `"interrupted"`는 R-03에 미명시 상태이므로 닫힌 실패를 유지해 열린 도구가 있으면 `Order(UnclosedToolCall)` 오류 반환, 없으면 `AiEvent::Unknown` 보존 | A) 현재 유지(S-09 실측에서 `turn/interrupt` 직후 `item/completed` 생략 여부를 확인한 뒤 AI-09에서 반영) / B) `"interrupted"`에서도 `"failed"`와 동일하게 열린 도구를 `ToolCall { status: Failed }`로 닫기 | A는 정상 완료 경로의 미종결 도구 누락을 실수로 가리지 않는다. S-09에서 실제 Codex CLI가 중단 시 `item/completed`를 생략함이 확인되면 AI-09에서 B로 전환한다 |

## 인계

| 받는 작업 | 인계 내용 |
|---|---|
| AI-02 | `ClaudeEventDecoder`와 `IsolationExpectation`을 `tokio::process::Command` 표준 출력 스트림에 연결, `StreamDecodeError::to_information_error()` 소비, `fixtures/ai/claude-*.jsonl` 재생 계약 테스트(회차 2 사소/nit 후속: 활성 재검증에서 예상 서버 연결 중 `mcp_server_errors > 0`의 퇴보 승격 여부 N1, `assistant` 없는 턴의 `result.modelUsage` 보조 모델 선별 N5 검토) |
| AI-03, AI-11 | 실행 프로필별 `IsolationExpectation`(분석: `--tools ""` + `mcp__app__*` + `app connected`; 수집: `WebSearch` + 빈 MCP) 구성 및 `/usage` 첫 턴 `init` 검증 |
| AI-04, AI-08 | `AiEvent::ActualModel`·`ModelRerouted`·`TokenUsage`(4종 토큰 수)를 실행 기록과 사용량 원장에 기록 |
| AI-05 | `AiEvent::Quota`·`AiEvent::RateLimited { resets_at }`의 절대 `ResetTime`을 `Clock`과 대조해 대기·재개 스케줄링 |
| AI-09 | `CodexAppServerDecoder`(기본)와 `CodexExecDecoder`(보조), `CodexIsolationExpectation`을 `codex app-server` 어댑터에 연결(실측 S-09 통과 후 활성화; 회차 2 사소/nit 후속: `CodexExecDecoder` 허용 도구 대조 추가 N2, 초기 `mcpServer/startupStatus/updated` 서버명 검사 N3, 요청 ID 기반 빈 `data: []` 응답 구분 N4, `model/rerouted` 뒤 `turn/completed.model` `ActualModel` 중복 억제 N6, 초기 미지 시동 상태 오류 변형 정리 N7 반영) |
| AI-16 | 확인된 CLI 버전 목록을 `IsolationExpectation`·`CodexIsolationExpectation`의 `allowed_cli_versions`로 전달(REL-8) |

## 실측 및 손 변형

### 커버리지 및 자동 변이

- 자동 변이(`just mutants-daily`): 대상 밖(사용자 결정 2026-11-24: 금액·수량·환율 계산, 주문 판정·실행, 비밀·권한 경계 코드에만 적용), 아래 격리 사본 손 변형으로 갈음.
- `cargo llvm-cov nextest --locked -p ai-claude-code -p ai-codex -p testkit --summary-only` 요약 (`ai-claude-code`·`ai-codex` 본체와 함께 측정; `testkit`의 `fake_ai_cli` 본체는 자식 프로세스 `env_clear()` 실행이라 `llvm-cov` 계측에 잡히지 않으며 A-11 ADR의 15건 손 변형으로 검증):

```text
Filename                                     Regions    Missed Regions     Cover   Functions  Missed Functions  Executed       Lines      Missed Lines     Cover    Branches   Missed Branches     Cover
--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
adapters/ai-claude-code/src/decoder.rs           890               104    88.31%          28                 1    96.43%         583                48    91.77%           0                 0         -
adapters/ai-claude-code/src/error.rs              70                57    18.57%           2                 0   100.00%          33                21    36.36%           0                 0         -
adapters/ai-claude-code/src/isolation.rs         346                44    87.28%          25                 3    88.00%         278                27    90.29%           0                 0         -
adapters/ai-claude-code/src/parse.rs             240                32    86.67%          20                 4    80.00%         200                29    85.50%           0                 0         -
adapters/ai-codex/src/app_server.rs             1003               171    82.95%          41                 3    92.68%         661               111    83.21%           0                 0         -
adapters/ai-codex/src/error.rs                    76                64    15.79%           2                 0   100.00%          35                24    31.43%           0                 0         -
adapters/ai-codex/src/exec_jsonl.rs              427                66    84.54%          17                 0   100.00%         259                28    89.19%           0                 0         -
adapters/ai-codex/src/isolation.rs               351                51    85.47%          19                 1    94.74%         289                27    90.66%           0                 0         -
adapters/ai-codex/src/parse.rs                   312                49    84.29%          25                 5    80.00%         228                42    81.58%           0                 0         -
testkit/src/bin/fake_ai_cli.rs                    46                46     0.00%           3                 3     0.00%          34                34     0.00%           0                 0         -
testkit/src/fake_ai_cli.rs                       454               252    44.49%          40                19    52.50%         308               173    43.83%           0                 0         -
testkit/src/fake_ai_cli_replay.rs                126               126     0.00%           6                 6     0.00%          65                65     0.00%           0                 0         -
testkit/src/fake_mcp_client.rs                  1583              1583     0.00%          80                80     0.00%        1070              1070     0.00%           0                 0         -
testkit/src/http_server.rs                       263               263     0.00%          23                23     0.00%         151               151     0.00%           0                 0         -
testkit/src/lib.rs                                68                 0   100.00%           7                 0   100.00%          41                 0   100.00%           0                 0         -
testkit/src/scripted_http_server.rs              230               230     0.00%          23                23     0.00%         130               130     0.00%           0                 0         -
--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------
TOTAL                                           6485              3138    51.61%         361               171    52.63%        4365              1980    54.64%           0                 0         -
```

### 격리 사본 손 변형 20건 (AI-01 몫, 묶음 총 35/35 검출, 생존 0, 컴파일 불가 0)

`target/AI-01/mutations/copy`에서 각 변형을 하나씩 적용해 `--no-run` 컴파일 성공 후 `cargo nextest run --locked`로 검출을 확인하고 원복·해시 대조·기준 테스트 재통과 후 임시 사본을 삭제했다. 1차 구현 직후 실행에서는 #14(`exec` `turn.completed` 미종결 도구 검사 제거)가 `finish()` 단계에 가려 생존해 회귀 테스트 2건을 보강했고, 1차 리뷰·비판 수정 뒤 새 수정 경로(#28~#35)를 추가한 실행에서는 #30(Codex 활성 재검증에서 비정상 예상 서버+비허용 도구 조합)이 생존해 해당 회귀를 보강한 뒤 재실행에서 35/35 전건 검출을 확인했다.

| 번호 | 파일 | 바꾼 것 | 결과 | 검출한 테스트 |
|---|---|---|---|---|
| 1 | `crates/adapters/ai-claude-code/src/isolation.rs` | apiKeySource 검증 생략 | 컴파일 성공·검출 | `ai-claude-code::event_model tests::every_isolation_failure_cause_is_rejected_and_blocks_following_events` |
| 2 | `crates/adapters/ai-claude-code/src/isolation.rs` | permissionMode dontAsk 검증 생략 | 컴파일 성공·검출 | `ai-claude-code::event_model tests::every_isolation_failure_cause_is_rejected_and_blocks_following_events` |
| 3 | `crates/adapters/ai-claude-code/src/isolation.rs` | mcp_server_errors 1건 허용 | 컴파일 성공·검출 | `ai-claude-code::event_model tests::duplicate_tools_plugin_errors_and_mcp_errors_are_rejected` |
| 4 | `crates/adapters/ai-claude-code/src/isolation.rs` | 중복 도구 목록 검사 제거 | 컴파일 성공·검출 | `ai-claude-code::event_model tests::duplicate_tools_plugin_errors_and_mcp_errors_are_rejected` |
| 5 | `crates/adapters/ai-claude-code/src/decoder.rs` | init 이전 이벤트 차단 제거 | 컴파일 성공·검출 | `ai-claude-code::event_model tests::events_before_initial_isolation_check_are_rejected` |
| 6 | `crates/adapters/ai-claude-code/src/decoder.rs` | 격리 거절 뒤 후속 이벤트 차단 제거 | 컴파일 성공·검출 | `ai-claude-code::event_model tests::active_recheck_blocks_sec2_regressions_and_allows_real_mcp_disconnect` |
| 7 | `crates/adapters/ai-claude-code/src/parse.rs` | cache_read와 cache_creation 토큰 뒤바꿈 | 컴파일 성공·검출 | `ai-claude-code::event_model tests::all_eleven_ai_event_variants_are_produced_distinctly` |
| 8 | `crates/adapters/ai-codex/src/isolation.rs` | 분석 실행 networkAccess 허용 차단 제거 | 컴파일 성공·검출 | `ai-codex::event_model tests::codex_fail_closed_status_network_and_exec_rejection` |
| 9 | `crates/adapters/ai-codex/src/isolation.rs` | Codex MCP 상태 누락 시 ready로 기본 허용 | 컴파일 성공·검출 | `ai-codex::event_model tests::codex_fail_closed_status_network_and_exec_rejection` |
| 10 | `crates/adapters/ai-codex/src/isolation.rs` | multiAgentMode 활성 차단 제거 | 컴파일 성공·검출 | `ai-codex::event_model tests::codex_isolation_rejections_terminate_stream_and_block_subsequent_lines` |
| 11 | `crates/adapters/ai-codex/src/app_server.rs` | ordinaryUsageAllowed null을 Allowed로 오인 | 컴파일 성공·검출 | `ai-codex::event_model tests::all_eleven_ai_event_variants_and_null_ordinary_usage_are_handled` |
| 12 | `crates/adapters/ai-codex/src/parse.rs` | 한 자리 usedPercent 소수점 이동 오류(0.0x→0.x) | 컴파일 성공·검출 | `ai-codex::event_model tests::all_eleven_ai_event_variants_and_null_ordinary_usage_are_handled` |
| 13 | `crates/adapters/ai-codex/src/app_server.rs` | 고아 mcpToolCall 완료 오류 무시 | 컴파일 성공·검출 | `ai-codex::event_model tests::codex_validation_and_order_errors_are_rejected` |
| 14 | `crates/adapters/ai-codex/src/exec_jsonl.rs` | exec turn.completed 미종결 도구 검사 제거 | 컴파일 성공·검출 | `ai-codex::event_model tests::exec_and_app_server_turn_completed_reject_unclosed_tool_before_finish` |
| 28 | `crates/adapters/ai-claude-code/src/isolation.rs` | Claude 활성 재검증 낯선 MCP 서버 차단 제거 | 컴파일 성공·검출 | `ai-claude-code::event_model tests::active_recheck_blocks_sec2_regressions_and_allows_real_mcp_disconnect` |
| 29 | `crates/adapters/ai-claude-code/src/decoder.rs` | Claude <synthetic> 조기 반환 제거(TextDelta 누출) | 컴파일 성공·검출 | `ai-claude-code::event_model tests::recorded_rate_limit_fixture_preserves_waiting_quota_and_reset_time` |
| 30 | `crates/adapters/ai-codex/src/isolation.rs` | Codex 활성 재검증 비허용 도구 부분집합 검사 제거 | 컴파일 성공·검출 | `ai-codex::event_model tests::active_codex_recheck_blocks_extra_server_and_extra_tool` |
| 31 | `crates/adapters/ai-codex/src/app_server.rs` | Codex 초기 thread/start 중복 스냅숏 거부 제거 | 컴파일 성공·검출 | `ai-codex::event_model tests::duplicate_startup_snapshots_active_thread_start_and_model_list_are_handled` |
| 32 | `crates/adapters/ai-codex/src/app_server.rs` | rateLimitReachedType 비문자열 타입 거부 대신 도달 오인 | 컴파일 성공·검출 | `ai-codex::event_model tests::rate_limit_schema_secondary_window_and_pre_init_status_are_strict` |
| 33 | `crates/adapters/ai-codex/src/exec_jsonl.rs` | CodexExecDecoder::decode_line 초기 거절 즉시 반환 제거 | 컴파일 성공·검출 | `ai-codex::event_model tests::exec_direct_decode_line_emits_initial_rejection_and_finish_checks_init` |

## 결과

- `ai-claude-code`와 `ai-codex`가 `application::AiEvent` 11개 변형을 기존 테스트 변경 없이 공통으로 생산하며, 알 수 없는 이벤트는 `AiEvent::Unknown`으로, 깨진 줄·스키마·검증·순서 오류는 `StreamDecodeError`로 구분 보존한다.
- 새 외부 패키지 없이(`deny.toml` 무변경) `fixtures/ai/*.jsonl` 비식별 합성 자료로 두 제공자 호환성과 속성 테스트를 고정했다.
- 바뀌면 다시 볼 조건: 실제 Claude Code / Codex CLI 업그레이드로 `stream-json` 또는 `app-server` 필드 이름이 바뀌거나(REL-8), 정의 대기 1~5번이 다른 선택지로 확정될 때.
