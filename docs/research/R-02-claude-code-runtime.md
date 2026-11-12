# R-02 Claude Code를 앱에서 구동하기

- 조사일: 2026-11-11 (KST)
- 대상 버전: Claude Code CLI 2.1.322 (`claude --version`으로 확인)
- 방법: code.claude.com 공식 문서, Anthropic 약관·지원 문서를 1차 출처로 썼다. 이 PC에서 `claude` CLI를 짧은 질문(대부분 `haiku`)으로 20번 남짓 실제로 돌렸다. 실험 파일은 세션 scratchpad의 `research/R-02/` 아래에만 두었다.
- 실험 환경: 사용자 전역 훅(SessionStart 등 17종), 플러그인 20개 넘게, 사용자 스킬이 설치된 계정 두 개. 하나는 주간 한도를 다 쓴 상태여서 한도 신호를 실제로 관찰할 수 있었다.

## 1. 요약

1. Rust 앱에서는 Agent SDK 대신 **`claude -p`를 `stream-json` 입출력으로 직접 띄우는 방식**이 맞다. SDK는 TS/Python 전용이고, 내부에서 같은 `claude` 바이너리를 띄운다. 구독 인증 정책도 CLI보다 불리하다.
2. 설계 문서에 적힌 명령(`--mcp-config … --strict-mcp-config --tools WebSearch`)만으로는 **격리가 안 된다**. 사용자 훅 8개, 플러그인 22개, 프로젝트 CLAUDE.md, 프로젝트 훅, 사용자 언어 설정이 모두 섞여 들어왔다(실측). 여기에 `--restricted --disable-slash-commands`와 권한 플래그를 더하고, **앱 전용 `CLAUDE_CONFIG_DIR`**를 쓰고, 환경 변수를 비우면 격리된다(실측).
3. 격리 검증은 `system/init` 이벤트로 매번 할 수 있다. 다만 init은 첫 사용자 메시지를 보낸 뒤에야 나온다. 첫 메시지로 `/usage`를 보내면 **모델 호출 없이(비용 0, num_turns 0)** init을 받는다. 이것으로 사전 점검을 할 수 있다(실측).
4. 한도: 요청마다 `rate_limit_event`가 `status`, `resetsAt`, `rateLimitType`, `utilization`을 준다. 한도에 걸리면 결과가 `is_error: true`, `api_error: "usage_limit_reached"`, `api_error_status: 429`로 오고 종료 코드는 1이다(실측).
5. 약관: 사용자가 Anthropic의 로그인 절차로 수정하지 않은 `claude` 바이너리에 로그인하고 본인 PC에서 본인만 쓰는 것은 공식 문서의 허용 문구에 가장 가깝다. 하지만 "자동화된 접근" 금지 조항(소비자 약관 3.7)과 "일반적·개인적 사용" 전제가 있어서, **한도를 다 쓰려고 예약 작업을 계속 돌리는 설계(D2)는 위험**하다. 판정은 **조건부**다.

## 2. 질문별 답

### Q1. 비대화형 실행의 입출력 규약

**실행 형태**: `claude -p --verbose --input-format stream-json --output-format stream-json [--include-partial-messages]`. stdin과 stdout이 모두 줄 단위 JSON(NDJSON)이다. stdin을 열어 두면 한 프로세스에서 여러 턴을 주고받을 수 있다(실측: 한 프로세스에 메시지 2개를 보내 결과 2개를 받음). [headless], [cli-reference]

**입력 메시지**(stdin 한 줄):
```json
{"type":"user","message":{"role":"user","content":"..."},"parent_tool_use_id":null}
```
SDK 타입 `SDKUserMessage`에는 `priority`("now"|"next"|"later") 같은 선택 필드도 있다. [sdk-ts]
`{"type":"control_request","request_id":"…","request":{"subtype":"initialize"}}` 같은 제어 요청도 받는다(실측: `control_response`가 돌아옴). 하지만 SDK 내부 프로토콜이고 CLI 문서에는 규약이 없다. **비공식이므로 의존하지 않는다.**

**출력 메시지 종류**(실측과 [sdk-ts], [headless]):

| type / subtype | 내용 | 비고 |
|---|---|---|
| `system/init` | `session_id`, `cwd`, `tools[]`, `mcp_servers[{name,status,source}]`, `model`, `permissionMode`, `slash_commands[]`, `skills[]`, `plugins[{name,path}]`, `plugin_errors?`, `mcp_server_errors?`, `agents[]`, `apiKeySource`, `claude_code_version`, `capabilities[]`, `effort?` | **턴마다** 나온다(실측: 2턴이면 2번) |
| `system/hook_started`, `hook_response` | 훅 실행 | SessionStart 훅은 항상 나온다. 나머지는 `--include-hook-events`를 켜야 나온다 |
| `system/status` | `"status":"requesting"` 등 | 실측 |
| `system/api_retry` | `attempt`, `max_retries`, `retry_delay_ms`, `error_status`, `error`(`rate_limit`, `overloaded`, `authentication_failed` …) | [headless] |
| `system/permission_denied` | `tool_name`, `tool_use_id`, `decision_reason` | 최선 노력으로만 보낸다. 확정 기록은 result의 `permission_denials`다 [sdk-ts] |
| `system/compact_boundary` | `compact_metadata{trigger:"manual"\|"auto", pre_tokens}` | [sdk-ts] |
| `stream_event` | Messages API 스트림 이벤트를 그대로 감싼 것. `message_start`, `content_block_start`(tool_use 포함), `content_block_delta`(`text_delta`, `input_json_delta`), `message_delta`, `message_stop` | `--include-partial-messages`일 때만 나온다. 실측 |
| `assistant` | 완성된 메시지. `content[]`에 `text`와 `tool_use{id,name,input}`. 서브에이전트 메시지면 `parent_tool_use_id`가 붙는다 | 한도에 걸리면 `"model":"<synthetic>"`, `"error":"rate_limit"` (실측) |
| `user` | `tool_result` 블록(도구 결과) | 실측 |
| `rate_limit_event` | `rate_limit_info{status, resetsAt, rateLimitType, utilization, overageStatus, isUsingOverage, unifiedWindows{…}}` | 실측. Q7 참고 |
| `result` | `subtype`("success" 또는 `error_max_turns`, `error_during_execution`, `error_max_budget_usd`, `error_max_structured_output_retries`), `is_error`, `result`, `session_id`, `num_turns`, `total_cost_usd`, `usage`, `modelUsage`, `permission_denials[]`, `terminal_reason`, `api_error_status?`, `structured_output?` | 스트림의 마지막 줄. 실측에서 한도에 걸렸을 때는 **subtype이 "success"인데 is_error가 true**였다 |

