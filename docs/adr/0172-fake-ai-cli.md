# ADR 0172: A-11 테스트용 가짜 AI CLI

- 상태: 확정 (2026-11-24)

- 날짜: 2026-11-24
- 결정한 사람: 구현 담당 초안(메인·사용자 확인 대기)
- 관련 작업: A-11 (선행 A-03·A-04, 병합 묶음 AI-01, 연관 AI-02·AI-03·AI-04·AI-09·AI-11·M-01·M-09·C-01)
- 반영한 문서·코드: `crates/testkit/**`, `crates/adapters/ai-claude-code/tests/fake_cli_replay.rs`, `crates/adapters/ai-codex/tests/fake_cli_replay.rs`, `harness/arch-check/rules.toml`, `harness/source-checks/allowlists.toml`, `crates/xtask/src/source_check/tests.rs`, `Cargo.lock`

## 맥락

`docs/harness.md` 6.1, `docs/research/R-15-tauri-testing.md` 2.4, `docs/research/R-04-rust-mcp-server.md`는 AI 러너와 MCP 서버 계약 테스트를 위해 실제 Claude·Codex CLI와 구독·네트워크를 쓰지 않는 가짜 AI CLI 실행 파일을 요구한다.

- 녹화한 `stream-json` 출력 재생 모드와 대본 MCP 클라이언트 모드 두 가지를 제공한다.
- `PATH`를 바꾸지 않고 **실행 파일 경로 주입**으로만 교체한다.
- [ADR 0091](0091-testkit-fakes.md)에 따라 `testkit`은 작업 공간 crate에 의존하지 않는 잎 크레이트를 유지해야 한다(`domain`/`application`이 `testkit`에 개발 의존하므로 순환 의존 시 `cargo-deny`가 중단된다).
- MCP 토큰(`STOCK_MCP_TOKEN`)과 API 키 등 민감한 환경 변수를 stdout·stderr·호출 기록·`Debug` 출력에 절대 노출하지 않는다.
- 새 외부 패키지를 추가하지 않는다(`deny.toml` 무변경).

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| `crates/testkit`에 바이너리 대상 `fake_ai_cli`(`src/bin/fake_ai_cli.rs`)와 라이브러리 모듈 `fake_ai_cli`·`fake_ai_cli_replay`·`fake_mcp_client`를 두고, 외부 의존성은 이미 잠금 파일에 있는 `serde`·`serde_json`만 사용한다(`rmcp`·`tokio`·작업 공간 의존성 미추가) | `testkit`의 잎 크레이트 불변식(ADR 0091)과 새 외부 패키지 금지 규칙을 동시에 지키면서, 표준 라이브러리(`std::os::unix::net::UnixStream`, `std::process::Command`, `std::thread`) + `serde_json`만으로 두 MCP 프로토콜 세대(`2026-07-28`, `2025-06-18`)를 완전하게 구현한다 | `testkit`에 `rmcp`·`tokio` 추가(M-01이 `rmcp` 도입 중이며 새 외부 패키지 추가 금지 위반); `testkit`이 `application`에 의존(ADR 0091 순환 의존 위반) | 메인 선택지 A 결정 1·3, ADR 0091 |
| `PreparedFakeAiCli::prepare(dir, binary_name, scenario)`와 `prepare_with_binary(source_bin, dir, binary_name, scenario)`가 테스트 임시 디렉터리에 `fake_ai_cli` 실행 파일을 복사하고 같은 경로 옆 사이드카 `<binary_name>.fake-ai.json`에 대본을 저장한다 | 호출자는 `Command::new(prepared.executable_path())`로 실행 파일 경로만 주입하며 `env_clear()`로 `PATH`가 완전히 비어 있어도 동작한다. `resolve_fake_ai_cli_binary()`는 실행 중인 테스트 바이너리의 프로필 디렉터리(`debug`·`release` 등)에서 소스(`crates/testkit/src`·`Cargo.toml`)보다 최신인 미리 빌드된 바이너리를 먼저 찾고, 없거나 오래된 경우에만 해당 `target_dir`·프로필로 `--offline --locked` 빌드한다. 중첩 `cargo`를 피하려면 `prepare_with_binary`로 명시 경로를 주입한다 | `PATH` 앞쪽에 가짜 `claude`/`codex` 배치(harness 6.1 금지); `"debug"` 프로필 고정 및 오래된 바이너리 무검사 재사용; 환경 변수로 대본 전달 | harness 6.1, R-15 2.4 |
| `harness/source-checks/allowlists.toml`에는 진입 파일 1개(`crates/testkit/src/bin/fake_ai_cli.rs`)만 등록하고, 인자 읽기(`test-awareness:env::args_os`)와 문서에 명시된 5개 환경 변수 개별 키(`clock-env:var:STOCK_MCP_TOKEN`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC`, `USER`)만 허용한다 | `vars`/`vars_os` 같은 전체 환경 열람을 원천 차단하고, 키별 허용 목록으로 필요한 관찰 항목만 최소 허용한다. `crates/xtask/src/source_check/tests.rs`의 `allowlists_hold_only_the_documented_paths` 기대 집합에 해당 한 경로를 추가한다 | 전체 `vars`/`vars_os` 허용(보안 경계 약화); 라이브러리 파일까지 허용 목록 확대 | 메인 선택지 A 결정 2, architecture 2.3, design 4.1 |
| `RedactedToken`은 고정 문자열 `RedactedToken([REDACTED])`만 `Debug`로 출력하며 `Display`·`Serialize`·`Clone`을 구현하지 않는다. `argv`와 `stdin_lines` 기록은 `scrub_optional_token`으로 토큰을 `[REDACTED_TOKEN]`으로 치환하고, `ANTHROPIC_API_KEY`·`OPENAI_API_KEY`·`USER`는 존재 여부(`EnvPresence`)만 기록한다 | 가짜 CLI 자체가 토큰이나 키를 표준 출력·표준 오류·호출 기록 파일에 남기지 않도록 구조와 테스트로 고정한다 | 원시 `String`으로 토큰 보관·기록(SEC-11 및 비노출 요구 위반) | 메인 선택지 A 결정 2, SEC-11 |
| `PreflightBehavior::WaitForStdinTurns`에서는 시작 시 MCP 핸드셰이크와 `tools/list`만 수행하고, 첫 줄(`/usage`)에 `init`을 출력한 뒤 두 번째 줄이 오기 전에 stdin이 닫히면 `tools/call`을 0회 실행하고 즉시 종료한다 | AI-11("사전 점검 실패 시 실제 요청 0건")과 실제 Claude/Codex의 `/usage` 사전 점검 동작을 정확히 재현한다 | 시작 시 `tools/call`까지 미리 실행(사전 점검 실패 테스트에서 원치 않는 도구 호출 발생) | design 4.1, tasks AI-11 |
| `ScriptedAiProvider`(`AiProvider` 포트 메모리 가짜)는 이번 A-11 범위에 넣지 않고 후속 작업에 인계한다 | `docs/tasks.md` A-11 행의 완료 기준은 가짜 AI CLI 실행 파일(재생 모드·대본 MCP 클라이언트 모드·경로 주입)이며 포트 가짜는 `testkit-ports`의 책임이다 | `testkit`에 `AiProvider` 구현 추가(잎 크레이트 위반) | 메인 선택지 A 결정 4, ADR 0091 |

## 두 모드 사용법과 흉내 내는 범위

### 실행 파일 준비(공통)

1. 테스트가 `PreparedFakeAiCli::prepare(dir, "fake-claude", &FakeAiScenario { mode, capture_path: None })`를 호출한다.
2. 어댑터 또는 테스트가 `Command::new(prepared.executable_path())`로 실행 파일 경로를 직접 주입한다(`env_clear()`로 `PATH` 없이 실행 가능).
3. 필요하면 `--fake-ai-scenario <경로>`로 사이드카 대신 명시적 대본 파일을 지정할 수 있다.
4. 종료 후 `prepared.read_invocations()`로 `argv`(토큰 가림), `stdin_lines`(토큰 가림), `EnvObservation`(존재 여부만), 발견된 MCP 도구 목록, 실행된 MCP 도구 호출 수, 종료 코드를 검증한다.

### 모드 1: 녹화 `stream-json` 재생 (`FakeAiMode::Replay`)

- `immediate_lines`: 시작 즉시 stdout으로 출력할 줄 목록(`fixtures/ai/*.jsonl` 등).
- `turns`: stdin 한 줄을 읽을 때마다 대응 `stdout_lines`·`stderr_lines` 출력 후 `TurnStop::Continue` 또는 `TurnStop::Terminate`(다중 턴 채팅 및 `/usage` 사전 점검 대본 지원).
- `trailing_without_newline`: 줄바꿈 없는 잘린 마지막 조각(프로세스 중단·깨진 줄 재현).
- `final_stderr_lines`, `exit_code`: 종료 전 stderr 출력 및 임의 종료 코드(0 포함 비정상 종료 코드).

### 모드 2: 대본 MCP 클라이언트 (`FakeAiMode::ScriptedMcpClient`)

- **두 프로토콜 세대(`McpProtocolGeneration`)**:
  - `V2026_07_28`(Claude Code 2.1.322): `server/discover` → `tools/list` → `tools/call`. R-04 규약에 따라 세 요청 모두 `params._meta["io.modelcontextprotocol/protocolVersion"] = "2026-07-28"`과 `params._meta["clientCapabilities"]`를 싣는다.
  - `V2025_06_18`(Codex 0.167.1): `initialize` → `notifications/initialized` → `tools/list` → `tools/call`.
  - JSON-RPC 응답 대조: 서버가 먼저 보내는 알림(`id` 없음)은 건너뛰고, 요청 `id`가 일치하는 응답만 수용하며 불일치 시 실패한다.
- **세 가지 전송 경로(`McpTransportSpec`)**:
  - `FromClaudeMcpConfig`: `--mcp-config <파일>`을 읽어 `mcpServers.<name>.command/args/env`를 파싱하고, `env`의 `"${STOCK_MCP_TOKEN}"`(전체 및 `"Bearer ${STOCK_MCP_TOKEN}"` 부분 치환)을 자식 프로세스 환경으로 치환해 stdio 브리지 프로세스를 띄운다. 설정 파일 누락·JSON 구문 오류는 MCP 장애로 가리지 않고 비정상 종료 코드 `2`로 즉시 실패한다.
  - `FromCodexConfigFlags`: `-c mcp_servers.<name>.command/args/env_vars`를 파싱하며(JSON 쌍따옴표, TOML 홑따옴표 `'...'`, 따옴표 없는 단순 문자열 및 홑·쌍따옴표 배열 지원), `env_vars`에 `"STOCK_MCP_TOKEN"`이 포함된 경우에만 토큰을 자식 환경으로 전달한다(미포함 시 토큰 누락 → 인증 실패 재현).
  - `DirectUnixSocket { socket_path }`: Unix 도메인 소켓에 직접 연결해 첫 줄 `AUTH <token>\n` 인증 후 JSON-RPC 2.0을 주고받는다.
- **내장 stdio↔UDS 브리지(`--fake-ai-bridge <socket_path>`)**: 테스트에서 별도 `mcp-bridge` 바이너리 없이도 Claude/Codex의 실제 stdio 브리지 구조를 종단 간 시험할 수 있다. 서버가 `AUTH` 거절로 응답 없이 연결을 닫으면 종료 코드 `2`로 실패한다.
- **세 가지 출력 와이어 형식(`CliWireFormat`)**: `ClaudeStreamJson`, `CodexAppServerJsonRpc`, `CodexExecJsonl` — 모두 AI-01 디코더로 그대로 파싱되어 동일한 `application::AiEvent` 시퀀스를 만든다.
- **SEC-2·오류·사전 점검 주입**: `InitOverride`(비정상 `permissionMode`·`apiKeySource`·`skills`·`slash_commands`·`plugins`·`extra_tools`·`sandbox_type`·`network_access`·`instruction_sources`·`multi_agent`·`cli_version`·`model`/`actual_model`), `ANTHROPIC_API_KEY` 존재 시 `apiKeySource: "env"` 자동 반영, `McpFailureBehavior::ContinueWithFailedInit`(실제 CLI처럼 MCP 실패 후에도 종료 코드 0으로 `status: "failed"` init 출력) / `ExitWithCode(u8)`, `PreflightBehavior::Immediate` / `WaitForStdinTurns`.

## 승인된 보호 파일·기존 테스트 변경

메인 선택지 A 결정으로 사전 승인된 변경은 다음 3개 파일뿐이며, 그 외 보호 파일과 기존 테스트 단언은 변경하지 않았다.

| 파일 | 변경 내용 | 비고 |
|---|---|---|
| `harness/arch-check/rules.toml` | `ai-claude-code`·`ai-codex`의 `serde_json`·`proptest`, `testkit`의 `serde`·`serde_json` 경유 `syn`·`proc-macro2` `[[external-location-exception]]` 최소 등록 | 기존 crate들과 동일 패턴, `deny.toml` 무변경(`Cargo.lock` 외부 패키지 변경 없음, 작업 공간 crate 3개의 기존 잠금 의존 목록만 갱신) |
| `harness/source-checks/allowlists.toml` | `crates/testkit/src/bin/fake_ai_cli.rs` 단일 파일에 `test-awareness:env::args_os` 및 `clock-env:var:<KEY>` 5개(`STOCK_MCP_TOKEN`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC`, `USER`) 등록 | `vars`/`vars_os` 미허용 |
| `crates/xtask/src/source_check/tests.rs` | `allowlists_hold_only_the_documented_paths` 기대 집합에 `"crates/testkit/src/bin/fake_ai_cli.rs"` 1줄 추가(기존 경로와 동등성 단언 유지) | 지적: `test-diff:modified:xtask::bin/xtask source_check::tests::allowlists_hold_only_the_documented_paths`<br>SHA-256 전: `a8843268f389f630347ee3c7efc967b5c94952dbae06ab5891c91378de75980a`<br>SHA-256 후: `99c05ade3aedf8826f692dab901c36078f1fb29c894c1f68a48fd3552a5d2111` |

## 정의 대기

| 항목 | 현재 안전한 기본값 | 선택지 | 사용자 영향 |
|---|---|---|---|
| 1. M-01(`rmcp` 도입) 이후 대본 MCP 클라이언트의 `rmcp` 전환 여부 | 순수 `std` + `serde_json`으로 두 프로토콜 세대 구현(`testkit` 경량 잎 유지) | A) 현 순수 `std` + `serde_json` 유지(빠른 컴파일, `rmcp` 서버와 독립된 wire 검증) / B) M-01 병합 뒤 `testkit`의 클라이언트 구현을 `rmcp` 클라이언트 기능으로 교체(`architecture.md` 3.2 표 반영) | 테스트 전용 크레이트라 사용자 런타임 영향 없음. A는 `rmcp` 서버 자체의 프로토콜 버그를 독립 클라이언트 관점에서 잡기 유리하고 빌드가 가볍다. B는 프로토콜 세부 구현 중복을 줄인다 |
| 2. `ScriptedAiProvider`(`AiProvider` 포트 메모리 가짜) 구현 위치와 시점 | A-11 범위 밖으로 분리(미구현) | A) `testkit-ports`에 추가(C-01·AI-02 유스케이스 테스트 선행 시) / B) 실제 어댑터 + 가짜 CLI 경로만 사용하고 포트 가짜는 생략 | 사용자 런타임 영향 없음. A는 상위 유스케이스(C-01·E 묶음)를 자식 프로세스 없이 빠르게 시험할 수 있다. B는 유지할 가짜 수를 줄인다 |

## 인계

| 받는 작업 | 인계 내용 |
|---|---|
| AI-02, AI-03, AI-04 | `PreparedFakeAiCli` 경로 주입(`PATH` 무변경)으로 녹화 스트림 재생, 환경 변수 허용 목록 검증(`EnvObservation`), 다중 턴 채팅 유지·재주입 테스트 |
| AI-11 | `PreflightBehavior::WaitForStdinTurns` + `InitOverride`로 design 4.1 8항목 위반마다 실제 도구 요청 0건 계약 테스트 |
| AI-09 | `CliWireFormat::CodexAppServerJsonRpc` + `McpProtocolGeneration::V2025_06_18` + `FromCodexConfigFlags`로 Codex 어댑터 계약 테스트 |
| M-01, M-09 | 실제 `mcp-server`·`mcp-bridge`에 `fake_ai_cli`의 두 프로토콜 세대(`2026-07-28`, `2025-06-18`) 클라이언트를 연결하는 종단 간 테스트 및 향후 `rmcp` 클라이언트 교체 검토(정의 대기 1번) |
| C-01 / `testkit-ports` | 필요 시 `ScriptedAiProvider` 메모리 포트 가짜 추가 검토(정의 대기 2번) |

## 실측 및 손 변형

### 커버리지 및 자동 변이

- 자동 변이(`just mutants-daily`): 대상 밖(사용자 결정 2026-11-24), 아래 격리 사본 손 변형으로 갈음.
- `cargo llvm-cov nextest --locked -p ai-claude-code -p ai-codex -p testkit --summary-only` 요약 (`fake_ai_cli` 바이너리 본체는 자식 프로세스로 `env_clear()` 실행되어 `LLVM_PROFILE_FILE`이 전달되지 않으므로 `llvm-cov` 계측에 잡히지 않으며, 실제 실행 검증은 아래 15건의 격리 사본 손 변형과 통합 테스트가 담당한다):

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

### 격리 사본 손 변형 15건 (A-11 몫, 묶음 총 35/35 검출, 생존 0, 컴파일 불가 0)

`target/AI-01/mutations/copy`에서 각 변형을 하나씩 적용해 `--no-run` 컴파일 및 `fake_ai_cli` 바이너리 재빌드 성공 후 `cargo nextest run --locked -p testkit`으로 검출을 확인하고 원복·해시 대조·기준 테스트 재통과 후 임시 사본을 삭제했다.

| 번호 | 파일 | 바꾼 것 | 결과 | 검출한 테스트 |
|---|---|---|---|---|
| 15 | `crates/testkit/src/fake_ai_cli.rs` | RedactedToken Debug에 원본 토큰 노출 | 컴파일 성공·검출 | `testkit::fake_ai_cli tests::claude_2026_07_28_mcp_config_bridge_executes_tools_and_never_leaks_token` |
| 16 | `crates/testkit/src/fake_ai_cli_replay.rs` | scrub_optional_token 토큰 가림 생략 | 컴파일 성공·검출 | `testkit::fake_ai_cli tests::bad_token_sec2_overrides_preflight_and_argv_scrubbing_are_enforced` |
| 17 | `crates/testkit/src/fake_ai_cli_replay.rs` | trailing_without_newline 출력 생략 | 컴파일 성공·검출 | `testkit::fake_ai_cli tests::replay_mode_injects_executable_without_path_and_handles_multiturn_broken_and_exit` |
| 18 | `crates/testkit/src/fake_ai_cli.rs` | run_from_process 종료 코드를 항상 0으로 고정 | 컴파일 성공·검출 | `testkit::fake_ai_cli tests::mcp_client_rejects_mismatched_response_id` |
| 19 | `crates/testkit/src/fake_mcp_client.rs` | stdio 브리지 AUTH 헤더 전송 생략 | 컴파일 성공·검출 | `testkit::fake_ai_cli tests::claude_2026_07_28_mcp_config_bridge_executes_tools_and_never_leaks_token` |
| 20 | `crates/testkit/src/fake_mcp_client.rs` | 2026-07-28 핸드셰이크 메서드를 initialize로 변경 | 컴파일 성공·검출 | `testkit::fake_ai_cli tests::claude_2026_07_28_mcp_config_bridge_executes_tools_and_never_leaks_token` |
| 21 | `crates/testkit/src/fake_mcp_client.rs` | 2025-06-18 notifications/initialized 알림 생략 | 컴파일 성공·검출 | `testkit::fake_ai_cli tests::codex_2025_06_18_flags_require_env_vars_and_propagate_result` |
| 22 | `crates/testkit/src/fake_mcp_client.rs` | Claude mcp-config ${STOCK_MCP_TOKEN} 치환 생략 | 컴파일 성공·검출 | `testkit::fake_ai_cli tests::claude_2026_07_28_mcp_config_bridge_executes_tools_and_never_leaks_token` |
| 23 | `crates/testkit/src/fake_mcp_client.rs` | Codex env_vars 없이도 STOCK_MCP_TOKEN 무조건 전달 | 컴파일 성공·검출 | `testkit::fake_ai_cli tests::codex_flags_without_env_vars_omit_token_and_fail_handshake` |
| 24 | `crates/testkit/src/fake_mcp_client.rs` | ANTHROPIC_API_KEY 존재 시 apiKeySource를 none으로 고정 | 컴파일 성공·검출 | `testkit::fake_ai_cli tests::bad_token_sec2_overrides_preflight_and_argv_scrubbing_are_enforced` |
| 25 | `crates/testkit/src/fake_mcp_client.rs` | preflight 뒤 stdin EOF에서도 본문·도구 호출 강제 실행 | 컴파일 성공·검출 | `testkit::fake_ai_cli tests::bad_token_sec2_overrides_preflight_and_argv_scrubbing_are_enforced` |
| 26 | `crates/testkit/src/fake_mcp_client.rs` | McpFailureBehavior::ExitWithCode 무시하고 0 반환 | 컴파일 성공·검출 | `testkit::fake_ai_cli tests::mcp_client_rejects_mismatched_response_id` |
| 27 | `crates/testkit/src/fake_mcp_client.rs` | Codex MCP 실패 시 runtimeStatus를 connected로 위장 | 컴파일 성공·검출 | `testkit::fake_ai_cli tests::codex_flags_without_env_vars_omit_token_and_fail_handshake` |
| 34 | `crates/testkit/src/fake_mcp_client.rs` | 2026-07-28 _meta 프로토콜 버전 변조 | 컴파일 성공·검출 | `testkit::fake_ai_cli tests::claude_2026_07_28_mcp_config_bridge_executes_tools_and_never_leaks_token` |
| 35 | `crates/testkit/src/fake_mcp_client.rs` | read_response JSON-RPC 응답 id 불일치 거부 제거 | 컴파일 성공·검출 | `testkit::fake_ai_cli tests::mcp_client_rejects_mismatched_response_id` |

## 결과

- `crates/testkit`이 잎 크레이트 불변식과 새 외부 패키지 금지 규칙을 유지한 채, `PATH` 변경 없이 경로 주입으로 교체 가능한 가짜 AI CLI 실행 파일(`fake_ai_cli`)을 제공한다.
- `ai-claude-code`·`ai-codex`·`testkit` 통합 테스트가 빈 `PATH` 재생, 두 세대 MCP 핸드셰이크·도구 호출, 토큰 비노출, 사전 점검 실패 시 도구 호출 0건을 실제로 검증한다.
- 바뀌면 다시 볼 조건: M-01에서 `rmcp`가 도입되어 클라이언트 구현 통합을 결정하거나(정의 대기 1번), Claude/Codex CLI의 `--mcp-config`/`-c mcp_servers` 인자 규약이 바뀔 때.
