# R-03 Codex CLI를 앱에서 구동하기

- 조사일: 2026-11-11 (KST). 아래 출처는 모두 이날 확인했다.
- 대상 버전: Codex CLI 0.167.1 (`codex --version`). 0.167.1 릴리스는 2026-11-10 21:32 UTC에 배포됐다 ([release]).
- 방법:
  - 1차 출처: learn.chatgpt.com의 Codex 공식 문서(developers.openai.com/codex/*는 이리로 308 리다이렉트된다), GitHub `openai/codex` 소스(태그 `rust-v0.167.1`), OpenAI 이용약관.
  - 로컬 검증: `codex --help`, `codex exec --help`, `codex features list`, `codex debug models`, `codex app-server generate-json-schema`/`generate-ts`(프로토콜 스키마 생성).
  - **가짜 모델 제공자 실험**: 127.0.0.1에 Responses API 흉내 서버를 띄웠다. `-c model_provider=fake`로 Codex를 거기에 붙여, CLI가 모델에 보내는 요청 본문(도구 목록 포함)을 그대로 잡았다. 비용은 0이고 계정을 쓰지 않는다. 이 서버로 도구 호출도 흉내 내어, 각 도구가 실제로 실행되는지 확인했다.
  - 실제 구독(ChatGPT Pro)으로 대화 실행은 **두 번만** 했다. `account/rateLimits/read`와 `model/list` 조회도 한 번 했다. 이 조회는 모델을 부르지 않는다.
- 실험 파일: `<세션 임시 폴더>/research/R-03/` (저장소 밖). 사용자 전역 설정(`~/.codex`)은 읽기만 했다.
- 실험 환경: `~/.codex`에 훅(9종 이벤트), 사용자 스킬(`~/.codex/skills`, `~/.agents/skills`), 에이전트 정의 14개, 메모리, 플러그인이 있는 계정.

## 1. 요약

1. **구동 방식은 `codex app-server`(stdio JSON-RPC)가 맞다.** `codex exec --json`에는 글자 단위 스트림이 없다. 오류도 사람이 읽는 문장 하나뿐이고, 한도 정보도 주지 않는다. 한도에 걸리면 "try again at 6:37 PM"이라는 문장만 온다(실측). app-server는 다음을 모두 준다: 글자 단위 스트림, 구조화된 오류 코드(`usageLimitExceeded`), 한도 조회와 갱신 알림, 스레드별 MCP 도구 목록, 메모리에만 있는 대화 스레드, 대화 기록 주입. 단, CLI 도움말은 app-server를 "[experimental]"로 표시한다.
2. **SEC-2는 충족할 수 있다. 다만 설정 한 줄로는 안 된다.**
   - 셸 도구는 `--disable shell_tool --disable unified_exec`로 모델 요청에서 완전히 사라진다(실측).
   - 파일 편집 도구 `apply_patch`는 기능 플래그로 끌 수 없다. 기본 상태에서는 read-only 샌드박스가 쓰기를 막는다(실측).
   - `apply_patch`를 도구 목록에서 없애는 확인된 방법은 하나뿐이다. **`model_catalog_json`으로 모델 카탈로그를 덮어써 `apply_patch_tool_type`을 null로 두는 것**이다(실측).
   - 현재 모델은 모두 "code mode"다. 즉 다른 도구를 JS 코드 안에서 부르는 `exec` 도구가 하나 붙는다. 이 JS는 V8 격리 공간에서 돌고 파일·네트워크 접근이 없다고 문서화되어 있다.
3. **`--ignore-user-config`만으로는 격리되지 않는다.** 그래도 `~/.codex/agents/*.toml`을 읽고(오류 이벤트 28건), 사용자 스킬 목록을 프롬프트에 넣는다. 셸 도구가 꺼져 있어도 **셸 환경 스냅샷이 환경 변수를 `$CODEX_HOME/shell_snapshots/`에 파일로 쓴다**(실측).
   - 대책: 앱 전용 `CODEX_HOME`(별도 로그인), `skills.include_instructions=false`, `--disable shell_snapshot`, `env -i`.
4. **매 실행마다 내장 도구 목록을 검증하는 공식 수단은 없다.** Claude의 init 이벤트 같은 것이 없다.
   - MCP 도구 목록은 `mcpServerStatus/list`로 확인할 수 있다(실측).
   - 지시문 출처는 `thread/start` 응답의 `instructionSources`로 확인할 수 있다(실측).
   - 내장 도구는 가짜 제공자 점검으로 보완해야 한다.
   - 웹 검색은 서버가 실행하는 도구다. 페이지 열기(`open_page`)가 포함되어 있어 "검색만" 따로 남길 수 없다. 검색어를 실행 전에 가로챌 수도 없다.
5. **약관: 조건부.** app-server 문서는 "로컬 또는 오픈소스 앱"이 app-server 인증을 계속 써도 된다고 적고 있다. 상업·호스팅 용도는 금지다. 반면 이용약관에는 "Automatically or programmatically extract data or Output" 금지 조항이 있다. 그래서 예약 작업을 계속 돌리는 방식은 R-02와 같은 위험을 안는다.

## 2. 질문별 답

### Q1. 비대화형 실행의 입출력 규약

Codex에는 비대화형 경로가 두 개 있다.

**(가) `codex exec --json`**

- stdout이 JSONL이 된다. 진행 상황 텍스트는 stderr로 간다 ([noninteractive]).
- 프롬프트는 인수로 주거나, `-` 또는 인수를 생략해 stdin으로 준다. 프롬프트 인수와 stdin을 함께 주면 stdin이 `<stdin>` 블록으로 덧붙는다 (`codex exec --help`).
- stdin을 `/dev/null`로 줘도 stderr에 "Reading additional input from stdin..."이 찍혔다(실측).
- 한 프로세스는 턴 하나만 처리한다. 다음 턴은 `codex exec resume <id>`로 새 프로세스를 띄운다.
- 이벤트 종류(소스 `codex-rs/exec/src/exec_events.rs`, 태그 rust-v0.167.1 [exec-events]):

| type | 필드 |
|---|---|
| `thread.started` | `thread_id` (이어가기에 쓴다) |
| `turn.started` | 없음 |
| `turn.completed` | `usage{input_tokens, cached_input_tokens, cache_write_input_tokens, output_tokens, reasoning_output_tokens}` |
| `turn.failed` | `error{message}` (**문자열 하나뿐**) |
| `item.started` / `item.updated` / `item.completed` | `item{id, type, …}` |
| `error` | `message` |

- item 종류: `agent_message{text}`, `reasoning`, `command_execution{command, aggregated_output, exit_code, status}`, `file_change`, `mcp_tool_call{server, tool, arguments, result{content, structured_content}, error, status}`, `collab_tool_call`, `web_search{query, action, results}`, `todo_list`, `error{message}`.
- **글자 단위 스트림이 없다.** `agent_message`는 완성된 뒤에만 나온다(실측, 소스).
- code mode의 `exec`(JS) 호출 자체는 이벤트로 나오지 않는다. 그 안에서 부른 MCP 호출만 `mcp_tool_call`로 나온다(실측).
- 실측 예 (격리 구성, 실제 구독, 모델 gpt-6-luna):

```jsonl
{"type":"thread.started","thread_id":"01a1c9ba-a925-…"}
{"type":"turn.started"}
{"type":"item.started","item":{"id":"item_28","type":"mcp_tool_call","server":"stockapp","tool":"get_portfolio","arguments":{},"result":null,"error":null,"status":"in_progress"}}
{"type":"item.completed","item":{"id":"item_28","type":"mcp_tool_call",…,"result":{"content":[{"type":"text","text":"{\"cash\":\"0\"}"}],"structured_content":null},"status":"completed"}}
{"type":"item.started","item":{"id":"item_29","type":"web_search","query":"","action":{"type":"other"}}}
{"type":"item.completed","item":{"id":"item_29","type":"web_search","query":"KOSPI index today","action":{"type":"search",…},"results":[…]}}
{"type":"item.completed","item":{"id":"item_30","type":"agent_message","text":"Succeeded: stockapp portfolio call and one web search. `id` could not be run; no shell command tool is available."}}
{"type":"turn.completed","usage":{"input_tokens":57214,"cached_input_tokens":40448,"cache_write_input_tokens":0,"output_tokens":181,"reasoning_output_tokens":0}}
```

- **종료 코드**: 성공이면 0. 한도 초과(가짜 서버가 429 `usage_limit_reached`를 돌려준 경우)는 `error`와 `turn.failed`를 내고 **exit 1**이었다(실측).
- 모델 서버에 연결이 안 되면 180초 넘게 아무것도 출력하지 않고 멈춰 있었다(실측). `request_max_retries=0`을 줘도 같았다. 앱이 자체 시간 제한을 걸어야 한다.

**(나) `codex app-server` (JSON-RPC 2.0, stdio는 줄 단위 JSON)** [app-server]

- 연결마다 `initialize` 요청을 한 번 보내고, `initialized` 알림을 보낸 뒤에야 다른 요청을 보낼 수 있다.
- 주요 요청:
  - `thread/start`(`model, cwd, sandbox, approvalPolicy, ephemeral, baseInstructions, developerInstructions, config{…}`)
  - `turn/start`(`threadId, input[], model, effort, sandboxPolicy, outputSchema …`)
  - `turn/interrupt`, `turn/steer`, `thread/resume`, `thread/fork`, `thread/inject_items`, `thread/compact/start`
  - `account/rateLimits/read`, `mcpServerStatus/list`, `model/list`
  - 전체 목록은 로컬에서 생성한 스키마에 있다: `generate-json-schema`
- 서버 알림:
  - 턴: `turn/started`, `turn/completed`
  - 항목: `item/started`, `item/completed`(이것이 확정 상태다)
  - 스트림: **`item/agentMessage/delta`**, `item/reasoning/summaryTextDelta`
  - 그 밖: `item/mcpToolCall/progress`, `thread/tokenUsage/updated`, **`account/rateLimits/updated`**, `mcpServer/startupStatus/updated`, `model/rerouted`, `error`, `warning`
- 오류 응답의 `codexErrorInfo`: `usageLimitExceeded`, `rateLimitExceeded`, `contextWindowExceeded`, `serverOverloaded`, `unauthorized`, `sandboxError` … (생성한 TS 타입 `CodexErrorInfo.ts`). 서버가 과부하면 `-32001` "Server overloaded; retry later"가 오고, 지수 백오프로 재시도하라고 문서가 권한다 [app-server].
- 실측 순서 (가짜 제공자, MCP 하나):

```
thread/started → mcpServer/startupStatus/updated(starting → ready) → thread/status/changed(active)
→ turn/started → item/started|completed(userMessage) → item/started|completed(mcpToolCall stockapp.get_portfolio)
→ thread/tokenUsage/updated → account/rateLimits/updated → item/started|completed(agentMessage)
→ thread/status/changed(idle) → turn/completed
```

- 전송 방식: `stdio://`(기본), `unix://`, `ws://IP:PORT`. 문서는 WebSocket이 "experimental and unsupported"라고 적는다. 실험 기능은 `capabilities.experimentalApi`를 켜야 쓸 수 있다 [app-server].

### Q2. CLI 직접 실행과 SDK의 차이

| 항목 | `codex exec --json` | `codex app-server` | Codex SDK |
|---|---|---|---|
| 언어 | 아무 언어 | 아무 언어 (JSON-RPC) | TypeScript(Node 18+), Python 3.10+ [sdk] |
| 실체 | CLI | CLI 하위 명령 | Python SDK는 "controls the local Codex app-server over JSON-RPC" [sdk] |
| 스트림 | 항목 완성 단위 | 글자 단위 delta | 문서에 스트리밍 API가 없다. 스트림은 app-server를 쓰라고 안내한다 [sdk] |
| 한도 조회 | 없음 | `account/rateLimits/read` + 알림 | 미확인 |
| 공식 안내 | CI·스크립트용 [noninteractive] | "Use it when you want a deep integration inside your own product" [app-server] | "If you are automating jobs or running Codex in CI, use the Codex SDK" [app-server] |
| 안정성 | 안정 | CLI 도움말에 `[experimental]`. 실험 필드는 `experimentalApi`로 막혀 있다 | Python SDK는 stable [sdk] |
| 구독 인증 | 저장된 CLI 로그인을 다시 쓴다 [noninteractive] | ChatGPT 브라우저·장치 코드 로그인, 외부 토큰(실험), API 키 [app-server-auth] | 로그인한 계정 기준 [sdk] |

- Rust용 공식 SDK는 없다. app-server 프로토콜의 JSON Schema와 TS 바인딩은 CLI가 직접 만들어 준다(`codex app-server generate-json-schema --out …`, `generate-ts`). 이 스키마로 Rust serde 타입을 만들거나 계약 테스트 기준으로 쓸 수 있다.
- **결론**: Rust 앱은 app-server를 자식 프로세스(stdio)로 띄운다. exec는 대안 경로로 남긴다.

### Q3. 도구 제한과 격리

가짜 제공자로 잡은 모델 요청의 도구 목록(gpt-6-sol, code mode) 비교:

| 구성 | 최상위 도구 | `exec`(JS) 안의 도구 |
|---|---|---|
| `--ignore-user-config --ignore-rules -s read-only`만 | exec, wait, request_user_input(_async), clock.sleep, collaboration.{spawn_agent, send_message, wait_agent, …} | apply_patch, create/get/update_goal, **exec_command, write_stdin**, list/read MCP 리소스, request_plugin_install, view_image, clock |
| + 격리 플래그 (아래) | exec, wait, request_user_input(_async) | **apply_patch**, list/read MCP 리소스, `mcp__stockapp__get_portfolio`, clock |
| + `model_catalog_json`(apply_patch_tool_type=null) | 같음 | list/read MCP 리소스, `mcp__stockapp__get_portfolio`, clock (**셸·파일 도구 없음**) |
| + 카탈로그 `tool_mode=null` | functions.{MCP 리소스 도구, request_user_input(_async)}, clock.curr_time, tool_search (**JS exec도 없음**) | — |

격리 플래그 (실측에 쓴 것):

```
-c web_search="live" -c skills.bundled.enabled=false -c skills.include_instructions=false
-c project_doc_max_bytes=0 -c plugins._default.enabled=false -c agents.enabled=false
-c history.persistence="none"
--disable shell_tool --disable unified_exec --disable multi_agent --disable apps --disable plugins
--disable memories --disable hooks --disable view_image --disable image_generation --disable browser_use
--disable computer_use --disable goals --disable sleep_tool --disable tool_suggest --disable skill_search
--disable in_app_browser --disable remote_plugin --disable shell_snapshot
```

가짜 모델이 code mode로 각 도구를 직접 불러 본 결과(실측):

```
ALL_TOOLS=["apply_patch","clock__curr_time","list_mcp_resource_templates","list_mcp_resources","mcp__stockapp__get_portfolio","read_mcp_resource"]
ORDER_ERR=TypeError: tools.mcp__stockapp__place_order is not a function          ← enabled_tools 허용 목록이 동작함
PATCH_ERR=patch rejected: writing is blocked by read-only sandbox; rejected by user approval settings
SHELL_ERR=TypeError: tools.exec_command is not a function
```

카탈로그를 덮어쓴 뒤에는 `PATCH_ERR=TypeError: tools.apply_patch is not a function`이었다.

실제 구독 실행(두 번째)에서 모델이 `ALL_TOOLS`를 출력한 결과는 다음과 같았다:
`["clock__curr_time","list_mcp_resource_templates","list_mcp_resources","mcp__stockapp__get_portfolio","read_mcp_resource","web__run"]`.
모델은 최상위 도구를 `functions.exec, functions.wait, functions.request_user_input, functions.request_user_input_async`라고 보고했다. 이 최상위 목록은 모델의 자기 보고이므로 참고만 한다.

**섞여 들어오는 것과 막는 방법**:

| 출처 | `--ignore-user-config`만 | 막는 방법 (실측) |
|---|---|---|
| `~/.codex/config.toml` | 차단 [noninteractive] | — |
| execpolicy `.rules` | `--ignore-rules`로 차단 [noninteractive] | — |
| `~/.codex/agents/*.toml` | **읽힌다** (오류 이벤트 28건) | 앱 전용 `CODEX_HOME` → 사라짐(실측) |
| 사용자 스킬 `~/.codex/skills`, `~/.agents/skills` | **프롬프트에 목록이 들어간다** | `skills.include_instructions=false` → 스킬 블록 삭제(실측). `~/.agents/skills`는 `CODEX_HOME`을 바꿔도 읽힌다(HOME 기준, 실측) |
| 멀티 에이전트 지시문·도구 | 들어간다 | `agents.enabled=false` (`--disable multi_agent`만으로는 collaboration 도구가 남았다, 실측) |
| AGENTS.md | 작업 폴더 기준으로 읽는다 | 빈 작업 폴더 + `project_doc_max_bytes=0`. `thread/start` 응답의 `instructionSources: []`로 확인(실측) |
| 훅 (`~/.codex/hooks.json` 등) | 미확인 (사용자 훅이 실제로 실행되는 실험은 하지 않았다) | `--disable hooks`(문서: `features.hooks=false`) + 앱 전용 `CODEX_HOME`. 비관리 훅은 신뢰 등록이 있어야 돈다 [hooks] |
| 셸 환경 스냅샷 | **환경 변수 전체를 `$CODEX_HOME/shell_snapshots/`에 기록** (셸 도구를 꺼도 생김) | `--disable shell_snapshot` → 생기지 않음(실측) |
| 관리 설정(`/etc/codex/requirements.toml`, MDM) | 이 PC에는 없다 (`app-server --print-startup-config` → `requirements: null`) | 앱이 시작할 때 같은 명령으로 확인 |

- 환경 변수: 셸 스냅샷 실측 파일에 `CLAUDE_CODE_MESSAGING_TOKEN` 같은 부모 프로세스 비밀 변수가 들어 있었다. 그래서 **`env -i`로 비우고 허용 목록만 넘긴다**. `CODEX_API_KEY`나 `OPENAI_API_KEY`가 있으면 구독 대신 API 키가 쓰일 수 있다 [noninteractive]. 이 점도 비우는 이유다.
- **앱 전용 `CODEX_HOME`**: 인증 정보가 `CODEX_HOME` 안(`auth.json`)에 있다("`--ignore-user-config` … auth still uses `CODEX_HOME`", exec 도움말). 그래서 앱 전용 디렉터리를 쓰려면 사용자가 그 디렉터리로 `codex login`을 한 번 해야 한다.
  - **사용자의 `auth.json`을 복사하지 않는다.** 문서는 같은 `auth.json`을 동시 작업이나 여러 기기에서 함께 쓰지 말라고 한다. 토큰이 회전하면 다른 쪽이 끊기기 때문이다 [ci-cd-auth].
  - 이 로그인 절차 자체는 실험하지 않았다.

### Q4. MCP 연결

- **실행할 때 지정하는 방법**:
  - exec: `-c 'mcp_servers.<이름>={command=…, args=[…], …}'`
  - app-server: `thread/start`의 `config.mcp_servers`
  - 실측: 두 방식 모두 연결됐다. 앱 전용 `CODEX_HOME`을 쓰면 다른 서버는 설정에 존재하지 않는다.
- **키** [mcp]:
  - stdio: `command, args, env, env_vars, cwd`
  - Streamable HTTP: `url, bearer_token_env_var, http_headers, env_http_headers, http_headers_helper`
  - 공통: `required`, `enabled_tools`(허용 목록), `disabled_tools`, `startup_timeout_sec`(기본 10), `tool_timeout_sec`(기본 60), `default_tools_approval_mode`(`auto|prompt|writes|approve`), `tools.<도구>.approval_mode`, `tools.<도구>.output_token_limit`
- **전송**: stdio와 Streamable HTTP를 지원한다. 문서에 Unix 소켓 전송은 없다. R-04는 stdio 브리지와 HTTP 양쪽으로 Codex 호출에 성공했다. stdio 브리지에 토큰을 넘기려면 `env_vars` 허용 목록이 필요했다 (R-04 실험 4).
- **인증 헤더**: HTTP에서는 `bearer_token_env_var`로 환경 변수에서 Bearer 토큰을 읽는다 [mcp]. R-04에서 실측했다.
- **실측 주의 두 가지**:
  1. **MCP 도구 호출은 기본값에서 승인이 필요하다.** `approval_policy=never`(exec 기본값)와 함께면 `"MCP tool call requires approval, but approval policy is never"`로 실패했다. `default_tools_approval_mode="approve"`(또는 도구별 `approval_mode`)를 주자 성공했다. 앱은 읽기 도구만 `approve`로 두고, 그 밖의 도구는 `enabled_tools`에서 뺀다.
  2. `required=true`면 서버 초기화가 실패할 때 exec가 오류로 끝난다 [noninteractive]. Claude Code처럼 MCP 없이 조용히 계속 가는 일이 없다.
- MCP 도구는 처음 요청에 정의가 들어가지 않고 지연 노출된다(`tool_search` 또는 `ALL_TOOLS`, 실측). 모델은 필요할 때 찾아서 부른다. 실제 구독 실행에서 모델은 도구를 문제없이 찾았다.

### Q5. 세션

- **이어가기**:
  - exec: `codex exec resume <SESSION_ID|이름> "<프롬프트>"` (또는 `--last`) [noninteractive]. 실측: 2턴째 요청에 1턴의 사용자·조수 메시지가 들어 있었다.
  - app-server: `thread/resume`.
  - **이어 갈 때 `-m`을 다시 주지 않으면 기본 모델로 바뀐다.** 실측 경고: "This session was recorded with model `gpt-6-sol` but is resuming with `gpt-6.1-sol`". 모델, MCP, 격리 플래그는 매번 다시 넘긴다.
- **저장 위치(실측)**:
  - `$CODEX_HOME/sessions/YYYY/MM/DD/rollout-<시각>-<thread_id>.jsonl` (평문)
  - `$CODEX_HOME/state_5.sqlite`의 `threads` 테이블. `title`과 `first_user_message`에 첫 메시지 원문이 들어간다.
  - `$CODEX_HOME/thread_history_1.sqlite`의 `thread_items`와 `thread_turns`
  - TUI 입력 기록 `history.jsonl`은 `history.persistence`로 제어한다 [config-ref].
- **맥락 압축**:
  - 자동 압축 기준은 `model_auto_compact_token_limit`이다. 값을 주지 않으면 모델 기본값을 쓴다 [config-ref].
  - 수동으로는 app-server `thread/compact/start`를 쓴다.
  - 압축은 `contextCompaction` 항목으로 알린다 [app-server].
  - 카탈로그상 맥락 창: gpt-6.1-sol·gpt-6.1-terra 500K, 나머지 272K (`codex debug models`).

### Q6. 모델 선택

- 실행마다 다음을 준다:
  - exec: `-m <모델>`, `-c model_reasoning_effort="<단계>"`
  - app-server: `turn/start`의 `model`, `effort` (그 턴과 이후 턴에 적용) [app-server]
- 이 Pro 계정의 `model/list` 실측: gpt-6.1-sol(기본), gpt-6-astra, gpt-6.1-terra, gpt-6-sol, gpt-6-luna, gpt-5.6-sol, gpt-5.6-terra, gpt-5.6-luna.
  - 추론 단계: luna 계열은 `low~max`, 나머지는 `low~ultra`.
- 플랜별 목록 [pricing]:
  - Plus 카드에는 GPT-6.1 Sol과 GPT-6 Luna가 나온다. 사용량 표에는 Astra, Terra, Sol도 있다.
  - Free와 Go는 GPT-6 Luna Standard (데스크톱 앱, 단계적 출시).
  - Pro의 모델 목록은 따로 적혀 있지 않다. **플랜별 정확한 목록은 미확인이다.**
  - "GPT-5.5 retires from Codex with ChatGPT sign-in on October 14, 2026" [pricing]. 모델은 수시로 교체된다.
- 자동 전환: app-server에 `model/rerouted` 알림이 있다. 바이너리에 "Automatically switched to … due to usage limits"라는 문자열이 있다. exec나 app-server에서 이 전환이 언제 일어나는지는 미확인이다. 앱은 `turn/completed`와 `model/rerouted`에서 실제로 쓴 모델을 기록한다.

### Q7. 구독 사용량 한도

- **구조** [pricing]:
  - Plus와 Standard Business는 5시간 창이 있고, 주간 한도가 더해질 수 있다.
  - "Pro plans currently have no five-hour limit."
  - 로컬 실행과 클라우드가 같은 한도를 나눠 쓴다.
  - 한도를 넘은 뒤에는 크레딧을 사거나 API 키로 계속 쓸 수 있다.
- **남은 한도 조회(실측)**: `account/rateLimits/read` 결과 (`accountId`는 지움):

```json
{"ordinaryUsageAllowed":true,
 "rateLimits":{"limitId":"codex","primary":{"usedPercent":11,"windowDurationMins":10080,"resetsAt":1794450093},
   "secondary":null,"credits":{"hasCredits":false,"unlimited":false,"balance":"0"},
   "spendControlReached":false,"planType":"pro","rateLimitReachedType":null},
 "rateLimitsByLimitId":{"codex":{…같음…}},
 "rateLimitResetCredits":{"availableCount":0,"credits":[]}}
```

  - 실측에서는 주간 창(10080분) 하나만 있었다. 이것은 Pro에 5시간 창이 없다는 문서와 맞는다.
  - `resetsAt`은 유닉스 초다 [app-server-auth].
  - 문서는 `ordinaryUsageAllowed`가 null이면 "clients must not infer recovery from percentages or reset times"라고 적는다(생성한 스키마).
  - 실행 중에는 `account/rateLimits/updated` 알림이 온다.
- **한도에 걸렸을 때의 신호**:
  - app-server: `codexErrorInfo: "usageLimitExceeded"` [app-server]. 문서 기준이고 실제 한도로 관찰하지는 않았다.
  - exec(가짜 429로 실측): `{"type":"error","message":"You’ve hit your usage limit. Visit https://chatgpt.com/settings/usage to purchase more credits or try again at 6:37 PM."}`, 이어서 `turn.failed`, exit 1. **재개 시각이 현지 시각 문장으로만 온다**. exec만 쓰면 재개 시각을 얻으려면 이 문장을 파싱하거나 app-server로 따로 조회해야 한다.
- 문서상 "If you reach a limit mid-task, the agent will be able to continue working on that turn" [pricing].

### Q8. 약관 (판정: 조건부. 최종 판정은 R-13에서)

- **app-server 문서의 인증 절** [app-server-auth]:

  > "If you've built a local or open-source application using Codex app-server authentication, you can continue using it, though we recommend migrating to Sign in with ChatGPT so users have greater control over and visibility into their usage. … App-server authentication has never been permitted for commercial or hosted services."

  이 앱은 배포하지 않는 본인용 로컬 앱이므로 "local … application"에 해당한다. 날짜는 적혀 있지 않다.
- **이용약관**(Rest of World, 시행 2026-01-01) [tou]. 한국 거주자에게 이 판본이 적용되는지는 확인하지 못했다:
  - "You may not share your account credentials or make your account available to anyone else"
  - 금지 행위: "Automatically or programmatically extract data or Output (defined below)." / "Interfere with or disrupt our Services, including circumvent any rate limits or restrictions"
  - "You should not rely on Output from our Services as a sole source of truth or factual information, or as a substitute for professional advice."
- **Codex 문서의 자동화 안내** [ci-cd-auth]:
  - "The right way to authenticate automation is with an API key."
  - ChatGPT 계정 인증으로 자동화하는 것은 "advanced workflow for enterprise and other trusted private automation"이고, 공개 저장소에는 쓰지 말라고 한다.
  - 즉 금지는 아니지만 권장 경로도 아니다.
- **해석**:
  - 다음 조건을 지키면 문서상 허용 범위에 가깝다: (1) 수정하지 않은 `codex`를 쓴다, (2) 사용자가 직접 `codex login`을 한다, (3) 앱이 토큰을 읽거나 저장하지 않는다, (4) 배포하지 않는다.
  - "programmatically extract … Output" 조항이 예약 작업으로 계속 호출하는 사용에 걸리는지는 해석이 갈린다. Codex가 `exec`와 app-server를 공식 기능으로 문서화한 점은 허용 쪽 근거다.
  - "circumvent any rate limits"는 한도를 넘으려고 우회하는 것을 말한다. 한도 안에서 쓰는 것과는 다르다.
  - 예약 작업을 계속 돌리는 방식을 직접 제한하는 조항은 찾지 못했다.
- "Sign in with ChatGPT"(SIWC)는 제3자 앱이 사용자 플랜 한도를 쓰게 하는 공식 경로다 [siwc]. 파트너 대기 명단 방식이라 개인 앱에는 현실적이지 않다. 개인용 로컬 앱에 SIWC가 필요한지는 미확인이다.

### Q9. 동시 실행

- 구독에서 동시에 띄울 수 있는 실행 수의 공식 한도는 **미확인**이다.
- 하나의 app-server 프로세스가 여러 스레드를 관리한다(`thread/loaded/list` 등). 과부하면 `-32001`로 밀어낸다 [app-server]. 그래서 **프로세스 하나, 스레드 여러 개**가 기본 구조로 맞다.
- 같은 `auth.json`을 동시 작업이나 여러 기기에서 함께 쓰지 말라는 문서가 있다 [ci-cd-auth]. app-server 하나가 인증을 소유하면 이 문제가 생기지 않는다. 같은 `CODEX_HOME`에 exec 프로세스를 여러 개 띄울 때 SQLite 상태 파일이 경합하는지는 미확인이다.
- 동시 실행은 같은 한도(Pro는 주간)를 함께 깎는다.
- 권장: 채팅 스레드 1개와 예약 스레드 1개. 채팅이 오면 예약 턴에 `turn/interrupt`를 보낸다.

### Q10. 프롬프트 주입 방어에 쓸 수 있는 CLI 기능

- 도구 최소화: 셸·편집·이미지·에이전트·플러그인 도구를 끄고(Q3), MCP는 `enabled_tools` 허용 목록으로 제한한다(실측: 목록에 없는 도구는 호출 불가).
- 샌드박스: `-s read-only`(기본값). JS `exec`는 "no Node, no file system, no network access"라고 적혀 있다(도구 설명, 실측 요청 본문).
- MCP 도구별 승인 모드(`approve/prompt/writes`) [mcp]. 앱은 쓰기 성격의 도구(`propose_order`, `update_thesis`)를 MCP 서버 쪽에서 검증한다. Codex 승인에 기대지 않는다.
- 웹 검색 모드 [config-ref]:
  - `cached`(기본값): "uses an OpenAI-maintained index without external web access". 외부 페이지를 실시간으로 읽지 않는다.
  - `live`: 실시간 조회.
  - `disabled`: 도구를 없앤다.
  - `tools.web_search.allowed_domains`로 도메인을 제한할 수 있다.
  - 주입 노출을 줄이려면 예약 작업은 `cached`로 둔다.
- **검색어를 실행 전에 가로챌 수단은 없다(관측)**:
  - 웹 검색은 OpenAI 서버가 실행하는 호스팅 도구다.
  - `item.started` 시점에 `query`가 빈 문자열이었고, 완료 후에야 검색어가 채워졌다(실측).
  - R-12가 Claude에서 쓴 "검색어 게이트"는 Codex에서 쓸 수 없다. 훅으로 가로챌 수 있는지는 미확인이다.
- 출력 형식 강제: exec `--output-schema <파일>`, app-server `turn/start.outputSchema` [noninteractive].
- 지시문 교체: `thread/start`의 `baseInstructions`(기본 코딩 지시문 교체), `developerInstructions` [app-server]. `model_instructions_file`도 있다 [config-ref]. 동작은 실측하지 않았다.
- `untrusted_as_tool_output` 기능 플래그가 있지만 "under development"이고 문서가 없다 (`codex features list`).

### Q11. Claude Code와 Codex를 같은 인터페이스 뒤에 둘 때 어댑터가 흡수할 차이

Claude 쪽 값은 R-02 실측 결과를 따른다.

| 영역 | Claude Code 2.1.322 (R-02) | Codex 0.167.1 | 어댑터가 할 일 |
|---|---|---|---|
| 프로세스 모델 | `claude -p` 1프로세스, stdin을 열어 두면 여러 턴 | app-server 1프로세스에 스레드 여러 개 (JSON-RPC 요청·응답 + 알림) | `Runner` 하나에 `start_thread / send_turn / interrupt` 의미로 통일 |
| 전송 규약 | NDJSON 메시지 (`type` 필드) | JSON-RPC 2.0 (`id`, `method`), 서버가 먼저 요청하기도 함(`item/tool/requestUserInput`, 승인 요청) | Codex 서버 요청에는 자동 거부 응답 |
| 글자 스트림 | `stream_event`(`content_block_delta`) | `item/agentMessage/delta` | 공통 `TextDelta` 이벤트 |
| 도구 호출 이벤트 | `assistant.tool_use` → `user.tool_result` | `item/started` / `item/completed`(`mcpToolCall`, `webSearch`) | 공통 `ToolCall{server, tool, args, result, status}` |
| 사용량 | `result.usage`, `modelUsage`, `total_cost_usd` | `thread/tokenUsage/updated`(total/last), 비용 없음 | 토큰만 공통으로. 비용은 선택 필드 |
| 한도 정보 | `rate_limit_event`(문서에 없는 필드) | `account/rateLimits/read` + `updated` (문서화됨, `usedPercent`, `windowDurationMins`, `resetsAt`) | 공통 `Quota{windows[], resets_at}` |
| 한도 초과 | result `api_error:"usage_limit_reached"`, exit 1 | `codexErrorInfo:"usageLimitExceeded"` (exec는 문장 + exit 1) | 공통 `RateLimited{resets_at}` → 큐 대기(REL-4) |
| 세션 이어가기 | `--session-id`/`--resume`, 모델·MCP 다시 지정 | `thread/resume`(디스크 기록 필요) 또는 메모리 스레드 + `thread/inject_items` | `resume(session_ref)`가 "디스크 ID"와 "앱 DB에서 다시 주입"을 모두 표현 |
| 기록을 남기지 않기 | `--no-session-persistence` (빈 memory 폴더는 남음) | `ephemeral`(rollout과 threads 행 없음, 실측). SQLite 상태 파일은 생긴다 | 정책 필드 하나로 |
| 격리 검증 | `system/init`의 `tools`, `mcp_servers`, `plugins`, `skills` (`/usage` 사전 점검) | 내장 도구 목록을 주는 API가 없다. `mcpServerStatus/list`(MCP 도구), `instructionSources`, `sandbox`만 있다 | 제공자별 `verify_isolation()`. Codex는 가짜 제공자 점검으로 보완 (3.2) |
| 셸 도구 제거 | `--restricted`가 실행 도구와 WebFetch를 없앰 | 기능 플래그 + 카탈로그 덮어쓰기 | 제공자별 구성 생성기 |
| 웹 검색 | WebSearch(검색만). WebFetch는 따로 끈다 | 호스팅 web_search에 `open_page`, `find_in_page`가 포함됨 | "웹 읽기 가능" 기능 플래그로 표시 |
| MCP 승인 | `--allowedTools` + `dontAsk` | `default_tools_approval_mode="approve"` + `enabled_tools` | 허용 목록 하나에서 양쪽 설정 생성 |
| MCP 실패 | 계속 진행(init 검사 필수) | `required=true`면 즉시 실패 | 공통 `McpUnavailable` |
| MCP 프로토콜 세대 | 2026-07-28 | 2025-06-18 `initialize` (R-04) | 서버(rmcp)가 둘 다 받음 |
| 구조화 출력 | `--json-schema` → `structured_output` | `--output-schema` / `outputSchema` → 마지막 메시지 텍스트 | 앱이 JSON을 다시 검증 |
| 모델·추론 | `--model`, `--effort low~max` | `model`, `effort low~ultra`(모델별 다름) | 단계 이름 매핑 표. 지원하지 않는 단계는 거부 |
| 인증 설정 위치 | `CLAUDE_CONFIG_DIR`, 키체인 항목이 디렉터리마다 따로 | `CODEX_HOME/auth.json`(기본은 파일. `cli_auth_credentials_store`로 바꿀 수 있음) | 제공자별 "로그인 필요" 상태 |
| 중단 | SIGINT로 턴만 정리 | `turn/interrupt` | 공통 `interrupt()` |

### Q12. 한쪽에만 있는 기능 중 설계에 영향을 주는 것

- **Codex에만 있는 것**:
  - **문서화된 한도 조회 API**(`account/rateLimits/read`, 모델 호출 없음). AI-2 예산과 큐 재개 시각을 정확히 계산할 수 있다.
  - **메모리 스레드 + `thread/inject_items`**. 디스크에 대화를 남기지 않고, 앱 SQLite의 기록을 주입해 대화를 이어 갈 수 있다(실측: 주입한 user/assistant 메시지가 다음 요청에 들어갔고, `sessions/`는 생기지 않았다). Claude에는 이에 해당하는 공식 수단이 없다.
  - **`required=true`**: MCP가 실패하면 즉시 중단된다.
  - **스레드별 MCP 도구 목록 API**.
  - **프로토콜 스키마 생성**(`generate-json-schema`). Rust 타입과 계약 테스트의 기준으로 쓸 수 있다.
- **Claude에만 있는 것**:
  - 실행마다 내장 도구 목록을 그대로 보고하는 init 이벤트.
  - 웹 검색과 웹 읽기의 분리(WebSearch / WebFetch).
  - 검색어 게이트(R-12).
  - 비용 추정(`total_cost_usd`).
- **설계 영향**:
  1. "보유 내역을 아는 실행에서 웹 검색을 쓰면 검색어로 보유 내역이 샐 수 있다"(R-12 W1)는 위험을 Codex에서는 게이트로 막을 수 없다. Codex 프로필에서는 보유 내역을 다루는 단계의 웹 검색을 `disabled`로 두고, 웹 조사는 별도 실행으로 분리한다.
  2. SEC-2의 "웹 읽기 도구 없이"는 Codex 웹 검색에 페이지 열기가 포함되어 있어 엄밀히는 충족되지 않는다. `cached` 모드(외부 접근 없음)를 "웹 읽기 없음"으로 인정할지 정해야 한다.

### Q13. 셸 실행 도구를 샌드박스가 아니라 완전히 없앨 수 있는가

**있다(실측).**

- `features.shell_tool=false`와 `features.unified_exec=false`(`--disable shell_tool --disable unified_exec`)를 주면 `exec_command`와 `write_stdin`이 모델 요청에서 사라진다.
- 가짜 모델이 불러도 `TypeError: tools.exec_command is not a function`이었다.
- 문서상 `unified_exec`에는 "User opt-outs are ignored"라는 설명이 붙어 있다 [config-ref]. 그래도 실측에서는 도구가 사라졌다. 버전마다 다시 확인해야 한다.

파일 쓰기와 코드 실행에 대해서는 다음과 같다.

| 도구 | 끄는 방법 | 상태 |
|---|---|---|
| `exec_command`, `write_stdin` (셸) | 기능 플래그 두 개 | **완전 제거** (실측) |
| `apply_patch` (파일 편집) | 기능 플래그 없음. `model_catalog_json`에서 `apply_patch_tool_type: null` | 기본값: 남아 있지만 read-only가 거부. 카탈로그를 덮어쓰면 **완전 제거** (가짜 제공자로 실측) |
| `view_image` (로컬 이미지 읽기) | `--disable view_image` | 제거 (실측) |
| JS `exec` (code mode) | 카탈로그 `tool_mode: null` | 기본값: 남음(파일·네트워크 없음). 카탈로그를 덮어쓰면 제거 (가짜 제공자로만 실측. 실제 모델의 품질 영향은 미확인) |

- 판정: **SEC-2의 "셸 없음"은 충족한다.** "파일 도구 없음"은 카탈로그 덮어쓰기까지 하면 충족한다.
- `model_catalog_json`은 문서화된 설정 키다("Optional path to a JSON model catalog loaded on startup" [config-ref]). 하지만 카탈로그 항목의 `apply_patch_tool_type`과 `tool_mode` 필드의 의미는 문서화되어 있지 않다. 그래서 버전을 올릴 때마다 계약 테스트로 다시 확인해야 한다.
- 실제 구독 실행에서 덮어쓴 카탈로그가 쓰였는지 확인했다. 모델이 출력한 `ALL_TOOLS`에 `apply_patch`가 없었다.

## 3. 선택지 비교표와 추천

### 3.1 구동 방식

| 선택지 | 스트림 | 한도 신호 | 격리 검증 | 기록 안 남기고 이어가기 | 안정성 | 판정 |
|---|---|---|---|---|---|---|
| A. `codex app-server` (stdio) | 글자 단위 | 구조화(`usageLimitExceeded`, `rateLimits`) | MCP 목록 + instructionSources | 됨 (`ephemeral` + `inject_items`, 실측) | `[experimental]` 표시, 스키마 생성 가능 | **추천** |
| B. `codex exec --json` | 항목 단위 | 문장 하나 + exit 1 | 없음 | 안 됨 (ephemeral은 resume 불가) | 안정, 문서화 | 대안 (A가 깨질 때) |
| C. Codex SDK (Python/TS 사이드카) | 문서상 없음 | 미확인 | A와 같음 | A와 같음 | Python stable | 비추천 (사이드카 추가) |
| D. API 키 (`CODEX_API_KEY`) | A/B와 같음 | 종량제 | 같음 | 같음 | 공식 권장 자동화 경로 [ci-cd-auth] | 약관 위험 대안 |

### 3.2 격리 구성 (추천)

```text
환경: env -i 후 다음만 넘김
  HOME, PATH(codex·node 경로), USER, LOGNAME, TMPDIR,
  CODEX_HOME=~/Library/Application Support/<앱>/codex   (사용자가 한 번 `CODEX_HOME=… codex login`)
  STOCK_MCP_TOKEN=<실행마다 새 토큰>
작업 폴더: 앱 전용 빈 폴더 (git 아님)
명령:
codex app-server --listen stdio:// \
  -c model_catalog_json="<앱이 관리하는 카탈로그: apply_patch_tool_type=null>" \
  -c web_search="cached" -c skills.bundled.enabled=false -c skills.include_instructions=false \
  -c project_doc_max_bytes=0 -c plugins._default.enabled=false -c agents.enabled=false \
  -c history.persistence="none" -c analytics.enabled=false \
  --disable shell_tool --disable unified_exec --disable shell_snapshot --disable view_image \
  --disable multi_agent --disable apps --disable plugins --disable remote_plugin --disable memories \
  --disable hooks --disable image_generation --disable browser_use --disable computer_use \
  --disable in_app_browser --disable goals --disable sleep_tool --disable tool_suggest --disable skill_search
thread/start: { model, cwd:<빈 폴더>, sandbox:"read-only", approvalPolicy:"never", ephemeral:true,
                baseInstructions:<앱 프롬프트>, config:{ mcp_servers:{ app:{ command:<브리지>,
                env_vars:["STOCK_MCP_TOKEN"], required:true, enabled_tools:[…],
                default_tools_approval_mode:"approve" } } } }
```

시작 절차:

1. `app-server --print-startup-config`로 관리 요구사항이 null인지 확인한다.
2. 앱을 켤 때와 CLI 버전이 바뀔 때마다 **같은 플래그로 가짜 제공자 점검**을 돌린다. 127.0.0.1 흉내 서버가 요청 본문의 도구 목록을 기대 목록과 대조한다. 비용은 0이다.
3. `thread/start` 응답에서 `instructionSources == []`, `sandbox.type == "readOnly"`, `multiAgentMode`를 확인한다. `mcpServerStatus/list(threadId)`의 도구 목록이 기대와 같은지 확인한다. 다르면 중단한다.
4. 대화를 이어 갈 때는 앱 SQLite의 기록을 `thread/inject_items`로 넣는다.

`analytics.enabled=false`는 키가 존재한다는 것만 확인했다 [config-ref]. 동작은 실측하지 않았다.

**추천 이유**:
- 실측으로 확인한 구성이다: 이 PC처럼 사용자 설정이 많은 환경에서 셸·편집 도구가 0개가 되고, MCP 허용 목록이 동작하고, 구독 인증으로 돌아갔다.
- 한도를 구조화된 값으로 얻을 수 있다.
- 대화 기록을 Codex 디렉터리에 남기지 않고 이어 갈 수 있다.

**추천이 틀렸다고 판명될 조건**:
- app-server 프로토콜이 하위 호환을 깨거나, 앱 인증("local … application")을 막는 경우. 이때는 B(exec)나 D(API 키)로 옮긴다.
- 서버 쪽 모델 카탈로그가 바뀌어 `model_catalog_json` 덮어쓰기가 무시되거나 모델이 오동작하는 경우. 이때는 `apply_patch`가 남는 구성(read-only가 거부)을 받아들일지, Codex를 끌지(AI-09) 정한다.
- 내장 도구가 가짜 제공자 점검과 실제 제공자에서 다르게 노출되는 경우. 실측에서 이미 web_search가 실제 제공자에서만 나타났다. 이것이 다른 도구로 번지면 사전 점검을 믿을 수 없다.
- OpenAI가 개인용 자동 실행을 약관이나 기술로 막는 경우.

## 4. 위험과 미확인 항목

**위험**

- **비공식에 가까운 의존**: 카탈로그 필드(`apply_patch_tool_type`, `tool_mode`), code mode의 `ALL_TOOLS`, app-server의 `[experimental]` 표시. 버전 고정과 계약 테스트가 필요하다. 0.167.1은 조사 전날 배포됐다. 릴리스가 매우 잦다 [release].
- **격리 누수(실측)**: `--ignore-user-config`로도 `~/.codex/agents`, 사용자 스킬 목록, 셸 스냅샷(환경 변수의 평문 파일)이 남는다. 앱 전용 `CODEX_HOME`과 `env -i`를 빼먹으면 SEC-2와 R-16을 위반한다.
- **웹 검색에 페이지 읽기가 포함된다.** 실행 전 검색어 게이트가 불가능하므로 보유 내역이 검색어로 샐 수 있다(R-12 W1).
- **exec는 연결 실패 시 무기한 대기(실측)**. 앱이 시간 제한과 강제 종료를 해야 한다. `unbounded_connection_retries` 기능(stable, on)이 원인일 수 있다. 끄는 실험은 하지 않았다.
- **평문 기록**: 영속 스레드는 `sessions/*.jsonl`과 `state_5.sqlite.threads.first_user_message`에 원문을 남긴다. 메모리 스레드여도 `logs_2.sqlite`, `state_5.sqlite` 같은 파일은 생긴다. 실측에서 메모리 스레드의 대화 원문은 이 파일들에서 찾지 못했다.
- 약관: "programmatically extract … Output" 조항과 예약 작업이 충돌할 수 있다는 해석의 여지(Q8).
- 모델이 자주 교체된다(GPT-5.5 퇴역 등). 작업별 모델 설정에 없는 모델이 남을 수 있다.

**미확인**

- 플랜별 사용 가능 모델의 정확한 목록, 구독 동시 실행 한도, Plus 계정에서 `rateLimits`에 5시간 창이 `primary`로, 주간이 `secondary`로 오는지.
- app-server에서 실제 한도에 걸렸을 때의 `codexErrorInfo`와 알림 순서 (문서로만 확인).
- `--ignore-user-config` 상태에서 `~/.codex/hooks.json`의 훅이 실행되는지 (사용자 훅을 실제로 돌리는 실험은 하지 않았다).
- 앱 전용 `CODEX_HOME`에 로그인하는 실제 절차와, 그것이 기존 `~/.codex` 로그인에 주는 영향.
- 카탈로그 `tool_mode: null`이 실제 모델 품질에 주는 영향 (가짜 제공자로만 확인).
- 사용량 한도에 따른 자동 모델 전환이 app-server와 exec에서 일어나는지.
- 한국 거주자에게 적용되는 OpenAI 이용약관의 판본. 개인 로컬 앱에 SIWC가 필요한지.
- `baseInstructions`와 `model_instructions_file`로 기본 지시문을 바꾼 효과.

## 5. 이 결과로 바뀌어야 할 설계 문서 항목

| 문서·항목 | 현재 | 바꿀 내용 |
|---|---|---|
| design.md 4.1 Codex 실행 방식 | `codex exec --json` + 같은 MCP 서버 | `codex app-server --listen stdio://` + 3.2의 플래그로 교체. exec는 대안 경로로 남긴다. 앱 전용 `CODEX_HOME`(별도 `codex login`), `env -i`, `model_catalog_json` 관리를 명시 |
| design.md 4.1 "처음에 보고하는 도구·MCP 목록 검사" | 두 제공자 같은 방식 | Codex는 init이 없다. (1) 앱 시작과 버전 변경 때 가짜 제공자 점검(요청 본문의 도구 목록 대조), (2) 실행마다 `instructionSources`, `sandbox`, `mcpServerStatus/list` 검사로 구체화 |
| design.md 4.1 SEC-2 판정 문구 / tasks.md AI-09 | "Codex가 충족하는지는 R-03에서" | "셸은 기능 플래그로 완전 제거. 파일 편집은 카탈로그 덮어쓰기로 제거. 웹 검색은 페이지 읽기를 포함하므로 `cached` 모드를 쓰고 보유 내역 단계에서는 끈다"를 조건으로 적는다. 카탈로그 덮어쓰기가 깨지면 AI-09를 끈다 |
| design.md SEC-2 | 셸·파일·웹 읽기 도구 없이 | "웹 읽기 없음"의 정의를 제공자별로 적는다(Claude: WebFetch 끔. Codex: `web_search="cached"` 또는 `disabled`) |
| design.md 4.1 `AiProvider.usage()` | 남은 한도 추정 | Codex: `account/rateLimits/read`(모델 호출 없음)와 `account/rateLimits/updated`. `ordinaryUsageAllowed`가 null이면 추정하지 않는다 |
| design.md 7.x REL-4 | 한도 초과는 큐 대기 | Codex 판정 기준: `codexErrorInfo == "usageLimitExceeded"`, 재개 시각은 `rateLimits.*.resetsAt`. exec 경로에서는 문장만 오므로 app-server로 다시 조회한다 |
| design.md 4.2 대화 원본 | CLI 기록 처리 방식은 R-02에서 | Codex는 `ephemeral` 스레드 + `thread/inject_items`로 디스크에 남기지 않고 이어 간다. 채팅 스레드는 app-server 프로세스가 살아 있는 동안 유지하고, 재시작하면 SQLite에서 다시 주입한다 |
| design.md 4.1 `AiProvider` 시그니처 | `run`, `resume`, `usage` | `interrupt()`와 `verify_isolation()`을 추가. `resume`이 "제공자 세션 ID"와 "앱 기록 재주입"을 모두 받게 한다 (Q11 표) |
| tasks.md AI-01 실행 이벤트 모델 | 텍스트, 도구 호출, 사용량, 종료, 한도 초과 | `TextDelta`, `ToolCall{server,tool,args,result,status}`, `WebSearch{query, action}`, `TokenUsage`, `Quota`, `RateLimited{resets_at}`, `ModelRerouted`, `McpUnavailable`로 구체화 |
| tasks.md AI-09 | Codex 어댑터 | 하위 작업 추가: app-server 스키마로 Rust 타입 생성·대조, 가짜 제공자 점검기, `CODEX_HOME` 로그인 화면, 연결 실패 시간 제한 |
| R-12 W1 / 검색어 게이트 | Codex 수단 미확인 | Codex는 검색어를 실행 전에 가로챌 수 없다. 보유 내역을 다루는 Codex 실행은 `web_search="disabled"`, 웹 조사는 보유 내역 없는 별도 스레드에서 |
| architecture.md AI CLI 신뢰 경계 | 비신뢰 | 자식 환경 변수 허용 목록, 앱 전용 `CODEX_HOME`, 셸 스냅샷 금지를 경계의 일부로 기록 |
| design.md 4.1 버전 | Codex CLI 0.167.1 | 자동 업데이트를 끈다(`check_for_update_on_startup=false`, npm 버전 고정). 버전을 올릴 때 가짜 제공자 점검과 카탈로그 갱신을 돌린다 |

## 6. 실행 검증 (명령과 출력 요약)

모든 명령은 실험 폴더(`…/scratchpad/research/R-03/`)에서 실행했다. `S`는 그 경로다.

1. 가짜 제공자 + 격리 플래그 + MCP 허용 목록 (비용 0):

```sh
codex exec --json --ephemeral --skip-git-repo-check --ignore-user-config --ignore-rules -s read-only \
  -c model_provider=fake -c 'model_providers.fake={name="fake",base_url="http://127.0.0.1:18766/v1",wire_api="responses"}' \
  -m gpt-6-sol $(cat $S/iso-flags.txt) \
  -c 'mcp_servers.stockapp={command="python3",args=["-I","…/mini_mcp.py"],required=true,enabled_tools=["get_portfolio"],default_tools_approval_mode="approve"}' \
  "probe" </dev/null
```

   결과는 Q3의 `ALL_TOOLS` 출력과 같다. 승인 모드를 주지 않았을 때는 `"MCP tool call requires approval, but approval policy is never"`였다.

2. 같은 구성에 `-c model_catalog_json="…/catalog-nopatch.json"`을 더했다. `ALL_TOOLS`에서 `apply_patch`가 사라졌다.
3. 실제 구독 실행 두 번 (gpt-6-luna, effort low). 출력은 Q1에 있다. 두 번째 실행에서는 `ALL_TOOLS = [clock__curr_time, list_mcp_resource_templates, list_mcp_resources, mcp__stockapp__get_portfolio, read_mcp_resource, web__run]`이었다. 작업 폴더에는 파일이 생기지 않았다.
4. app-server (가짜 제공자, `CODEX_HOME` 분리):
   - `thread/start`(ephemeral, `config.mcp_servers`) → `instructionSources: []`, `sandbox: {type: readOnly, networkAccess: false}`
   - `mcpServerStatus/list` → `[{"name":"stockapp","runtimeStatus":"connected","tools":["get_portfolio"]}]`
   - `thread/inject_items`로 넣은 메시지 두 개가 다음 모델 요청에 들어갔고, `sessions/`는 생기지 않았다.
5. 한도 초과 흉내 (가짜 429 `usage_limit_reached`): `error` → `turn.failed` → exit 1 (Q7).
6. 세션: 영속 실행 후 `sessions/2026/11/11/rollout-…jsonl`이 생겼고, `state_5.sqlite.threads`에 `first_user_message="two"`가 저장됐다. ephemeral 실행은 `threads` 행을 남기지 않았다.

## 7. 출처 (모두 2026-11-11 확인)

- [noninteractive] Non-interactive mode — https://learn.chatgpt.com/docs/noninteractive (developers.openai.com/codex/noninteractive에서 접근)
- [app-server] Codex app-server — https://learn.chatgpt.com/docs/app-server
- [app-server-auth] 같은 문서의 "Auth endpoints" 절 — https://learn.chatgpt.com/docs/app-server#auth-endpoints
- [config-ref] Configuration reference — https://learn.chatgpt.com/docs/config-file/config-reference
- [mcp] Model Context Protocol (CLI) — https://learn.chatgpt.com/docs/extend/mcp?surface=cli
- [hooks] Hooks — https://learn.chatgpt.com/docs/hooks
- [sdk] Codex SDK — https://learn.chatgpt.com/docs/codex-sdk
- [pricing] Codex pricing — https://learn.chatgpt.com/docs/pricing
- [ci-cd-auth] Maintain Codex account auth in CI/CD — https://learn.chatgpt.com/docs/auth/ci-cd-auth
- [siwc] Codex app-server – Sign in with ChatGPT — https://developers.openai.com/siwc/token-sharing-open-source/codex-app-server
- [exec-events] `codex-rs/exec/src/exec_events.rs` @ rust-v0.167.1 — https://github.com/openai/codex/blob/rust-v0.167.1/codex-rs/exec/src/exec_events.rs
- [release] Release 0.167.1 (2026-11-10) — https://github.com/openai/codex/releases/tag/rust-v0.167.1
- [tou] OpenAI Terms of Use (Rest of World, 시행 2026-01-01) — https://openai.com/policies/row-terms-of-use/ (openai.com이 자동 요청을 403으로 막아 r.jina.ai 리더 프록시로 원문을 읽었다)
- 보조: ConductAtlas의 약관 조항 기록 — https://conductatlas.com/platform/openai/terms-of-use-row/provision/CA-P-017130/prohibition-on-automated-data-or-output-extraction/ (제3자 분석. 조항 문구 대조에만 썼다)
- 로컬: `codex --help`, `codex exec --help`, `codex exec resume --help`, `codex app-server --help`, `codex mcp add --help`, `codex features list`, `codex debug models`, `codex app-server generate-json-schema` / `generate-ts` (0.167.1)
- 관련 리서치: `docs/research/R-02-claude-code-runtime.md`, `docs/research/R-04-rust-mcp-server.md`, `docs/research/R-12-ai-analysis-design.md`