**도구 호출 이벤트**: `stream_event`의 `content_block_start`(type `tool_use`, `name: "mcp__app__get_portfolio_stub"`), `assistant`의 `tool_use` 블록, 이어서 `user`의 `tool_result` 블록 순서로 나온다(실측, 10절 출력 참고).

**종료 코드**: 성공하면 0, 실패하면 0이 아니다. 잘못된 플래그는 실행 전에 stderr로 알리고, 실행 중 실패(인증 누락 등)는 stdout의 result에 담는다. SIGTERM으로 끝내면 143이고 진행 중이던 턴은 결과 없이 남는다. 턴만 끝내려면 SIGINT를 쓴다. [headless] 실측: 주간 한도에 걸린 실행은 exit 1, 정상 실행은 0.

### Q2. CLI 직접 실행과 Agent SDK의 차이

| 항목 | CLI 직접 (`claude -p`) | Agent SDK |
|---|---|---|
| 언어 | 어느 언어든 (자식 프로세스와 NDJSON) | TypeScript, Python만 [sdk-overview] |
| 실체 | `claude` 바이너리 | "A library that runs the Claude Code binary" [sdk-overview]. 결국 같은 바이너리를 띄운다 |
| 공식 안내 | "To drive the same agent loop from a language other than Python or TypeScript, run the CLI as a subprocess" [sdk-overview] | — |
| 기능 | 플래그, stream-json, 공식 문서에 있는 이벤트 | 위 기능에 더해 `canUseTool` 콜백, 프로세스 안 MCP(`type:"sdk"`), `interrupt()`, `getContextUsage()` 같은 제어 요청 |
| 구독 인증 | 사용자가 수정하지 않은 바이너리에 직접 로그인. 문서에 허용 문구가 있다 (Q8) | 개요 문서: "Unless previously approved, Anthropic does not allow third party developers to offer claude.ai login or rate limits for their products, including agents built on the Claude Agent SDK." [sdk-overview] 지원 문서는 "Claude Agent SDK, `claude -p` … still draw from your subscription limits"라고 해서 기술적으로는 된다 [support-sdk] |
| 약관 | Claude Code 약관 (개인 플랜은 소비자 약관) | "Use of the Claude Agent SDK is governed by Anthropic's Commercial Terms" [sdk-overview] |
| Rust 앱에서 | `tokio::process`로 바로 다룬다 | Node 또는 Python 사이드카가 하나 더 필요하다 |

Rust용 공식 SDK는 없다. 커뮤니티 crate(`claude-agent-sdk-rs` 0.3.2, `apiari-claude-sdk` 0.2.0, `ccwire` 0.1.2 등)가 stream-json을 감싼다. 유지 상태와 품질은 미확인이다. [crates-search]
**결론**: CLI 직접 실행을 쓴다. 이벤트 타입은 SDK의 TS 타입 정의(`SDKMessage`)를 참고 스펙으로 삼아 Rust serde 타입으로 직접 정의하고, 모르는 필드와 타입은 무시하도록 만든다.

### Q3. 도구 제한과 격리

- `--tools WebSearch`: 내장 도구 중 WebSearch만 남긴다. 실측 init의 `tools`는 `["WebSearch","mcp__app__get_portfolio_stub"]`였다. `--tools`는 MCP 도구에는 영향이 없다. [cli-reference]
- `--disallowedTools "mcp__*"`처럼 이름만 쓴 거부 규칙은 해당 도구를 맥락에서 제거한다. [cli-reference]
- **권한**: `-p`에서 승인이 필요한 호출은 승인 주체가 없으면 거부된다. 실측에서 `--allowedTools` 없이 돌리자 MCP 도구 호출이 `permission_denials`로 거부됐다. 따라서 `--allowedTools mcp__app__<도구>`(필요하면 `WebSearch`도) + `--permission-mode dontAsk` + `--permission-prompts none`을 쓴다. [headless]
- **섞여 들어오는 것과 막는 옵션**(실측 결과):

| 실행 | 사용자 훅 | 플러그인 | 사용자 스킬 | 프로젝트 CLAUDE.md | 프로젝트 `.claude/settings.json` 훅 | 프로젝트 `.mcp.json` | `--mcp-config` 서버 |
|---|---|---|---|---|---|---|---|
| 설계 문서 명령 그대로 (`--strict-mcp-config --tools WebSearch`) | **실행됨**(SessionStart 8개) | **22개 로드** | 로드 | **로드**(카나리 단어를 답함) | **실행됨**(카나리 파일 생성) | 무시 | 연결 |
| `--safe-mode` 추가 | — | init에 나열됨 | — | 미로드 | 미실행 | — | **연결 안 됨**(mcp=[]) → 쓸 수 없음 |
| `--restricted` 추가 | 없음 | 내장 4개만(`cc-plugin-*`) | **사용자 스킬 1개가 남음** | 미로드 | 미실행 | 무시 | 연결 |
| `--restricted --disable-slash-commands` | 없음 | 내장 4개 | 없음(skills=[]) | 미로드 | 미실행 | 무시 | 연결 |

- `--restricted`(v2.1.248 이상): 명령·코드 실행 도구와 WebFetch를 없애고, user·project·local 설정 파일을 무시한다. managed 설정과 `--settings`는 적용된다. 파일 도구는 작업 폴더 안으로 묶고, `bypassPermissions`를 거부한다. [cli-reference]
- `--setting-sources`는 따로 실험하지 않았다. `--restricted`가 상위 기능이다.
- **환경 변수가 새는 경로**: 이 PC의 부모 프로세스 환경에 `CLAUDE_CODE_PLUGIN_DIRS`, `CLAUDE_CONFIG_DIR`, `CLAUDECODE` 같은 값이 있었다. 문서상 `ANTHROPIC_API_KEY`가 있으면 구독 대신 API 키가 쓰인다(인증 우선순위 3번, 구독은 7번). [auth] 그래서 **`env -i`로 비운 뒤 허용 목록만 넘긴다**. 실측: `HOME`, `PATH`만 넘기면 키체인을 읽지 못해 `loggedIn:false`였다. **`USER`(그리고 `LOGNAME`)까지 넘겨야** 구독 로그인이 읽혔다.
- **앱 전용 `CLAUDE_CONFIG_DIR`**: 문서상 설정 디렉터리마다 설정, 세션 기록, 로그인이 따로 있고, macOS 키체인 항목도 디렉터리별로 따로 둔다. [auth] 실측에서 새 디렉터리는 `loggedIn:false`였고, 키체인에는 `Claude Code-credentials-<해시>` 형태의 항목이 디렉터리마다 있었다. 앱 전용 디렉터리를 쓰면 사용자 스킬, 플러그인 캐시, 메모리, 대화 기록이 원천적으로 분리된다. 사용자는 이 디렉터리를 지정한 상태로 `claude auth login`을 한 번 실행해 Anthropic의 로그인 절차를 직접 거친다. 이 로그인은 실험하지 않았다(사용자 계정 조작이 되기 때문).
- `--bare`는 OAuth와 키체인을 읽지 않으므로 구독으로 쓸 수 없다. [headless] `CLAUDE_CODE_OAUTH_TOKEN`도 읽지 않는다. [auth]

### Q4. MCP 연결

- `--mcp-config <파일 또는 JSON 문자열>`과 `--strict-mcp-config`를 함께 쓰면 지정한 서버만 쓴다. 실측: 프로젝트 `.mcp.json`의 서버는 무시됐다. [cli-reference]
- 형식: `{"mcpServers":{"app":{"type":"stdio","command":…,"args":[…],"env":{…}}}}` 또는 `{"type":"http","url":…,"headers":{"Authorization":"Bearer ${VAR}"}}`. `type`은 `stdio`, `http`(=`streamable-http`), `sse`(폐기 예정), `ws`를 받는다. `url`이 있는데 `type`이 없으면 설정 오류가 되고, 그 항목은 건너뛰어 `mcp_server_errors`에 담긴다. [mcp], [headless]
- `${VAR}` 확장은 `command`, `args`, `env`, `url`, `headers`에서 된다. 원격 `url`/`headers`에서는 `ANTHROPIC_API_KEY` 같은 자격 증명 이름이 빈 값으로 읽힌다. 앱 고유 변수 이름을 쓰면 된다. `headersHelper`(10초 제한, 연결마다 실행)도 있다. [mcp]
- **실측 두 가지 모두 성공**:
  - stdio: Node로 만든 최소 서버(JSON-RPC 직접 구현)를 연결해 도구를 호출했다.
  - 로컬 HTTP: `127.0.0.1:47123`에 Bearer 토큰을 검사하는 서버를 띄우고, 실행마다 만든 토큰을 `APP_MCP_TOKEN` 환경 변수 → `headers`의 `${APP_MCP_TOKEN}`으로 넘겼다. 서버 로그에서 요청 순서는 `server/discover` → `initialize` → `notifications/initialized` → GET(스트림 시도, 405 응답) → `tools/list` → `tools/call`이었고, 모든 요청에 올바른 토큰이 실려 있었다.
- **주의(실측)**: MCP 서버가 연결에 실패해도 실행은 계속된다. init에 `"status":"failed"`가 찍히고 모델은 그대로 답했다. 앱이 init을 검사해서 중단해야 한다.
- `-p`와 `--mcp-config`를 함께 쓰면 첫 턴 전에 서버 연결을 `MCP_TIMEOUT`(기본 30초)까지 기다린다. 도구 출력은 기본 25,000토큰에서 잘리고(`MAX_MCP_OUTPUT_TOKENS`), 10,000토큰부터 경고한다. [mcp], [headless]
- stdio와 HTTP 중 어느 쪽을 쓸지는 R-04에서 정한다. 두 방식 모두 CLI 쪽 지원은 확인됐다.

### Q5. 세션

- 이어가기: `--session-id <uuid>`로 시작하고 `--resume <id>`로 이어 간다. 실측: 1턴에서 준 단어를 2턴(별도 프로세스)에서 기억했다. `--resume`은 transcript의 절대 경로도 받는다. [sessions]
- 저장 위치: `$CLAUDE_CONFIG_DIR/projects/<작업 폴더 경로의 영숫자 외 문자를 '-'로 바꾼 것>/<session-id>.jsonl`. 기본 보존 기간은 30일(`cleanupPeriodDays`)이다. [sessions] 실측: 파일 권한 0600, 1턴에 44KB. 대화 원문이 평문으로 들어 있고, `api-request-blob`, `cost-state`, `queue-operation` 같은 내부 항목도 함께 있다. 문서는 이 형식이 내부용이고 버전마다 바뀐다고 경고한다. [sessions]
- `-p`와 SDK 세션은 `/resume` 선택 목록과 `--continue`에서 빠지지만 ID로는 이어 갈 수 있다. 같은 세션을 두 프로세스에서 이어 가면 기록이 섞인다. [sessions]
- 이어 갈 때 `--mcp-config`, `--settings`, `--add-dir`는 복원되지 않으므로 다시 넘겨야 한다. [sessions] 시스템 프롬프트는 첫 요청 때 기록해 두고 재사용한다(`--system-prompt-snapshot` 기본값 on). [cli-help]
- 맥락 압축: 한계에 가까워지면 자동으로 압축하고 `compact_boundary` 이벤트를 낸다. 1M 맥락 모델은 기본 약 967K 토큰에서 압축한다. `--autocompact <auto|100k~1M>`로 바꿀 수 있다. 이어 갈 때는 마지막 압축 요약과 그 뒤의 메시지만 불러온다. [model-config], [context-window], [sessions]

### Q6. 모델 선택

- 실행마다 `--model <별칭|전체 이름>`과 `--effort low|medium|high|xhigh|max`를 준다. 별칭: `opus`(Opus 5.5), `sonnet`(Sonnet 5.5), `haiku`(Haiku 5.5), `fable`(Fable 5.2), `best`, `opusplan`, `[1m]` 접미사. 별칭이 가리키는 모델은 시간이 지나면 바뀌므로, 고정하려면 `claude-opus-5-5` 같은 전체 이름을 쓴다. [model-config]
- 우선순위: `CLAUDE_CODE_EFFORT_LEVEL` 환경 변수가 `--effort`보다 앞선다. 환경 변수를 비우면 문제없다. [model-config]
- 요청한 effort를 모델이 지원하지 않으면 지원하는 가장 높은 단계로 내려간다. 실측에서는 `--effort low` + haiku 조합일 때 init의 `effort`가 null이었다.
- 구독 등급별로 쓸 수 있는 모델: 문서에 표가 없다. 확인된 것은 기본 모델이 Pro, Max, Ultra, Team, Enterprise 모두 Opus 5.5라는 점, Fable은 플랜과 좌석 등급에 따라 사용량 크레딧으로 과금될 수 있다는 점, Opus 4.6 1M은 Pro에서 크레딧이 필요하다는 점뿐이다. [model-config] **등급별 정확한 목록은 미확인이다.** 실측: 비공식 제어 요청 `initialize`의 응답 `models[]`에 이 계정에서 고를 수 있는 모델 목록(`default, opus, fable, sonnet, haiku, claude-opus-5, …`)이 들어 있었다.
- 내용 기반 자동 폴백이 있다. 예를 들어 Opus 5.5에서 사이버보안으로 분류된 요청은 Opus 4.8로 다시 실행한다. 폴백한 뒤에는 세션이 그 모델로 계속 간다. 앱은 `assistant.message.model`이나 result의 `modelUsage`로 실제 쓴 모델을 기록해야 한다. [model-config]

### Q7. 구독 사용량 한도

- 구조: 5시간 단위의 굴러가는 세션 한도와 주간 한도가 동시에 차감된다. 모델별 주간 한도(Opus, Sonnet, Fable)도 따로 있을 수 있다. [errors]
- **한도 신호(실측)**:
  - 요청마다 `{"type":"rate_limit_event","rate_limit_info":{"status":"allowed","resetsAt":1794906000,"rateLimitType":"seven_day","utilization":…,"unifiedWindows":{"seven_day":{"utilization":0.07,"resetsAt":…,"status":"allowed"}}}}`가 나온다.
  - 한도에 걸리면 `"status":"rejected"`, `"surpassedThreshold":1`이 되고, assistant 텍스트는 `"You've hit your weekly limit · resets 8pm (Asia/Seoul)"`(합성 메시지)다. result는 `is_error:true`, `api_error:"usage_limit_reached"`, `api_error_status:429`, `terminal_reason:"api_error"`, `total_cost_usd:0`이고 exit 1로 끝난다.
- 남은 한도를 알아내는 방법:
  1. 실행 중 `rate_limit_event`의 `utilization`과 `resetsAt`(유닉스 초)를 읽는다. 필드 이름은 문서에 없고 실측으로만 확인했다.
  2. `claude -p "/usage"`는 모델 호출 없이(비용 0) 텍스트를 준다. 예: `Current week (all models): 7% used · resets Nov 17 at 6pm (Asia/Seoul)`. 사람이 읽는 텍스트라 파싱하기 쉽지 않다. `--disable-slash-commands`를 주면 쓸 수 없다(실측).
  3. 상태 줄 입력의 `rate_limits.five_hour/seven_day.used_percentage`와 `resets_at` 필드는 구조화되어 있지만 상태 줄 명령에만 들어온다. [statusline] `-p`에서 쓸 수 있는지는 미확인이다.
- 실측에서 `unifiedWindows`에는 `seven_day`만 있었다(5시간 창이 없음). 계정이나 플랜에 따라 달라지는 것으로 보이지만 미확인이다.
- 무인 실행용 `CLAUDE_CODE_RETRY_WATCHDOG=1`은 429와 529를 무한히 재시도하는 옵션이다. 예약 작업 큐는 앱이 직접 관리하므로 켜지 않는다. [errors]
- `--max-budget-usd`는 클라이언트 쪽 비용 추정에 거는 상한이라 구독 한도와는 관계가 없다. [cli-reference]

### Q8. 약관 (판정: 조건부. 최종 판정은 R-13에서)

- 소비자 약관(시행 2025-10-08) 3조 7호: "Except when you are accessing our Services via an Anthropic API Key or where we otherwise explicitly permit it, to access the Services through automated or non-human means, whether through a bot, script, or otherwise." [consumer-terms]
- Claude Code 법률 문서:
  - "OAuth authentication is intended exclusively for purchasers of Claude Free, Pro, Max, Ultra, Team, and Enterprise subscription plans and is designed to support ordinary use of Claude Code and other native Anthropic applications."
  - "Anthropic does not permit third-party developers to offer Claude.ai login into their own applications, or to route requests through Free, Pro, Max, or Ultra plan credentials on behalf of their users. Moreover, developers may not collect, store, or intermediate Claude.ai credentials or session tokens"
  - "Nor does it prevent an end user from signing in to the unmodified Claude Code binary with their own Claude subscription"
  - "Advertised usage limits for Pro, Max, and Ultra plans assume ordinary, individual usage of Claude Code and the Agent SDK." [legal]
- 지원 문서: "Claude Agent SDK, `claude -p`, and third-party app usage still draw from your subscription limits"(2026-06-15에 Agent SDK 관련 변경을 보류). [support-sdk]
- 2026-02에 Anthropic은 구독 OAuth 토큰을 Claude Code와 claude.ai 밖에서 쓰는 것을 금지한다고 명확히 했다(언론 보도). [register]
- **해석**: 이 앱은 (1) 수정하지 않은 `claude` 바이너리를 쓰고, (2) 사용자 본인이 Anthropic 로그인 절차로 로그인하고, (3) 앱이 토큰을 읽거나 저장하지 않고, (4) 다른 사람에게 배포하지 않는다. 이것은 허용 문구에 가장 가깝다. 반면 예약 작업이 사람 없이 도는 것은 3.7의 "automated … means"에 걸릴 수 있다. CLI가 `-p` 무인 실행을 공식 기능으로 문서화하고 지원 문서가 구독 차감을 명시한 것을 "explicitly permit"으로 볼 수 있는지는 해석이 갈린다. 예약 작업을 계속 돌리는 방식을 제한하는 명시 조항은 찾지 못했다. 다만 "ordinary, individual usage" 전제가 있고, Anthropic은 "may do so without prior notice"라며 사전 통지 없는 제재를 예고했다. [legal]
- `claude setup-token`(1년짜리 토큰)을 앱 키체인에 저장하는 방식은 "collect, store … credentials"에 걸릴 수 있으므로 쓰지 않는다.

### Q9. 동시 실행

- 구독에서 동시에 띄울 수 있는 실행 수의 공식 한도는 **미확인**이다. 문서는 429가 나면 동시성을 줄이라고만 권한다. [errors]
- 실측 `/usage` 출력에 "Run fewer sessions at once · up to 6 at once"라는 권고가 있었다. 이것은 이 계정의 사용 패턴 분석이지 한도가 아니다.
- 동시 실행은 같은 5시간·주간 한도를 함께 깎는다. 같은 세션 ID를 두 프로세스가 쓰면 기록이 섞인다. [sessions]
- 권장: 채팅 1개와 예약 작업 1개를 넘기지 않는다. 프로세스마다 고유 세션 ID와 고유 MCP 토큰을 쓴다. 채팅이 시작되면 예약 작업에 SIGINT를 보내 턴을 정리한다.

### Q10. 프롬프트 주입 방어에 쓸 수 있는 CLI 기능

- 도구 화이트리스트: `--tools WebSearch` 하나만 두고, `--restricted`로 실행 도구와 WebFetch를 막는다. 승인되지 않은 호출은 `dontAsk`와 `--permission-prompts none`으로 거부되고 `permission_denials`에 남는다. [cli-reference], [headless]
- WebFetch는 원문 대신 별도 모델의 요약을 넘긴다. [security] 이 앱은 WebFetch를 끄고, 기사 본문은 앱의 `read_article` MCP 도구가 수집 목록에 있는 주소만 반환한다.
- `--json-schema`로 결과 형식을 강제할 수 있다(`structured_output`). [headless]
- `--system-prompt-file`로 기본 시스템 프롬프트(코딩용)를 앱 프롬프트로 바꿀 수 있다. 이 앱에 맞지만 실측하지는 않았다.
- `--disallowedTools`로 MCP 도구 단위 거부가 된다. 하지만 근본 방어는 MCP 서버 쪽이다. 제안만 받고 승인하지 않으며, 주문 전송 도구는 아예 없다.
- `--restricted`에서도 `--settings`는 적용되므로 PreToolUse 훅으로 MCP 호출 인자를 검사할 수 있을 것으로 보인다. 그러나 문서상 `--settings`로 넘긴 훅이 `--restricted`에서 실행되는지는 **미확인**이다.
- init의 `capabilities`에 `mcp_call_result_not_for_model_v1`이 보이지만 문서가 없어서 의미는 미확인이다.

### Q11. 격리 옵션과 구독 인증의 공존

| 옵션 | 구독 인증 | 실측 효과 |
|---|---|---|
| `--bare` | **안 된다**(OAuth와 키체인을 읽지 않는다) [headless] | 실험하지 않음 |
| `--safe-mode` | 된다 | `--mcp-config` 서버까지 꺼져서(mcp=[]) **이 앱에 쓸 수 없다** |
| `--restricted` | **된다**(실측 `apiKeySource:none`, 구독 한도 이벤트 수신) | 훅, 플러그인, 설정, CLAUDE.md, 프로젝트 MCP 차단. 사용자 스킬 1개는 남음 |
| `--disable-slash-commands` | 된다 | 스킬과 명령 0개 |
| `--setting-sources` | 실험하지 않음 | — |
| 앱 전용 `CLAUDE_CONFIG_DIR` | 별도 로그인이 필요하다(디렉터리별 키체인) [auth] | 새 디렉터리에서 `loggedIn:false` 확인 |

### Q12. init 보고로 격리를 매번 검증할 수 있는가

검증할 수 있다. init의 `tools`, `mcp_servers[].status`, `plugins`, `skills`, `slash_commands`, `agents`, `permissionMode`, `apiKeySource`, `model`, `claude_code_version`, `mcp_server_errors`, `plugin_errors`로 판정한다. 단, 다음을 고려해야 한다.

- init은 **첫 사용자 메시지를 받은 뒤에** 나온다(실측: 입력 없이 stdin을 닫으면 훅 이벤트만 나오고 init은 없음). 실제 질문으로 init을 받으면 그 질문은 이미 모델에 가 있다.
- **해결(실측)**: 첫 메시지로 `/usage`를 보낸다. `--disable-slash-commands`가 켜져 있으면 결과 텍스트는 `"/usage isn't available in this environment."`이지만 **init은 나오고 `num_turns:0`, `total_cost_usd:0`**이다. 이 init을 검사해서 통과할 때만 같은 프로세스에 실제 질문을 보낸다.
- 검증 규칙(예): `tools == ["WebSearch", "mcp__app__…"(기대 목록)]`, 모든 `mcp_servers[].status == "connected"`, `plugins`의 이름이 모두 `cc-plugin-` 내장(관찰값: agents-md, telemetry, privacy, plugin-authoring), `skills == []`, `slash_commands == []`, `permissionMode == "dontAsk"`, `apiKeySource == "none"`, `claude_code_version`이 허용 목록에 있음.
- `agents` 목록(claude, Explore, general-purpose, Plan, statusline-setup)은 남지만 Agent 도구가 `tools`에 없으므로 호출할 수 없다.
- 내장 플러그인 목록은 버전마다 바뀔 수 있다. 버전을 올릴 때 허용 목록을 갱신한다.

### Q13. 대화 기록의 디스크 저장과 끄는 방법

- 위치와 형식은 Q5에 적었다. 끄는 방법은 `--no-session-persistence`(`-p` 전용) 또는 `CLAUDE_CODE_SKIP_PROMPT_HISTORY`다. [sessions]
- 실측: `--no-session-persistence` 실행은 transcript를 남기지 않았다. 다만 `projects/<작업 폴더>/memory/` **빈 폴더는 생겼다**.
- 남기지 않으면 이어 가는 방법이 두 가지다.
  - (가) 앱이 SQLite에 둔 대화를 매 실행의 첫 메시지(또는 `--append-system-prompt`)에 요약이나 원문으로 다시 넣는다. 프롬프트 캐시가 깨지고 한도를 더 쓴다.
  - (나) 실측: transcript를 앱 저장소로 옮긴 뒤 `--resume <절대 경로> --no-session-persistence`로 이어 가면 이전 대화를 기억했다. 하지만 새 턴은 파일에 추가되지 않았다(읽기 전용 재생). 내부 형식에 의존하므로 권장하지 않는다.
- 권장: 채팅은 기록을 남기되 **앱 전용 `CLAUDE_CONFIG_DIR` 안에만** 두고, 앱이 정한 주기로 지운다(`cleanupPeriodDays`를 `--settings`로 지정, 또는 앱이 직접 삭제). 예약 작업은 단발이므로 `--no-session-persistence`를 쓴다. 키체인 밖에 평문이 남는 위험은 R-16에서 디스크 암호화 전제로 다룬다.

## 3. 선택지 비교표와 추천

### 3.1 구동 방식

| 선택지 | Rust 적합성 | 구독 인증 | 격리 | 공식 문서화 | 판정 |
|---|---|---|---|---|---|
| A. `claude -p` stream-json 직접 | 높음 | 문서상 가장 안전한 형태 | 플래그로 됨(실측) | 됨 | **추천** |
| B. Agent SDK(TS) 사이드카 | 낮음(Node 동봉) | SDK 문서가 구독 로그인 제공을 명시적으로 금지 | 같음 | 됨 | 비추천 |
| C. 커뮤니티 Rust crate | 중간 | A와 같음 | A와 같음 | 아님 | 참고만 |
| D. API 키(`ANTHROPIC_API_KEY`) | 높음 | 구독이 아니라 종량제 | `--bare`까지 쓸 수 있음 | 됨 | 대안 어댑터(설계 문서의 "API 키 방식") |

### 3.2 격리 구성 (추천)

```text
환경: env -i 로 비운 뒤 다음만 넘김
  HOME, PATH(claude 경로 포함), USER, LOGNAME, TMPDIR,
  CLAUDE_CONFIG_DIR=~/Library/Application Support/<앱>/claude,
  APP_MCP_TOKEN=<실행마다 새로 만든 토큰>   (HTTP 전송일 때)
작업 폴더: 앱 전용 빈 폴더
명령:
claude -p --verbose \
  --input-format stream-json --output-format stream-json --include-partial-messages \
  --restricted --disable-slash-commands \
  --tools WebSearch \
  --allowedTools WebSearch mcp__app__get_portfolio mcp__app__… \
  --permission-mode dontAsk --permission-prompts none \
  --mcp-config <앱이 만든 임시 JSON> --strict-mcp-config \
  --model <설정값> --effort <설정값> \
  [--session-id <uuid> | --resume <uuid>] [--no-session-persistence]
절차: stdin에 "/usage" → init 검사 → 통과하면 실제 질문, 실패하면 kill
```

**추천 이유**: 이 PC처럼 훅과 플러그인이 많이 깔린 환경에서 격리가 실제로 확인된 유일한 조합이고, 구독 인증으로 동작하며, 실행마다 비용 없이 검증할 수 있다.

**추천이 틀렸다고 판명될 조건**:
- 이후 버전에서 `--restricted`가 MCP나 구독 인증과 충돌하거나, 사전 점검 턴(`/usage`)이 모델을 호출하게 바뀌는 경우. 버전을 올릴 때 계약 테스트로 확인한다.
- `--bare`가 `-p`의 기본값이 되었는데 구독 인증을 계속 지원하지 않는 경우. 문서는 "will become the default for `-p` in a future release"라고 예고한다. [headless] 이때는 `--bare`를 끄는 명시 플래그가 필요하거나, API 키 방식으로 옮겨야 한다.
- Anthropic이 개인용 자동 실행에 구독을 쓰는 것을 약관이나 기술로 막는 경우. 이때는 D(API 키)로 바꾼다.
- 앱 전용 `CLAUDE_CONFIG_DIR`에 로그인하는 것이 "두 번째 기기·세션"으로 취급되어 제한되는 경우(미확인).

## 4. 위험과 미확인 항목

**위험**
- 약관: 예약 작업으로 한도를 다 쓰는 사용 방식은 "ordinary, individual usage" 전제와 충돌한다. 사전 통지 없는 제재 가능성이 있다. [legal]
- 비공식 필드 의존: `rate_limit_event` 필드, `api_error`, `control_request` 프로토콜은 문서화되지 않았거나 SDK 타입에만 있다. 모르는 값을 견디는 파서와 버전 고정이 필요하다.
- `--restricted`를 빼먹거나 환경 변수를 비우지 않으면 사용자 훅이 앱의 자식 프로세스에서 실행된다(실측). 셸 명령을 실행하는 훅이므로 SEC-2 위반이다.
- MCP 연결이 실패해도 실행은 계속된다(실측). init을 검사하지 않으면 AI가 데이터 없이 답한다.
- transcript가 평문으로 남는다(0600). 내부에 요청 원문 조각(`api-request-blob`)도 있다.
- 자동 폴백으로 설정과 다른 모델이 답할 수 있다. [model-config]
- `claude` 자동 업데이트로 동작이 바뀔 수 있다. 실측 버전은 2.1.322였고, 이 PC는 npm 전역 설치였다.

**미확인**
- 구독 등급별 사용 가능 모델의 정확한 목록, 동시 실행 한도, `unifiedWindows`에 5시간 창이 언제 나오는지.
- 상태 줄의 구조화된 `rate_limits`를 `-p`에서 얻는 방법.
- `--restricted`에서 `--settings`로 넘긴 훅이 실행되는지. 사용자 auto-memory와 `~/.claude/CLAUDE.md`가 `--restricted`에서 차단되는지(이 PC에는 사용자 CLAUDE.md가 없어 시험하지 못했다).
- `--system-prompt-file`과 `--json-schema`를 이 구성에서 함께 쓸 때의 동작.
- 앱 전용 설정 디렉터리에 로그인하는 실제 절차(실험하지 않음).
- `usage.server_tool_use.web_search_requests`가 WebSearch를 실제로 쓴 실행에서도 0으로 찍힌 이유.
- `mcp_call_result_not_for_model_v1` 같은 capability의 의미.

## 5. 이 결과로 바뀌어야 할 설계 문서 항목

| 문서·항목 | 현재 | 바꿀 내용 |
|---|---|---|
| design.md 4.1 Claude Code 실행 방식 | `--strict-mcp-config --tools WebSearch` 위주 | 3.2의 명령으로 교체. `--restricted --disable-slash-commands`, `--allowedTools`, `--permission-mode dontAsk --permission-prompts none` 추가. `env -i` 허용 목록에 `USER` 포함. 앱 전용 `CLAUDE_CONFIG_DIR` 명시 |
| design.md 4.1 "처음에 보고하는 도구·MCP 목록 검사" | 첫 보고를 검사 | "init은 첫 메시지 뒤에 나온다 → `/usage` 사전 점검 턴(비용 0)으로 검사한 뒤 실제 질문을 보낸다"로 구체화. 검사 항목에 `mcp_servers[].status`, `plugins`, `skills`, `slash_commands`, `apiKeySource`, `claude_code_version` 추가 |
| design.md 4.1 `AiProvider.usage()` | 남은 한도 추정 | 출처를 `rate_limit_event`(실행 중)와 `claude -p "/usage"` 텍스트(대기 중, 비용 0)로 명시. `resetsAt`으로 큐 재개 시각 결정 |
| design.md 4.2 대화 원본 | "CLI가 평문으로 남기지 않게" | 채팅: 앱 전용 설정 디렉터리 안에 남기고 주기적으로 삭제. 예약 작업: `--no-session-persistence`. 완전히 남기지 않으려면 앱이 기록을 다시 넣어야 해서 한도를 더 쓴다는 트레이드오프를 적는다 |
| design.md 4.3 / D2 "구독 한도를 남기지 않고 쓰는 것이 목적" | 한도 소진 목표 | 약관 위험(4절)을 반영해 "한도의 일정 비율 안에서"로 완화. AI-2 기본값 70%도 다시 검토. R-13에 넘긴다 |
| design.md 7.x REL-4 | 한도 초과는 큐 대기 | 판정 기준 명시: result의 `api_error == "usage_limit_reached"` 또는 `rate_limit_event.status == "rejected"`, 재개 시각은 `resetsAt` |
| design.md SEC-2 | 셸·파일·웹 읽기 도구 없이 | `--restricted`가 WebFetch와 실행 도구를 없애는 것까지 근거로 추가. "MCP 실패 시 실행이 계속됨 → init 검사가 필수"를 추가 |
| architecture.md AI CLI 신뢰 경계 | 비신뢰 | 자식 프로세스 환경 변수 허용 목록과 전용 설정 디렉터리를 경계의 일부로 기록 |
| architecture.md 스트림 이벤트 | — | Rust 이벤트 타입은 SDK의 `SDKMessage`를 기준으로 하되, 모르는 type과 필드는 무시하도록(`#[serde(other)]`) 만든다. 녹화한 stream-json을 테스트 고정 자료로 쓴다(R-15) |
| design.md 4.1 버전 | 2.1.322 | 자동 업데이트를 끄거나 버전을 고정하고, 버전을 올릴 때 격리 계약 테스트를 돌리는 절차를 추가 |

## 6. 실행 검증 (명령과 출력)

**환경**: 사용자 전역 훅과 플러그인이 깔린 계정, 빈 작업 폴더 `…/R-02/work`. MCP 서버는 Node로 만든 최소 stdio 서버(`get_portfolio_stub` 도구 하나)다.

**MCP 설정** (`cfg/mcp.json`):
```json
{"mcpServers":{"app":{"type":"stdio","command":"node","args":["<scratch>/R-02/mcp-stdio.mjs"],"env":{"APP_MCP_TOKEN":"dummy-run-token"}}}}
```

**stdin** (`in-final.jsonl`, 사전 점검 + 실제 질문):
```json
{"type":"user","message":{"role":"user","content":"/usage"},"parent_tool_use_id":null}
{"type":"user","message":{"role":"user","content":"Call the get_portfolio_stub tool once, then reply in one short line with the symbols and their weights. Do not use any other tool."},"parent_tool_use_id":null}
```

**명령**:
```bash
cd <scratch>/R-02/work
env -i CLAUDE_CONFIG_DIR="$CFG" HOME="$HOME" PATH="$PATH" USER="$USER" LOGNAME="$LOGNAME" TMPDIR="$TMPDIR" \
  claude -p --verbose --output-format stream-json --input-format stream-json --include-partial-messages \
  --restricted --disable-slash-commands --tools WebSearch \
  --allowedTools mcp__app__get_portfolio_stub \
  --permission-mode dontAsk --permission-prompts none \
  --mcp-config ../cfg/mcp.json --strict-mcp-config \
  --model haiku --no-session-persistence < ../in-final.jsonl
# exit=0
```

**출력 요약** (이벤트 순서. 긴 필드는 줄였다):
```text
system init tools=['WebSearch','mcp__app__get_portfolio_stub'] mcp=[{'name':'app','status':'connected','source':'dynamic'}] perm=dontAsk
assistant [('text', "/usage isn't available in this environment.")]
result success is_error=False num_turns=0 cost=0          ← 사전 점검: 모델 호출 없음
system init tools=['WebSearch','mcp__app__get_portfolio_stub'] mcp=[…connected…] perm=dontAsk
system status {"status":"requesting"}
stream_event message_start
stream_event content_block_start {"type":"tool_use","name":"mcp__app__get_portfolio_stub","input":{}}
stream_event content_block_delta {"type":"input_json_delta","partial_json":""}
assistant [('tool_use','mcp__app__get_portfolio_stub')]
stream_event content_block_stop
user tool_result [{'type':'tool_result','content':'{"holdings":[{"symbol":"TEST","weight":"0.6"},…]}'}]
stream_event message_delta {"stop_reason":"tool_use"}
stream_event message_stop
rate_limit_event {"status":"allowed","resetsAt":1794906000,"rateLimitType":"seven_day",…}
system status {"status":"requesting"}
stream_event content_block_delta {"type":"text_delta","text":"TEST "}
stream_event content_block_delta {"type":"text_delta","text":"60%, CASHX "}
stream_event content_block_delta {"type":"text_delta","text":"40%"}
assistant [('text','TEST 60%, CASHX 40%')]
stream_event message_delta {"stop_reason":"end_turn"}
result success is_error=False num_turns=2 cost=0.00027313 result=TEST 60%, CASHX 40%
```
같은 구성의 init(이 PC 실측): `plugins`=`cc-plugin-agents-md, cc-plugin-telemetry, cc-plugin-privacy, cc-plugin-plugin-authoring`, `skills=[]`, `slash_commands=[]`(0개), 훅 이벤트 0개.

**비교 실행**:
- 설계 문서 명령 그대로(`--tools WebSearch --mcp-config … --strict-mcp-config`): `system/hook_started` 8개와 `hook_response` 8개(사용자 SessionStart 훅), plugins 22개, skills 108개, `permissionMode:"auto"`(사용자 설정값).
- 위 명령에 `--disable-slash-commands`와 권한 플래그만 더하고 `--restricted`는 뺀 실행: 작업 폴더에 넣은 `CLAUDE.md`의 카나리 단어 "CANARY-MD-7781"을 모델이 답했다. `.claude/settings.json`의 UserPromptSubmit 훅이 카나리 파일을 만들었다. 답이 사용자 언어 설정(한국어)으로 나왔다. 사용자 훅 16개 이벤트와 plugins 22개도 그대로였다.
- 같은 실행에 `--restricted`를 더하자 카나리 단어를 답하지 못했고("NONE") 훅도 실행되지 않았다.

**한도 신호** (주간 한도를 다 쓴 계정, exit=1):
```json
{"type":"rate_limit_event","rate_limit_info":{"status":"rejected","resetsAt":1794394800,"rateLimitType":"seven_day","overageStatus":"rejected","isUsingOverage":false,"unifiedWindows":{"seven_day":{"utilization":1,"resetsAt":1794394800,"status":"rejected","surpassedThreshold":1}}}}
{"type":"result","subtype":"success","is_error":true,"api_error_status":429,"api_error":"usage_limit_reached","terminal_reason":"api_error","result":"You've hit your weekly limit · resets 8pm (Asia/Seoul)","total_cost_usd":0}
```

**로컬 HTTP MCP** (Bearer 토큰, `"headers":{"Authorization":"Bearer ${APP_MCP_TOKEN}"}`): 도구 호출에 성공했다. 서버 로그의 모든 요청이 `auth=ok`, `host=127.0.0.1:47123`, `origin=-`였다.

**MCP 서버가 꺼져 있을 때**: init에 `mcp=[{'name':'app','status':'failed'}]`, `tools=['WebSearch']`가 찍히고 모델은 "OK"로 정상 답했다(exit 0).

**세션 이어가기**: `--session-id <uuid>`로 1턴("PLUM-42"를 기억하라) → 별도 프로세스에서 `--resume <uuid>`로 2턴 → "PLUM-42"라고 답했다. transcript는 `$CLAUDE_CONFIG_DIR/projects/<경로>/<id>.jsonl`(0600, 44,728바이트). 실험이 끝난 뒤 transcript와 빈 폴더는 scratch로 옮기거나 지웠다.

## 7. 출처 (모두 2026-11-11 확인)

- [headless] Run Claude Code programmatically — https://code.claude.com/docs/en/headless
- [cli-reference] CLI reference — https://code.claude.com/docs/en/cli-reference
- [cli-help] `claude --help` 출력, Claude Code 2.1.322 (로컬)
- [sdk-overview] Agent SDK overview — https://code.claude.com/docs/en/agent-sdk/overview
- [sdk-ts] Agent SDK TypeScript reference (`SDKSystemMessage`, `SDKResultMessage`, `SDKUserMessage`, `SDKCompactBoundaryMessage`, `SDKPermissionDeniedMessage`) — https://code.claude.com/docs/en/agent-sdk/typescript
- [sessions] Manage sessions — https://code.claude.com/docs/en/sessions
- [mcp] Connect Claude Code to tools via MCP — https://code.claude.com/docs/en/mcp
- [model-config] Model configuration — https://code.claude.com/docs/en/model-config
- [context-window] Explore the context window — https://code.claude.com/docs/en/context-window
- [errors] Error reference, Usage limits 절 — https://code.claude.com/docs/en/errors
- [statusline] Status line, `rate_limits` 필드 — https://code.claude.com/docs/en/statusline
- [auth] Authentication (우선순위, `CLAUDE_CONFIG_DIR`별 키체인, `setup-token`) — https://code.claude.com/docs/en/authentication
- [security] Security — https://code.claude.com/docs/en/security
- [legal] Legal and compliance — https://code.claude.com/docs/en/legal-and-compliance
- [consumer-terms] Anthropic Consumer Terms of Service (시행 2025-10-08) 3조 7호 — https://www.anthropic.com/legal/consumer-terms
- [support-sdk] Use the Claude Agent SDK with your Claude plan — https://support.claude.com/en/articles/15036540-use-the-claude-agent-sdk-with-your-claude-plan
- [register] The Register, "Anthropic clarifies ban on third-party tool access to Claude" (2026-02-20, 보조 출처) — https://www.theregister.com/software/2026/02/20/anthropic-clarifies-ban-on-third-party-tool-access-to-claude/5014546
- [crates-search] docs.rs의 커뮤니티 crate 목록(보조): https://docs.rs/crate/claude-agent-sdk-rs/0.3.2, https://docs.rs/crate/apiari-claude-sdk/0.2.0, https://docs.rs/crate/ccwire/0.1.2
- 사용량 한도 구조의 제3자 정리(보조, 수치는 쓰지 않음): https://www.morphllm.com/claude-code-usage-limits
