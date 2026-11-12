# R-10 에이전트 하네스와 리뷰 절차

- 조사일: 2026-11-11 (KST)
- 대상 버전: Claude Code CLI 2.1.322 (`claude --version`으로 확인), macOS 26.5.2, git 2.45.2(로컬), lefthook v2.2.2, just 1.59.0, Secretive v4.0.0 (각 GitHub 최신 릴리스, 2026-11-11 확인)
- 방법: code.claude.com 공식 문서(hooks, sub-agents, permissions, sandboxing, settings-reference, env-vars, worktrees, best-practices, managed-settings)의 마크다운 원문을 내려받아 읽었다. 연구·실무 근거는 arXiv 논문과 Anthropic 엔지니어링 글을 1차 출처로 썼다. 이 PC에서 `claude -p --model haiku`로 실험 12번을 돌렸다. 실험 파일은 세션 scratchpad의 `research/R-10/exp/` 아래에만 두었다(아래에서 `<exp>`로 줄여 쓴다).
- 실험 공통 명령: `claude -p --model haiku --setting-sources project --permission-mode acceptEdits --permission-prompts none --no-session-persistence "<지시>"`. 사용자 전역 설정을 빼고 구독 로그인으로 실행됐다.

## 1. 요약

1. 보호 경로는 **세 겹으로 막아야 한다**. 권한 `deny` 규칙과 `PreToolUse` 훅은 파일 도구와 셸 리다이렉트만 막는다. `python3 -c "open(...)"` 같은 셸 쓰기는 **샌드박스(Seatbelt)의 `denyWrite`만 막는다**(실측). 샌드박스는 거부 목록 방식이다. `denyWrite: ["."]` 뒤에 `allowWrite: ["./src"]`를 두어도 다시 열리지 않는다(실측). 그래서 "허용 목록" 규칙은 로컬에서는 거부 목록으로 근사하고, 정확한 허용 목록 검사는 CI가 맡는다.
2. 훅 규약은 확인됐다. `PreToolUse`는 `hookSpecificOutput.permissionDecision: "deny"`나 종료 코드 2로 막는다. `PostToolUse`는 `decision: "block"`과 `reason`으로 결과를 돌려준다. `SubagentStop`은 `decision: "block"`으로 종료를 막는다. **설정 파일의 훅은 서브에이전트 안에서도 돈다.** 입력에 `agent_type`과 `agent_id`가 들어 있어 역할별로 다르게 처리할 수 있다(실측). 연속 차단 상한은 기본 8회다. "3회 차단 뒤 실패 상태로 종료 허용"은 훅이 직접 횟수를 세야 한다(실측).
3. 서브에이전트 정의의 `model`은 **고정이 아니다**. 호출할 때 넘기는 `model` 인자가 정의보다 우선한다. 포크는 부모 맥락을 통째로 물려받는다. `PreToolUse(Agent)` 훅으로 `model` 인자, 허용되지 않은 종류, 포크를 거부할 수 있다(실측). `CLAUDE_CODE_FORK_SUBAGENT=0`과 `Agent(fork)` 거부를 함께 쓴다.
4. `isolation: worktree`로 띄운 리뷰어도 셸로 **원본 작업 사본에 쓸 수 있다**(실측: 절대 경로 쓰기 성공). 그래서 `SubagentStart`/`SubagentStop` 훅으로 원본 상태를 비교해 막는다. 이 방법은 실제로 동작해 리뷰어가 스스로 되돌렸다(실측). 또 `Edit(/.claude/**)` 같은 넓은 거부 규칙은 `.claude/worktrees/` 아래 리뷰어 작업 사본의 쓰기까지 막는다(실측). 좁게 써야 한다.
5. 근거: 생성자와 평가자를 나누고 평가자를 회의적으로 조정하는 쪽이 효과적이라는 Anthropic 보고가 있다. PR 설명을 조작하면 Claude Code 리뷰를 97% 속일 수 있다는 2026년 연구도 있다. 둘 다 "리뷰어에게 구현자 설명을 주지 않는다"를 지지한다. 반박 단계가 오탐을 얼마나 줄이는지에 대한 정량 연구는 찾지 못했다(**미확인**). 공식 code-review 플러그인도 지적별 검증 에이전트를 쓰지만 수치는 공개하지 않았다.

## 2. 질문별 답

### Q1. 훅의 종류와 규약

현재 이벤트는 33종이다: `SessionStart`, `Setup`, `UserPromptSubmit`, `UserPromptExpansion`, `PreToolUse`, `PermissionRequest`, `PermissionDenied`, `PostToolUse`, `PostToolUseFailure`, `PostToolBatch`, `Notification`, `MessageDisplay`, `SubagentStart`, `SubagentStop`, `TaskCreated`, `TaskCompleted`, `Stop`, `StopFailure`, `TeammateIdle`, `InstructionsLoaded`, `ConfigChange`, `CwdChanged`, `DirectoryAdded`, `FileChanged`, `WorktreeCreate`, `WorktreeRemove`, `PreCompact`, `PostCompact`, `PreModelSwitch`, `PostModelSwitch`, `Elicitation`, `ElicitationResult`, `SessionEnd`. [hooks]

처리기 종류는 `command`, `http`, `mcp_tool`, `prompt`, `agent` 다섯 가지다. 공식 문서는 `prompt`와 `agent` 훅도 모델의 판단이라 결정적이지 않다고 한다. 강제에는 `command`나 `http`를 쓰고, 필요하면 `onFailure: "block"`을 붙이라고 권한다. [best-practices]

**공통 입력**(stdin JSON): `session_id`, `prompt_id`, `transcript_path`, `cwd`, `scratchpad_dir`, `permission_mode`, `effort`, `hook_event_name`. 서브에이전트 안에서 실행되면 `agent_id`와 `agent_type`이 추가된다. `agent_type`은 정의 파일의 `name`이다. [hooks]

**종료 코드**: 0이면 성공이다. stdout은 JSON 객체 하나일 때만 JSON으로 읽는다. 2는 차단할 수 있는 이벤트에서 차단이고, stderr가 이유가 된다. 그 밖의 코드는 차단하지 않는 오류다. 다만 v2.1.295부터 `command`나 `http` 훅에 `"onFailure": "block"`을 주면, 실행 실패·타임아웃·잘못된 출력도 종료 코드 2처럼 처리한다. **`Stop`과 `SubagentStop`에서는 이 옵션이 효과가 없다.** [hooks]

| 목적 | 이벤트 | 입력(이벤트 고유) | 출력 |
|---|---|---|---|
| 편집 전 차단 | `PreToolUse` (matcher `Edit\|Write\|NotebookEdit`) | `tool_name`, `tool_input`(`file_path`, `content`/`old_string`/`new_string` 등, 경로는 항상 절대 경로), `tool_use_id` | `hookSpecificOutput.permissionDecision`: `allow`/`deny`/`ask`/`defer`, `permissionDecisionReason`(deny이면 Claude에게 보임), `updatedInput`, `additionalContext`. 여러 훅의 결정이 다르면 deny > defer > ask > allow 순으로 이긴다. 종료 코드 2는 deny와 같다 |
| 셸 차단 | `PreToolUse` (matcher `Bash`) | `tool_input.command`, `description`, `timeout`, `run_in_background`, `dangerouslyDisableSandbox` | 위와 같음 |
| 편집 후 검사 결과 돌려주기 | `PostToolUse` | `tool_input`, `tool_response`, `duration_ms` | 최상위 `decision: "block"` + `reason`: 이유가 도구 결과 옆에 붙어 Claude에게 간다. `hookSpecificOutput.additionalContext`, `updatedToolOutput`. 종료 코드 2는 stderr를 Claude에게 보여 줄 뿐 이미 실행된 도구는 되돌리지 않는다 |
| 메인 종료 차단 | `Stop` | `stop_hook_active`, `last_assistant_message`, `background_tasks`, `session_crons` | `decision: "block"` + `reason`(필수), 또는 `hookSpecificOutput.additionalContext`(오류 표시 없이 계속) |
| 서브에이전트 종료 차단 | `SubagentStop` (matcher = 에이전트 이름) | 위에 더해 `agent_id`, `agent_type`, `agent_transcript_path` | `Stop`과 같다. `reason`이 서브에이전트의 다음 지시가 된다 |

모든 이벤트가 `continue: false`와 `stopReason`, `systemMessage`(사용자에게 보이는 경고)를 받는다. `suppressOutput`은 받기만 하고 효과가 없다. `additionalContext`, `systemMessage`, 일반 stdout은 각각 10,000자까지다. 넘으면 파일로 저장되고 앞 2,000자만 보인다. [hooks]

**서브에이전트 적용**: 설정 파일·관리 설정·플러그인의 훅은 서브에이전트 안에서도 돈다. `PreToolUse`와 `PostToolUse`도 서브에이전트의 도구 호출마다 실행된다. [hooks], [sub-agents]

실측 결과(실험 7, 10): implementer의 `Write`에서 `guard_edit` 훅이 `agent_type: "implementer"`, `agent_id: "a98231fbd25211cf7"`를 받았다. 리뷰어의 Bash에서는 `agent_type: "reviewer"`를 받았다. 메인의 도구 호출에서는 `agent_type`이 없다(`null`).

**서브에이전트 정의 파일 안의 훅**은 그 서브에이전트가 도는 동안만 등록된다. 그 안의 `Stop`은 `SubagentStop`으로 바뀐다. 다만 프로젝트 서브에이전트의 정의 훅은 **작업 공간 신뢰 대화상자를 받아들인 뒤에만** 돈다. `-p` 실행은 신뢰로 치지 않는다. 설정 파일 훅은 `-p`에서 신뢰된 것으로 보고 돈다. [hooks], [sub-agents] → 하네스 훅은 정의 파일에 두지 말고 `.claude/settings.json`에 두고, `agent_type`으로 나눈다.

**작은 함정**:
- `@` 파일 참조로 넣은 내용에는 `PreToolUse`가 돌지 않는다. [hooks]
- `if` 필드의 Bash 패턴 일치는 최선 노력이다. 문서가 "단단한 허용·거부는 권한 시스템으로 하라"고 적는다. [hooks]
- 권한 `deny` 규칙에 걸린 호출에는 `PreToolUse` 훅이 아예 실행되지 않았다. 실험 1에서 `justfile` 편집은 "denied by your permission settings"로 끝났고, 훅 기록이 남지 않았다. 따라서 거부 사유를 훅이 꾸밀 수는 없다.

### Q2. 특정 경로를 수정하지 못하게 하는 방법

| 수단 | 막는 것 | 못 막는 것 | 실측 |
|---|---|---|---|
| 권한 `deny` 규칙 `Edit(/path)` | Edit/Write/NotebookEdit. Claude Code가 아는 셸 파일 명령(`cat`, `sed`, `tee` 등). 리다이렉트 대상(`> file`) | 스크립트가 직접 여는 파일(`python -c`, `node -e`), `cp`, 이름 없이 읽는 명령 [permissions] | 실험 1: Write 차단, `echo >> justfile` 차단 |
| `PreToolUse` 훅 | 위와 같은 도구 호출 + 사용자 정의 판단(역할별 허용 목록, 경로 정규화) | 셸 안에서 간접적으로 일어나는 쓰기(명령 문자열만 보이므로) | 실험 1, 7: 허용 목록 밖 경로 거부 |
| 샌드박스 `sandbox.filesystem.denyWrite` (macOS Seatbelt) | Bash와 그 자식 프로세스의 모든 쓰기. `python`, `cp`, `sh -c` 포함. `Edit` deny 규칙도 자동으로 여기에 합쳐진다 [sandboxing], [settings-reference] | 내장 파일 도구(이건 권한 규칙이 맡는다), 훅·MCP 서버 프로세스(샌드박스 밖에서 돈다) | 실험 1, 5, 6: `python3 -c open(...)`, `cp`, `sh -c` 모두 차단 |
| 샌드박스 기본 보호 경로 | `.claude/settings*.json`, `.claude/{skills,agents,commands,hooks}`, `.mcp.json`, `.git/hooks`, `.git/config`, `~/.claude` 대부분 [sandboxing] | 프로젝트 고유 파일(`justfile`, `eslint.config.*` 등)은 직접 적어야 한다 | 실험 1: `.claude/settings.json` 쓰기 차단 |
| 파일 시스템 권한·플래그(`chmod`, `chflags uchg`) | 일반 쓰기 | 같은 사용자는 `chflags nouchg`로 풀 수 있다. `git checkout`도 깨진다 | 미실험. 보조 수단으로도 권하지 않는다 |
| 별도 사용자 계정·컨테이너 | 전부 | 설정 비용이 크다. 구독 로그인을 다시 해야 한다 | 미실험 |
| CI 검사(서명 + 허용 목록) | 로컬에서 놓친 모든 것. 로컬 훅을 꺼도 걸린다 | 커밋하기 전의 작업 사본 상태 | 초안만 작성(3.4) |

**셸 우회를 막을 수 있는가**: 샌드박스를 켜고 다음을 설정하면 막을 수 있다(실측).
- `allowUnsandboxedCommands: false`: 이것이 없으면 Claude가 `dangerouslyDisableSandbox`로 샌드박스 밖에서 다시 실행하자고 요청할 수 있다.
- `failIfUnavailable: true`: 샌드박스를 띄울 수 없을 때 샌드박스 없이 실행하지 않는다.

실험에서 확인한 제약은 다섯 가지다.

1. 샌드박스 쓰기 규칙은 **거부가 이긴다**. 실험 2에서 `denyWrite: ["."]`와 `allowWrite: ["./src", "./.git"]`를 함께 두자 `src/b.txt`도 `.git/index.lock`도 막혔다. 문서도 "deny 안을 다시 여는" 기능은 읽기(`allowRead`)에만 적혀 있다. 그래서 harness.md 7.1의 허용 목록을 샌드박스로 그대로 옮길 수 없다. 보호 대상 경로와 패턴(`./*.ts`, `./crates/*/Cargo.toml` 등)을 거부 목록으로 적어야 한다. 목록에 없는 새 최상위 파일(예: `vitest.config.ts`를 못 막는 패턴)은 로컬에서 새어 나간다. 그것은 CI가 잡는다.
2. **`--settings`로 넘긴 상대 경로(`./x`)는 프로젝트 기준으로 풀리지 않았다.** 실험 3, 4에서 모든 쓰기가 허용됐다. 같은 경로를 절대 경로로 주자 막혔다(실험 5). 프로젝트 `.claude/settings.json` 안의 `./` 경로는 프로젝트 기준으로 풀렸다. 와일드카드, 아직 없는 디렉터리, 패턴에 맞는 새 파일도 막혔다(실험 6). 문서는 `./`가 프로젝트 설정에서는 프로젝트 루트, 사용자 설정에서는 `~/.claude` 기준이라고만 적는다. `--settings`의 기준은 적혀 있지 않다(**미확인**). → 경로 규칙은 프로젝트 설정에 둔다.
3. 내장 파일 도구는 샌드박스 밖에서 돈다. 그래서 `Write`를 막는 일은 권한 규칙과 훅이 해야 한다. [sandboxing]
4. 훅과 로컬 MCP 서버는 샌드박스 밖에서 사용자 권한 전체로 돈다. [sandboxing] 따라서 **훅 스크립트 자체가 에이전트가 쓸 수 있는 곳에 있으면 안 된다.** `.claude/hooks/`는 샌드박스 기본 보호 경로라 안전하다. 반면 `target/debug/xtask` 같은 빌드 산출물을 훅으로 부르면 에이전트가 빌드로 바꿔치기할 수 있다.
5. 샌드박스는 Bash 명령을 막는다. 샌드박스 쓰기 가능 영역은 작업 디렉터리, 사용자별 임시 디렉터리, 추가 디렉터리다. 홈 디렉터리는 기본으로 쓸 수 없다. 그래서 하네스 상태 파일(검사 통과 기록 등)은 `~/.local/state/...`처럼 **작업 사본과 임시 디렉터리 밖**에 두어야 위조되지 않는다. `.git/` 안이나 `$TMPDIR`은 샌드박스 안에서 쓸 수 있다.

### Q3. 서브에이전트 정의 파일의 현재 형식

정의 파일은 `.claude/agents/*.md`의 YAML 앞머리와 본문(시스템 프롬프트)이다. 필수는 `name`과 `description`뿐이다. 모르는 필드는 **오류 없이 무시된다**. 철자 오류는 `claude --debug` 로그에서 `does not recognize or apply`로 찾는다. [sub-agents]

| 필드 | 이 하네스에서의 의미 |
|---|---|
| `model` | `claude-opus-5-5` 같은 전체 ID나 별칭. **우선순위: 호출할 때의 `model` 인자 > 정의의 `model` > `CLAUDE_CODE_SUBAGENT_MODEL` > 메인 모델.** 정의만으로는 고정되지 않는다. `CLAUDE_CODE_SUBAGENT_MODEL_FORCE=1`을 쓰면 모든 서브에이전트(내장 Explore 포함)가 한 모델로 고정되고, 정의의 `model`은 무시된다 [sub-agents] |
| `tools` / `disallowedTools` | 허용 목록 / 거부 목록. 둘 다 있으면 거부를 먼저 적용한다. `disallowedTools`의 `Bash(git push *)` 같은 지정자는 **도구 전체를 뺀다**. 명령 단위로 막으려면 권한 `deny`를 쓴다 |
| `permissionMode` | `default`, `acceptEdits`, `auto`, `dontAsk`, `bypassPermissions`, `plan`. **메인이 `bypassPermissions`, `acceptEdits`, `auto` 모드이면 무시되고 메인의 모드를 따른다** |
| `isolation: worktree` | 임시 git 작업 사본에서 돈다. 기본 기준은 원격 기본 브랜치다(`worktree.baseRef: "fresh"`) |
| `omitClaudeMd: true` | CLAUDE.md·AGENTS.md를 빼고 시작한다(v2.1.271+) |
| `maxTurns`, `effort`, `hooks`, `skills`, `mcpServers`, `memory`, `background`, `color` 등 | — |

**도구 필터**: 서브에이전트는 깊이 한도(기본 3단)에 닿으면 `Agent`를, 그리고 `AskUserQuestion`을 자동으로 잃는다. 백그라운드 서브에이전트는 내장 도구가 더 줄어든다. [sub-agents]

**부모 맥락을 물려받지 않게 하는 방법**: 포크가 아닌 서브에이전트는 항상 새 맥락으로 시작한다. 받는 것은 다음뿐이다. [sub-agents]
- 자기 시스템 프롬프트
- 메인이 쓴 위임 메시지
- CLAUDE.md 계층(`omitClaudeMd`가 없으면)
- git 상태 스냅샷
- 미리 불러온 스킬

맥락을 물려받는 경로는 두 가지다. 둘 다 막아야 한다.
1. **포크**: v2.1.302부터 포크 모드가 기본으로 켜져 있다. 포크는 메인의 대화 전체를 물려받는다. → `CLAUDE_CODE_FORK_SUBAGENT=0`과 권한 `deny`의 `Agent(fork)`를 함께 쓴다.
2. **재개**: `SendMessage`로 끝난 서브에이전트를 다시 깨우면 이전 대화 전체를 그대로 이어 간다. workflow.md의 "수정은 새 implementer"를 지키려면 `SendMessage`를 메인에서 금지하는 편이 확실하다.

`PreToolUse(Agent)` 훅의 입력에는 `subagent_type`, `prompt`, `description`, `model`, `run_in_background`가 들어온다(실측). 실험 12의 결과는 다음과 같다.
- `model: "haiku"`를 붙인 호출 → 거부됨
- `subagent_type: "general-purpose"` → 거부됨
- `subagent_type`가 없는 포크 시도 → 거부됨

`PostToolUse(Agent)`의 `tool_response.resolvedModel`로 실제 모델을 사후 확인할 수 있다(실측: `claude-haiku-5-5`).

### Q4. "구현 → 적대적 리뷰 → 반박 → 메인 판정"의 효과 근거

| 근거 | 내용 | 이 절차에 주는 의미 |
|---|---|---|
| Anthropic, "Harness design for long-running application development"(2026-03-24) [anthropic-harness] | "일하는 에이전트와 판정하는 에이전트를 나누는 것이 강한 지렛대"다. 독립된 평가자를 회의적으로 조정하는 것이 생성자가 자기 결과를 비판하게 만드는 것보다 "훨씬 다루기 쉽다". 실패 유형도 적었다: 평가자가 진짜 문제를 찾고도 "큰 문제가 아니라고 스스로 설득해" 승인했다. 경계 사례를 찌르지 않고 피상적으로 테스트했다 | 리뷰어 분리를 지지한다. 리뷰어가 스스로 심각도를 낮추지 못하게, 판정은 비판자와 메인에게 넘긴다. 재현 명령을 요구한다 |
| Claude Code best practices [best-practices] | 새 서브에이전트 리뷰는 "코드를 만든 추론에 흔들리지 않는다". "문제를 찾으라고 하면 리뷰어는 항상 *무언가* 찾는다. 다 고치면 쓸데없는 추상화와 일어날 수 없는 경우의 테스트가 생긴다" → `skip style`을 넣으라고 한다 | 형식·취향 지적 금지를 명시한다. 등급 기준이 필요하다 |
| Alexopoulos 외, "When AI Agrees Too Much: Manipulating Automated Code Review via Bias Injection"(arXiv 2603.18740, v5 2026-10-27) [bias-injection] | 6개 LLM, 33개 CVE에 대해 실험했다. PR 메타데이터(설명)를 다듬어 보안 판정을 바꾸는 공격이 Claude Code와 CodeRabbit 리뷰 파이프라인에서 33건 중 32건(97%) 성공했다. 단순한 틀 문구는 오히려 역효과였다 | **리뷰어에게 구현자 보고서를 주지 않는다**는 결정을 지지한다. 주는 것은 메인이 쓴 브리프(의도)뿐이다 |
| "Don't Judge Code by Its Cover"(arXiv 2505.16222, 저자 미확인) [cover] | 코드 속 "전문가가 썼다"는 주석(권위 편향)과 틀린 작업 설명 주석이 LLM 판정을 흐린다 | 주석 금지 규칙이 리뷰 품질에도 도움이 된다 |
| McAleese 외, "LLM Critics Help Catch LLM Bugs"(CriticGPT, arXiv 2407.00215) [criticgpt] | 비판 모델은 사람 계약자보다 버그를 더 찾는다. 하지만 환각 버그와 사소한 지적도 낸다. 사람과 비판 모델이 함께하면 모델 혼자보다 환각이 적다. 정밀도와 재현율은 서로 맞바꾸는 관계다 | 리뷰어(재현율)와 비판자(정밀도)를 나누는 구조와 맞다. 마지막 판정자(메인, 사람)가 필요하다 |
| Anthropic 공식 `code-review` 플러그인 [code-review-plugin] | 독립 리뷰 에이전트 4개가 병렬로 찾는다. 그다음 **지적마다 검증 서브에이전트를 띄워** 확인되지 않은 것을 걸러 낸다. 오탐 목록(기존 문제, 린터가 잡을 것, 사소한 지적)은 지적하지 않는다 | 반박 단계에 해당하는 실무 사례다. 효과 수치는 공개하지 않았다 |
| Smit 외, "Should we be going MAD?"(arXiv 2311.17371, ICML 2024) [mad] | 다중 에이전트 토론은 자기 일관성이나 앙상블보다 안정적으로 낫지 않다. 동의 정도 같은 설정에 민감하다 | 비판자는 "토론"하지 말고 코드와 실행으로 **검증**하게 한다 |
| Huang 외, "Large Language Models Cannot Self-Correct Reasoning Yet"(arXiv 2310.01798, ICLR 2024) [selfcorrect] | 외부 피드백 없이 스스로 고치면 성능이 오히려 떨어질 수 있다 | 리뷰·반박 모두 테스트·명령 출력 같은 외부 근거를 요구한다 |

**리뷰어가 구현자 설명을 볼 때와 안 볼 때의 직접 비교**: 이 조건만 다룬 통제 실험은 찾지 못했다(**미확인**). 가장 가까운 근거는 위의 PR 메타데이터 편향 연구와 판정 편향 연구다. 둘 다 "작성자 쪽 서술이 판정을 움직인다"를 보인다.

**반박 단계가 오탐을 줄이는 정도**: 정량 연구는 찾지 못했다(**미확인**). 공식 플러그인이 검증 단계를 쓰고, CriticGPT가 정밀도와 재현율의 맞바꿈을 보인 것이 정성적 근거다. → 이 저장소의 `docs/reviews/*.md` 기록에서 직접 측정하라고 권한다. 측정 항목: 기각률, 메인이 기각을 뒤집은 비율, 나중에 버그로 확인된 기각 건수.

### Q5. 리뷰어 프롬프트 설계

근거에서 나온 규칙을 정리했다. 초안은 3.3에 있다.
1. **발견 유도**: 목표를 "승인"이 아니라 "깨뜨리기"로 둔다. 확인 목록(완료 기준, 경계값, 순서, 동시성, 부분 실패, SEC/ORD/ACC, 취지 위반, 구현을 베낀 테스트)을 준다. 단, "모든 지적에 재현 명령이나 반례"를 요구한다. Anthropic 평가자가 피상적으로 테스트한 실패 유형이 있었기 때문이다. [anthropic-harness]
2. **근거 없는 지적 줄이기**: 지적마다 파일과 줄, 재현 명령과 실제 출력, 위반한 규칙의 문서 위치를 인용하게 한다. 재현하지 못하면 "추정"으로 표시한다(workflow.md와 같다). 형식·린트·취향은 지적하지 않는다. [best-practices], [code-review-plugin]
3. **등급 기준을 결과로 정의한다**:
   - 차단 = 완료 기준 미충족, SEC/ORD/ACC 위반, 데이터 손상, 검사 우회
   - 중요 = 특정 입력·순서·동시성에서 틀린 결과
   - 사소 = 동작은 맞지만 유지보수 위험

   리뷰어는 등급을 매기기만 하고 "괜찮다"고 판정하지 않는다. 판정을 비판자와 메인에게 넘겨서 "스스로 설득해 승인"하는 실패를 구조로 막는다. [anthropic-harness]
4. **비판자 형식**: 지적마다 반박을 먼저 시도한다. 판정 근거는 명령과 출력, 또는 파일과 줄로만 낸다. "기각"에는 반례 실행 결과를 붙인다. 비판자도 놓친 결함을 추가 지적으로 낼 수 있다.
5. **입력 통제**: 리뷰어에게는 브리프, 기준 커밋 SHA, 대상 커밋 SHA만 준다. 차이는 리뷰어가 스스로 `git diff`로 만든다. 메인이 차이를 요약해 주면 그 요약이 다시 편향이 된다.

### Q6. 에이전트의 흔한 편법과 자동 탐지

ImpossibleBench(arXiv 2510.20270)는 명세와 모순되는 테스트를 주고 "통과하면 곧 편법"인 조건에서 측정했다. 결과는 다음과 같다. [impossiblebench]
- GPT-5는 Conflicting-SWEbench에서 54%를 편법으로 통과했다.
- Claude 계열은 편법의 79% 넘게가 **테스트 수정**이었다. OpenAI 계열은 연산자 재정의, 상태 기록, 테스트별 특수 처리도 고르게 썼다.
- 테스트를 읽기 전용으로 두자 정상 성능은 유지됐다. Opus 4.1에 특히 효과가 있었다. 하지만 특수 처리와 연산자 재정의는 막지 못했다.
- "사람에게 넘기기" 출구를 주자 GPT-5의 편법이 54%에서 9%로 줄었다. Claude Opus 4.1에서는 효과가 작았다.

METR도 최신 모델이 테스트나 채점 코드를 바꾸는 보상 해킹을 한다고 보고했다. 탐지는 LLM 감시자와 점수 휴리스틱이 서로 다른 사례를 잡았다. [metr-rh]

| 편법 | 자동 탐지 방법 | 어디서 |
|---|---|---|
| 기존 테스트 삭제·약화(단언 제거, 기대값을 구현 출력으로 바꿈) | 기준 커밋 대비 **기존 테스트 함수의 삭제·변경 목록**을 `xtask test-diff`로 뽑는다. 있으면 차단하고 사람 승인을 요구한다. 새 테스트 추가는 허용한다. 테스트 수 감소(`cargo nextest list`, `vitest list` 비교)도 검사한다 | SubagentStop 훅, CI |
| 테스트 건너뛰기(`#[ignore]`, `.skip`, `.only`) | harness.md 2장 규칙(린트, `xtask`) | PostToolUse, CI |
| 검사 끄기(`#[allow]`, `eslint-disable`, `@ts-ignore`, 설정 파일 변경) | 속성·주석 검사 + 보호 경로 | PreToolUse, CI |
| 예외·오류 삼키기 | Rust: `let _ =` 금지, `clippy::let_underscore_must_use`, `map_err(\|_\| ...)` 패턴 검사(R-09). TS: try/catch 금지, `no-floating-promises`, 무시된 Result 금지(R-08) | 린트 |
| 구현을 베낀 테스트, 테스트별 특수 처리 | 변이 테스트(`cargo-mutants`): 베낀 테스트는 변이를 죽이지 못한다. 속성 테스트. 테스트 입력 리터럴을 구현에서 다시 쓰는지 찾는 `xtask` 검사(후보, **효과 미확인**). 리뷰어 확인 목록 | `just check-full`, 리뷰 |
| 연산자 재정의·상태 기록(테스트 감지 분기) | 구현 코드에서 `cfg(test)` 분기, 테스트 이름이나 경로 문자열 참조 금지(`xtask`). 변이 테스트 | CI |
| 훅 끄기(`--no-verify`, `-n`, `LEFTHOOK=0`, `LEFTHOOK_CONFIG`, `lefthook-local.yml`, `core.hooksPath`, `GIT_CONFIG_PARAMETERS`) | PreToolUse(Bash) 정규식(최선 노력). `.git/hooks`와 `.git/config`는 샌드박스 기본 보호 경로다. `lefthook-local.yml`은 denyWrite. 마지막은 CI가 같은 명령을 다시 돌린다 | PreToolUse, 샌드박스, CI |
| 하네스 상태 위조(통과 기록 파일 조작) | 상태 파일을 샌드박스가 쓸 수 없는 곳(`~/.local/state/...`)에 둔다 | 3.2 초안 |
| 범위 밖 수정 | 브리프의 "손댈 범위"와 실제 변경 파일 비교(메인 평가 단계). 보호 경로는 훅·CI | 메인, CI |

lefthook 관련 근거: `LEFTHOOK=0`과 `LEFTHOOK_CONFIG`가 동작을 바꾸고, `lefthook-local.yml`이 설정을 덮어쓴다는 것은 lefthook 공식 문서의 `LEFTHOOK`, `LEFTHOOK_CONFIG`, `rc` 항목에서 확인했다. [lefthook-docs]

### Q7. 로컬 훅과 CI에서 같은 명령 돌리기

- **just**(1.59.0): 명령 정의를 한곳(`justfile`)에 둔다. 에이전트 훅, git 훅, CI가 모두 `just check`와 `just check-full`만 부른다.
- **lefthook**(v2.2.2): git 훅 관리자다.
  - `assert_lefthook_installed: true`로 훅이 빠진 상태를 실패로 만든다. [lefthook-docs]
  - `ai:` 절(베타)로 `.claude/settings.json` 훅을 생성하는 기능이 있다. 하지만 이 하네스의 훅은 역할별 판단이 필요하고, 생성물이 보호 파일을 다시 쓰게 된다. 그래서 **쓰지 않는다**.
- 대안 비교:

| 도구 | 장점 | 단점 |
|---|---|---|
| lefthook | 단일 바이너리. 병렬 작업. `assert_lefthook_installed`. 설정 한 파일 | `LEFTHOOK=0`·`lefthook-local.yml` 같은 끄는 수단이 많다(막아야 함) |
| pre-commit(Python) | 생태계가 크다 | Python 의존. 이 저장소의 도구 체계와 겹친다 |
| husky | Node 생태계 | Rust 쪽 커밋에도 Node가 필요하다 |
| `.githooks/` + `core.hooksPath` | 의존성이 없다 | `core.hooksPath`를 바꾸는 것이 곧 우회 수단이 된다 |

추천은 **just + lefthook**이다. CI는 lefthook을 거치지 않고 `just check-full`을 직접 부른다. 로컬 훅은 편의일 뿐이고, 강제는 CI다(harness.md 1장과 같다).

### Q8. 작업 브리프 형식과 작업 크기

공식 권고는 다음과 같다. [best-practices]
- 구체적인 프롬프트에 대상 파일, 증상, "고쳐진 상태"를 앞에 적는다.
- **통과·실패로 돌아오는 검사**를 준다.
- 증거(실행한 명령과 출력)를 요구한다.
- 스펙 대비 리뷰를 할 때는 "모든 요구사항이 구현됐는지, 나열한 경계 사례에 테스트가 있는지"를 보게 한다.

Anthropic 하네스 글은 작업 전에 생성자와 평가자가 "완료가 무엇인지" 합의하는 스프린트 계약을 썼다. 에이전트끼리의 인계는 파일로 했다. [anthropic-harness]

작업 크기: METR은 사람 기준 작업 길이가 길수록 에이전트 성공률이 떨어진다고 측정했다. 80% 성공 시간 지평은 50% 지평의 약 1/5이다(원 논문). [metr-horizon] 줄 수나 파일 수 같은 구체적 상한의 1차 근거는 찾지 못했다(**미확인**). 그래서 다음 기준을 **판단으로** 권한다.
- 완료 기준 3~5개, 크레이트나 기능 폴더 하나에 들어가는 크기
- 리뷰 3회 규칙(workflow.md 2.6)에 걸리면 쪼갠다

브리프 초안(workflow.md 2.1에 더할 항목에 표시):

```markdown
# 브리프 <작업 ID> <제목>
- 기준 커밋: <SHA>                       ← 추가: 리뷰어 작업 사본의 기준
- 완료 기준 (tasks.md 그대로, 각 기준은 통과·실패로 판정 가능해야 함)
  1. ...
- 관련 요구사항: SEC-.., ORD-..   관련 구조 규칙: architecture.md x.y
- 손댈 범위: crates/<name>/src/**, crates/<name>/tests/**
- 손대면 안 되는 범위: (보호 경로 외 추가로)
- 정해진 결정: ADR-NNNN
- 검사 명령: just check                  ← 추가: 증거로 낼 명령
- 막혔을 때: 보고서 첫 줄 "막힘:"으로 멈춘다   ← 추가: ImpossibleBench의 출구 효과
```

### Q9. 보호 파일 변경의 사람 승인을 흉내 내지 못하게 하는 방법

구성:
1. **서명 키를 에이전트가 쓸 수 없게 한다**:
   - SSH 서명 키를 Secure Enclave에 두고 사용할 때마다 Touch ID를 요구한다. Secretive가 이 기능을 제공한다. 키를 내보낼 수 없고 접근 통지도 준다. [secretive]
   - 저장소에는 `gpg.format=ssh`, `user.signingkey=<Secretive 공개키>`, **`commit.gpgsign=false`**를 둔다.
   - 사용자가 보호 파일을 바꿀 때만 `git commit -S`로 서명한다. `gpgsign=true`로 두면 에이전트 커밋마다 Touch ID 창이 떠서, 습관적으로 승인할 위험이 생긴다.
2. **검증은 기준 브랜치의 신뢰 자료로 한다**:
   - git은 `gpg.ssh.allowedSignersFile`에 있는 키만 신뢰한다. 없으면 `git verify-commit`이 실패한다. [git-config], [ssh-keygen]
   - CI는 허용 서명자 파일과 보호 경로 목록을 **PR 머리가 아니라 기준 브랜치(`origin/main`)에서** 읽는다. 에이전트가 같은 PR에서 서명자 목록을 바꿔 통과하는 길을 막기 위해서다.
3. **검사 워크플로가 PR 쪽 수정으로 무력화되지 않게 한다**: `pull_request` 워크플로는 PR 쪽 워크플로 파일을 쓰는 것으로 보인다(문서에 직접적인 문장은 없음, **추론**). `pull_request_target`은 "기준 저장소 기본 브랜치의 맥락에서" 돈다. [gh-events] 보호 경로 서명 검사는 PR 코드를 실행하지 않고 git 명령만 쓴다. 그래서 `pull_request_target`에 둬도 공식 경고(신뢰할 수 없는 코드 실행)에 해당하지 않는다.
4. **병합은 사람이 한다**: GitHub 규칙 세트로 `main`에 "PR 필수"와 "상태 검사 필수"를 건다. [gh-rulesets] 에이전트에게 주는 토큰에는 저장소 관리 권한(규칙 세트 수정)을 주지 않는다. 개인 비공개 저장소에서 "파일 경로 제한" 규칙을 쓸 수 있는지는 문서에 나오지 않는다(**미확인**). 규칙 세트의 "서명 커밋 필수"는 에이전트 커밋까지 서명을 요구하므로 쓰지 않는다.
5. **로컬 보조 장치(선택)**: macOS 관리 설정 파일 `/Library/Application Support/ClaudeCode/managed-settings.json`은 root 소유라 에이전트가 고칠 수 없다. [managed-settings] 여기에 `sandbox.enabled`, `allowUnsandboxedCommands: false`, `failIfUnavailable`, `permissions.disableBypassPermissionsMode: "disable"`을 두면 샌드박스가 "관리자 필수"가 된다. 그러면 저장소 설정으로 샌드박스를 느슨하게 할 수 없다. [sandboxing] 단점: 이 PC의 모든 프로젝트에 적용된다.

### Q10. 서브에이전트 종료 훅의 규약

- **차단**: `{"decision":"block","reason":"..."}`(종료 코드 0)이나 종료 코드 2 + stderr로 막는다. `reason`은 서브에이전트의 다음 지시가 된다. [hooks]
- **횟수 제한**: `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`의 기본은 8이다. `Stop`과 `SubagentStop` 모두에 적용된다. `0`이면 상한이 꺼진다. [env-vars] 연속 횟수는 Claude가 도구를 부를 때마다 0으로 돌아간다(`Stop` 문서). [hooks]
  - 실측(실험 8): 항상 막는 훅에 도구를 쓰지 않는 서브에이전트를 붙였다. 훅은 9번 불렸고, 9번째 차단은 무시된 채 종료됐다. 상한 8이 서브에이전트에도 적용됨을 확인했다.
  - 서브에이전트가 매번 도구를 부르며 고치려 들면, 횟수가 계속 0으로 돌아가 사실상 끝없이 돌 수 있다. 그래서 harness.md의 "3회"는 **훅이 `agent_id`별로 직접 세야 한다**. `stop_hook_active`는 "이미 한 번 막혔다"만 알려 준다.
- **실패 상태로 종료 허용**: 그냥 종료 코드 0을 내면 된다. 실측(실험 7): `subagent_stop.py`가 `./check.sh` 실패로 3번 막았다. 3번째 차단 문구에 "'막힘:'으로 시작하는 보고를 쓰고 끝내라"를 넣었다. 4번째에는 `systemMessage`만 남기고 종료를 허용했다. 서브에이전트의 마지막 보고는 "막힘: `./check.sh`가 3회 연속 실패했고…"로 시작했다.
- **주의**:
  - `SubagentStop` 훅이 타임아웃되거나 실패하면 차단되지 않는다. `onFailure`도 효과가 없다. 즉 **"검사하지 못한 채 종료"가 가능하다.** 그래서 메인 쪽 `PostToolUse(Agent)` 훅이 상태 파일을 읽어 "통과/막힘/미확인"을 메인 맥락에 붙여야 한다(3.2 초안).
  - 백그라운드 서브에이전트는 `PostToolUse(Agent)`가 실행 시작 시점에 불린다(`status: "async_launched"`). 그래서 `CLAUDE_CODE_DISABLE_BACKGROUND_TASKS=1`로 서브에이전트를 앞쪽 실행으로 고정한다. 이렇게 하면 메인이 결과를 기다린다. [sub-agents]
  - `continue: false`를 `SubagentStop`에서 내면 서브에이전트만 멈추는지 메인까지 멈추는지 문서에 나오지 않는다(**미확인**). 쓰지 않는다.
  - Claude Code 내부 에이전트(프롬프트 제안 등)도 `SubagentStop`을 일으킨다. 이때 `agent_type`이 빈 문자열이다. matcher에 이름을 적으면 이런 이벤트에는 맞지 않는다. [hooks]

### Q11. 리뷰 서브에이전트를 격리된 작업 사본에서 돌리고 변경 버리기

- `isolation: worktree`를 쓰면 `.claude/worktrees/agent-<id>`에 작업 사본과 `worktree-agent-<id>` 브랜치가 생긴다(실측). 변경이 없으면 끝날 때 자동으로 지운다. 변경이 있으면 `cleanupPeriodDays` 정리 때까지 남는다. [worktrees] 실측(실험 9, 11): 변경 없는 작업 사본은 끝난 뒤 사라졌다.
- **기준 커밋**: 기본(`"fresh"`)은 원격 기본 브랜치에서 갈라진다. 원격이 없으면 로컬 HEAD에서 갈라진다. 구현 브랜치를 리뷰하려면 둘 중 하나가 필요하다.
  - 리뷰어가 첫 단계로 `git checkout --detach <대상 SHA>`를 한다. 실측(실험 9): 작업 사본 안에서 성공했다.
  - 또는 `worktree.baseRef: "head"`를 쓰고 메인이 구현 브랜치를 체크아웃한 상태여야 한다.

  이 하네스에는 앞의 방법을 권한다. 브리프의 SHA로 결정적이다.
- **원본에 남지 않게 하는 문제**: 작업 사본 격리 검사는 다음 세 가지만 막는다. [worktrees]
  - 원본을 겨냥한 Edit/Write
  - 원본을 작업 디렉터리로 쓰는 명령
  - git 리다이렉트

  셸이 원본 경로에 쓰는 것은 추적하지 않는다. 문서에도 그렇게 적혀 있다.
  - 실측(실험 10): 리뷰어가 `python3 -c open("<exp>/repo/src/escape.txt","w")`로 원본에 파일을 만들었다(성공). 샌드박스 쓰기 영역이 세션의 작업 디렉터리(원본)이고, 작업 사본이 그 안(`.claude/worktrees/`)에 있기 때문이다.
  - 대응(실측, 실험 11): `SubagentStart`(matcher `adversarial-reviewer|review-critic`) 훅이 원본의 `git status --porcelain -uall`과 HEAD를 저장한다. `SubagentStop` 훅이 다시 비교하고, 다르면 `decision: "block"`으로 "네가 만든 변경을 되돌린 뒤 끝내라"를 돌려준다. 리뷰어는 `src/escape.txt`를 지우고 끝냈다. 원본 상태는 깨끗해졌다.
  - 더 강한 대안: 리뷰어를 Agent 도구가 아니라 저장소 밖 작업 사본(`git worktree add ~/…/review-<id>`)에서 `claude -p --agent adversarial-reviewer`로 따로 띄운다. 그러면 샌드박스 쓰기 영역이 그 사본뿐이다. 다만 workflow.md의 "메인이 서브에이전트를 부른다" 구조가 바뀌고, 중첩 `claude` 실행을 허용하는 `just` 명령이 필요하다. 현재 단계에서는 감지 훅으로 충분하다고 판단한다.
- **넓은 거부 규칙의 부작용**: `Edit(/.claude/**)`를 거부 규칙에 넣으면 그 규칙이 샌드박스 denyWrite에 합쳐진다. 그 결과 `.claude/worktrees/` 아래 리뷰어 작업 사본에서 `echo > src/repro.txt`도 `operation not permitted`로 막혔다(실험 9). `.claude/settings.json`, `.claude/agents/**`, `.claude/hooks/**`처럼 **좁게** 적어야 한다(실험 10에서 해결 확인).
- `.gitignore`에 `.claude/worktrees/`를 넣는다. 그래야 원본 상태 비교에 잡음이 생기지 않는다.

### 실행 검증 요약

모든 실험은 `<exp>/repo`(scratchpad 안 임시 git 저장소, 원격 없음)에서 haiku 모델로 돌렸다. 로그는 `<exp>/logs/`에 있다.

| # | 설정 | 시도 | 결과 |
|---|---|---|---|
| 1 | 프로젝트 설정: `Edit(/justfile)` 등 거부, 샌드박스(엄격), `denyWrite ./justfile ./docs/harness.md`, 훅 | Write src/a.txt / Write justfile / Write docs/harness.md / `echo >> justfile` / `python3 open('justfile')` / `python3 open('docs/harness.md')` / `python3 open('.claude/settings.json')` / `git commit --no-verify` / `cp src/a.txt Cargo.toml` | 허용 / 차단("denied by your permission settings") / 차단 / 차단(`operation not permitted`) / 차단(`PermissionError: [Errno 1]`) / 차단 / 차단 / 차단(guard_bash) / **허용**(목록에 없는 경로) |
| 2 | `--settings`로 `denyWrite ["."]` + `allowWrite ["./src","./.git"]` | src 쓰기, 루트 새 파일, `git add` | 모두 차단(`.git/index.lock` 포함). allow가 deny를 다시 열지 못함 |
| 3·4 | `--settings`로 상대 경로 `./*.ts`, `./crates/*/Cargo.toml`, `./.github` | 각 쓰기 | 모두 **허용**(상대 경로가 프로젝트 기준으로 풀리지 않음) |
| 5 | `--settings`로 같은 규칙을 절대 경로로 | 각 쓰기 | 모두 차단. `crates/core/src/ok.rs`만 허용 |
| 6 | 프로젝트 설정에 같은 규칙을 `./` 상대 경로로 | 각 쓰기 | 모두 차단(와일드카드, 새 디렉터리, 새 파일 포함) |
| 7 | `SubagentStop`(implementer) 3회 차단 훅, 검사 항상 실패 | implementer 실행 | 훅 4번 호출(`stop_hook_active` false→true→true→true), 3번 차단 후 허용. 마지막 보고가 "막힘:"으로 시작 |
| 8 | 같은 훅, 상한 100, 도구 미사용 서브에이전트 | implementer 실행 | 훅 9번 호출 후 종료. 내장 상한 8 확인 |
| 9 | 리뷰어 `isolation: worktree`, 거부 규칙 `Edit(/.claude/**)` | pwd / 작업 사본 안 쓰기 / `git checkout --detach` | `.claude/worktrees/agent-…`, 브랜치 `worktree-agent-…` / **차단** / 성공 |
| 10 | 거부 규칙을 좁힘 | 작업 사본 안 쓰기 / `../../../justfile` / 원본 절대 경로에 python 쓰기 | 허용 / 차단 / **허용(원본 오염)** |
| 11 | 10 + `isolation_guard.py`(SubagentStart/Stop) | 원본 절대 경로에 python 쓰기 | 종료 차단 → 리뷰어가 스스로 삭제 → 원본 깨끗 |
| 12 | `guard_agent.py`(PreToolUse Agent) | `model` 인자 / general-purpose / 포크 | 모두 차단 |

`--setting-sources project`만 주어도 구독 로그인으로 실행됐다(모든 실험). 대화형 세션에서 같은 옵션을 쓸 때의 동작은 시험하지 않았다(**미확인**).

## 3. 선택지 비교표와 추천

### 3.1 보호 경로 강제 구성

| 구성 | 파일 도구 | 셸 직접 쓰기 | 스크립트 간접 쓰기 | 허용 목록 정확도 | 비용 |
|---|---|---|---|---|---|
| A. 거부 규칙만 | 막음 | 일부(리다이렉트, 알려진 명령) | 못 막음 | 거부 목록 | 낮음 |
| B. A + PreToolUse 훅(역할별 허용 목록) | 막음(허용 목록) | 정규식 최선 노력 | 못 막음 | 파일 도구는 정확 | 중간 |
| **C. B + 엄격 샌드박스 denyWrite + CI 서명·허용 목록 검사** | 막음 | 막음 | 막음(목록에 있는 경로) | 로컬은 근사, CI는 정확 | 중간 |
| D. C + 별도 사용자·컨테이너 | 막음 | 막음 | 막음 | 정확 | 높음 |

**추천: C.** 실측으로 셸 우회까지 막히는 것을 확인했다. 남는 틈은 "목록에 없는 새 경로를 셸로 만드는 것"뿐이고, 이것은 CI가 잡는다.

**추천이 틀렸다고 판명될 조건**:
- 샌드박스 때문에 `cargo`, `pnpm`, `tauri` 빌드나 테스트가 자주 깨져 `excludedCommands`가 늘어난다. 제외 명령은 샌드박스 밖에서 돈다.
- Claude Code가 샌드박스의 `Edit` 규칙 병합 동작을 바꾼다.
- CI 검사가 아직 없는 기간에 보호 경로 변경이 실제로 새어 나간다.

이런 경우 D(별도 사용자나 VM)로 올린다.

### 3.2 `.claude/settings.json` 초안

경로는 실제 저장소 배치(architecture.md)에 맞춰 다듬어야 한다. `permissions.deny`의 `Edit(...)`는 샌드박스 denyWrite에도 자동으로 합쳐진다. 그래서 같은 경로를 `sandbox.filesystem.denyWrite`에 다시 적을 필요는 없다. `denyWrite`에는 **Edit 도구로는 허용하되 셸로는 막을 경로**(의존성 목록 파일)와 패턴만 둔다.

```json
{
  "env": {
    "CLAUDE_CODE_FORK_SUBAGENT": "0",
    "CLAUDE_CODE_DISABLE_BACKGROUND_TASKS": "1",
    "CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH": "1",
    "CLAUDE_CODE_STOP_HOOK_BLOCK_CAP": "8",
    "STOCK_COPILOT_HARNESS_STATE": "~/.local/state/stock-copilot-harness"
  },
  "permissions": {
    "disableBypassPermissionsMode": "disable",
    "deny": [
      "Agent(fork)",
      "SendMessage",
      "Read(./.env*)",
      "Edit(/.claude/settings.json)",
      "Edit(/.claude/settings.local.json)",
      "Edit(/.claude/agents/**)",
      "Edit(/.claude/hooks/**)",
      "Edit(/.claude/skills/**)",
      "Edit(/.claude/commands/**)",
      "Edit(/.mcp.json)",
      "Edit(/AGENTS.md)",
      "Edit(/CLAUDE.md)",
      "Edit(/docs/harness.md)",
      "Edit(/docs/workflow.md)",
      "Edit(/justfile)",
      "Edit(/lefthook.yml)",
      "Edit(/lefthook-local.yml)",
      "Edit(/.lefthook-local.yml)",
      "Edit(/.github/**)",
      "Edit(/.cargo/**)",
      "Edit(/crates/xtask/**)",
      "Edit(/clippy.toml)",
      "Edit(/rustfmt.toml)",
      "Edit(/deny.toml)",
      "Edit(/rust-toolchain.toml)",
      "Edit(/harness/**)",
      "Edit(/**/eslint.config.*)",
      "Edit(/**/tsconfig*.json)",
      "Edit(/**/vitest.config.*)",
      "Edit(/**/knip.*)",
      "Edit(/**/.gitignore)",
      "Edit(/.worktreeinclude)"
    ]
  },
  "sandbox": {
    "enabled": true,
    "failIfUnavailable": true,
    "allowUnsandboxedCommands": false,
    "filesystem": {
      "denyWrite": [
        "./Cargo.toml",
        "./crates/*/Cargo.toml",
        "./package.json",
        "./apps/desktop/package.json",
        "./*.toml",
        "./*.yml",
        "./*.yaml",
        "./*.json",
        "./*.js",
        "./*.ts",
        "./*.mjs",
        "./*.cjs"
      ]
    }
  },
  "worktree": { "baseRef": "fresh" },
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Edit|Write|NotebookEdit",
        "hooks": [{ "type": "command", "command": "python3", "args": ["-I", "${CLAUDE_PROJECT_DIR}/.claude/hooks/guard_edit.py"], "onFailure": "block", "timeout": 10 }]
      },
      {
        "matcher": "Bash",
        "hooks": [{ "type": "command", "command": "python3", "args": ["-I", "${CLAUDE_PROJECT_DIR}/.claude/hooks/guard_bash.py"], "onFailure": "block", "timeout": 10 }]
      },
      {
        "matcher": "Agent",
        "hooks": [{ "type": "command", "command": "python3", "args": ["-I", "${CLAUDE_PROJECT_DIR}/.claude/hooks/guard_agent.py"], "onFailure": "block", "timeout": 10 }]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Edit|Write",
        "hooks": [{ "type": "command", "command": "python3", "args": ["-I", "${CLAUDE_PROJECT_DIR}/.claude/hooks/post_edit_check.py"], "timeout": 120 }]
      },
      {
        "matcher": "Agent",
        "hooks": [{ "type": "command", "command": "python3", "args": ["-I", "${CLAUDE_PROJECT_DIR}/.claude/hooks/agent_result.py"], "timeout": 10 }]
      }
    ],
    "SubagentStart": [
      {
        "matcher": "adversarial-reviewer|review-critic",
        "hooks": [{ "type": "command", "command": "python3", "args": ["-I", "${CLAUDE_PROJECT_DIR}/.claude/hooks/isolation_guard.py", "start"], "timeout": 30 }]
      }
    ],
    "SubagentStop": [
      {
        "matcher": "implementer",
        "hooks": [{ "type": "command", "command": "python3", "args": ["-I", "${CLAUDE_PROJECT_DIR}/.claude/hooks/subagent_stop.py"], "timeout": 1500 }]
      },
      {
        "matcher": "adversarial-reviewer|review-critic",
        "hooks": [{ "type": "command", "command": "python3", "args": ["-I", "${CLAUDE_PROJECT_DIR}/.claude/hooks/isolation_guard.py", "stop"], "timeout": 30 }]
      }
    ],
    "ConfigChange": [
      {
        "matcher": "project_settings|local_settings|skills",
        "hooks": [{ "type": "command", "command": "sh", "args": ["-c", "echo '세션 중 설정 변경은 적용하지 않는다. 세션을 다시 시작하라.' >&2; exit 2"] }]
      }
    ]
  }
}
```

설정 메모:
- `SendMessage` 거부: 끝난 서브에이전트를 이어 쓰는 것을 막아 "수정은 새 implementer"를 강제한다. 메인이 사용자 세션 간 메시지를 써야 한다면 이 항목을 뺀다.
- `CLAUDE_CODE_DISABLE_BACKGROUND_TASKS=1`: 서브에이전트를 앞쪽 실행으로 고정한다. 그래야 `PostToolUse(Agent)`가 결과를 받는다. 백그라운드 셸 명령도 꺼진다. [sub-agents]
- `CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH=1`: 서브에이전트가 서브에이전트를 띄우지 못한다. 기존 정의 파일의 "서브에이전트를 띄우지 않는다"를 강제한다.
- `.gitignore`도 보호한다. 다만 `.claude/worktrees/` 항목은 처음에 사람이 넣어 둔다.
- `sandbox.filesystem.denyWrite`의 `./*.json` 같은 루트 패턴은 루트에 쓰여야 하는 생성물이 생기면 조정한다. `Cargo.lock`, `pnpm-lock.yaml`, `target/`, `node_modules/`는 막지 않는다.
- 사용자 전역 플러그인과 훅(이 PC에는 security-guidance 등이 설치돼 있다)도 이 저장소에서 같이 돈다. 섞이지 않게 하려면 개발 세션을 `claude --setting-sources project,local`로 시작한다. `-p`에서 구독 로그인과 함께 동작함을 확인했다. 대화형에서는 **미확인**이다.

훅 스크립트 초안(`.claude/hooks/`, 표준 라이브러리만 사용, `python3 -I`로 실행). 실험에서 검증한 것을 하네스용으로 정리했다.

```python
# .claude/hooks/common.py 를 두지 않는다: -I 실행에서는 스크립트 디렉터리가 sys.path에 들어가지 않으므로 파일마다 독립적으로 쓴다.
# .claude/hooks/guard_edit.py
import fnmatch, json, os, sys

IMPLEMENTER = ["crates/*/src/**", "crates/*/tests/**", "apps/desktop/src/**",
               "apps/desktop/src-tauri/src/**", "prompts/**", "fixtures/**", "docs/**",
               "Cargo.toml", "crates/*/Cargo.toml", "apps/desktop/package.json"]
MAIN = ["docs/**"]
NEVER = ["docs/harness.md", "docs/workflow.md", "crates/xtask/**"]
ROLE = {None: MAIN, "implementer": IMPLEMENTER}

def deny(reason):
    print(json.dumps({"hookSpecificOutput": {"hookEventName": "PreToolUse",
          "permissionDecision": "deny", "permissionDecisionReason": reason}}, ensure_ascii=False))
    sys.exit(0)

data = json.load(sys.stdin)
role = data.get("agent_type")
allowed = ROLE.get(role)
if allowed is None:
    deny(f"{role}는 파일을 편집하지 않는다.")
ti = data.get("tool_input") or {}
target = ti.get("file_path") or ti.get("notebook_path")
if not target:
    deny("경로 없는 편집은 허용하지 않는다.")
root = os.path.realpath(os.environ["CLAUDE_PROJECT_DIR"])
real = os.path.realpath(target)
if not real.startswith(root + os.sep):
    deny(f"작업 사본 밖 경로다: {target}")
rel = os.path.relpath(real, root)
if rel.startswith(".claude/worktrees/") or any(fnmatch.fnmatchcase(rel, p) for p in NEVER) \
        or not any(fnmatch.fnmatchcase(rel, p) for p in allowed):
    deny(f"보호 경로다(docs/harness.md 7.1): {rel}. 우회하지 말고 멈춘 뒤 이유를 보고하라.")
```

```python
# .claude/hooks/guard_bash.py  (최선 노력. 최종 강제는 샌드박스와 CI)
import json, re, sys

RULES = [
    (r"\bgit\b[^|;&]*\bcommit\b[^|;&]*(--no-verify|\s-[a-zA-Z]*n\b)", "git commit --no-verify/-n 금지"),
    (r"\bgit\b[^|;&]*\bpush\b[^|;&]*(--force|--force-with-lease|--no-verify|\s-[a-zA-Z]*f\b|\s\+\S)", "강제 푸시·검증 건너뛰기 금지"),
    (r"\bgit\b\s+(-c\b|--config-env\b|config\b)|--git-dir|--work-tree|core\.hooksPath", "git 설정 변경 금지"),
    (r"\b(LEFTHOOK|LEFTHOOK_CONFIG|LEFTHOOK_EXCLUDE|HUSKY|SKIP|GIT_CONFIG_PARAMETERS|GIT_CONFIG_COUNT|GIT_CONFIG_GLOBAL|GIT_CONFIG_SYSTEM)=", "훅·git 설정을 바꾸는 환경 변수 금지"),
    (r"\blefthook\s+(uninstall|install|run\b.*--no)", "git 훅 상태 변경 금지"),
    (r"\bgit\b[^|;&]*\b(update-ref|replace|filter-branch|filter-repo|rebase|reset\s+--hard)\b", "이력 조작 금지"),
    (r"\b(chmod|chflags|xattr)\b", "파일 권한·속성 변경 금지"),
    (r"(^|[;&|(]\s*)claude(\s|$)", "중첩 claude 실행 금지"),
]
SUBAGENT_ONLY = [(r"\bgit\b[^|;&]*\bpush\b", "서브에이전트는 푸시하지 않는다")]

data = json.load(sys.stdin)
ti = data.get("tool_input") or {}
cmd = ti.get("command") or ""
rules = RULES + (SUBAGENT_ONLY if data.get("agent_type") else [])
hits = [m for p, m in rules if re.search(p, cmd)]
if ti.get("dangerouslyDisableSandbox"):
    hits.append("샌드박스 밖 실행 금지")
if hits:
    print("; ".join(hits) + ". 하네스가 막으면 우회하지 말고 멈춘 뒤 보고하라.", file=sys.stderr)
    sys.exit(2)
```

```python
# .claude/hooks/guard_agent.py
import json, sys

ALLOWED = {"implementer", "adversarial-reviewer", "review-critic"}
data = json.load(sys.stdin)
ti = data.get("tool_input") or {}
problems = []
if data.get("agent_type"):
    problems.append("서브에이전트는 서브에이전트를 띄우지 않는다")
if ti.get("subagent_type") not in ALLOWED:
    problems.append(f"허용되지 않은 서브에이전트: {ti.get('subagent_type')}")
if ti.get("model"):
    problems.append("model 인자로 정의 파일의 모델을 덮어쓸 수 없다")
if "isolation" in ti:
    problems.append("isolation은 정의 파일에서만 정한다")
if ti.get("run_in_background"):
    problems.append("서브에이전트는 앞쪽 실행만 허용한다")
if problems:
    print(json.dumps({"hookSpecificOutput": {"hookEventName": "PreToolUse",
          "permissionDecision": "deny", "permissionDecisionReason": "; ".join(problems)}}, ensure_ascii=False))
```

```python
# .claude/hooks/post_edit_check.py
import json, os, subprocess, sys

data = json.load(sys.stdin)
path = (data.get("tool_input") or {}).get("file_path")
if not path:
    sys.exit(0)
r = subprocess.run(["just", "lint-file", path], cwd=os.environ["CLAUDE_PROJECT_DIR"],
                   capture_output=True, text=True, timeout=110)
if r.returncode != 0:
    print(json.dumps({"decision": "block",
                      "reason": f"just lint-file {path} 실패. 고쳐라:\n{(r.stdout + r.stderr)[-8000:]}"},
                     ensure_ascii=False))
```

```python
# .claude/hooks/subagent_stop.py  (implementer 전용)
import json, os, subprocess, sys

MAX_BLOCKS = 3
data = json.load(sys.stdin)
state_dir = os.path.expanduser(os.environ.get("STOCK_COPILOT_HARNESS_STATE", "~/.local/state/stock-copilot-harness"))
os.makedirs(state_dir, exist_ok=True)
path = os.path.join(state_dir, f"implementer-{data['agent_id']}.json")
try:
    blocks = json.load(open(path, encoding="utf-8"))["blocks"]
except (OSError, ValueError, KeyError):
    blocks = 0
r = subprocess.run(["just", "check"], cwd=os.environ["CLAUDE_PROJECT_DIR"], capture_output=True, text=True)
out = (r.stdout + r.stderr)[-6000:]

def save(status):
    head = subprocess.run(["git", "-C", os.environ["CLAUDE_PROJECT_DIR"], "rev-parse", "HEAD"],
                          capture_output=True, text=True).stdout.strip()
    json.dump({"blocks": blocks, "status": status, "head": head}, open(path, "w", encoding="utf-8"))

if r.returncode == 0:
    save("passed"); sys.exit(0)
if blocks >= MAX_BLOCKS:
    save("blocked")
    print(json.dumps({"systemMessage": f"implementer: just check 실패 상태로 종료({blocks}회 차단 후)"}, ensure_ascii=False))
    sys.exit(0)
blocks += 1
save("failing")
last = "\n\n마지막 차단이다. 고칠 수 없으면 보고서 첫 줄을 '막힘:'으로 시작하고 남은 실패와 원인을 쓴 뒤 끝내라." if blocks >= MAX_BLOCKS else ""
print(json.dumps({"decision": "block", "reason":
      f"just check 실패({blocks}/{MAX_BLOCKS}). 검사를 끄거나 테스트를 약화하지 말고 원인을 고쳐라.\n\n{out}{last}"},
      ensure_ascii=False))
```

```python
# .claude/hooks/agent_result.py  (메인 쪽 PostToolUse(Agent): 검사 상태를 메인 맥락에 붙인다)
import json, os, sys

data = json.load(sys.stdin)
tr = data.get("tool_response") or {}
ti = data.get("tool_input") or {}
if ti.get("subagent_type") != "implementer" or tr.get("status") != "completed":
    sys.exit(0)
state_dir = os.path.expanduser(os.environ.get("STOCK_COPILOT_HARNESS_STATE", "~/.local/state/stock-copilot-harness"))
try:
    state = json.load(open(os.path.join(state_dir, f"implementer-{tr.get('agentId')}.json"), encoding="utf-8"))
except (OSError, ValueError):
    state = {"status": "unchecked"}
print(json.dumps({"hookSpecificOutput": {"hookEventName": "PostToolUse", "additionalContext":
      f"하네스 기록: implementer {tr.get('agentId')} just check 상태={state.get('status')}, "
      f"HEAD={state.get('head')}, 모델={tr.get('resolvedModel')}. passed가 아니면 완료로 취급하지 않는다."}},
      ensure_ascii=False))
```

```python
# .claude/hooks/isolation_guard.py  (리뷰어·비판자가 원본 작업 사본을 바꾸면 종료를 막는다)
import json, os, subprocess, sys

def snapshot(root):
    st = subprocess.run(["git", "-C", root, "status", "--porcelain=v1", "-uall"], capture_output=True, text=True).stdout
    head = subprocess.run(["git", "-C", root, "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
    return {"head": head, "status": sorted(l for l in st.splitlines() if ".claude/worktrees/" not in l)}

mode = sys.argv[1]
data = json.load(sys.stdin)
root = os.environ["CLAUDE_PROJECT_DIR"]
state_dir = os.path.expanduser(os.environ.get("STOCK_COPILOT_HARNESS_STATE", "~/.local/state/stock-copilot-harness"))
os.makedirs(state_dir, exist_ok=True)
path = os.path.join(state_dir, f"isolation-{data.get('agent_id')}.json")
now = snapshot(root)
if mode == "start":
    json.dump(now, open(path, "w", encoding="utf-8")); sys.exit(0)
try:
    before = json.load(open(path, encoding="utf-8"))
except (OSError, ValueError):
    before = None
if before is not None and before != now:
    added = sorted(set(now["status"]) - set(before["status"]))
    print(json.dumps({"decision": "block",
          "reason": f"원본 작업 사본이 바뀌었다. 리뷰어는 원본을 바꾸지 않는다. 네가 만든 변경을 되돌린 뒤 끝내라.\n"
                    f"HEAD {before['head']} -> {now['head']}\n새 상태: {added}",
          "systemMessage": f"{data.get('agent_type')}가 원본을 바꿨다: {added}"}, ensure_ascii=False))
```

### 3.3 서브에이전트 정의 초안

세 정의 모두 `model: claude-opus-5-5`를 적는다. 실제 고정은 `guard_agent.py`가 `model` 인자를 거부하는 것과 `agent_result.py`가 `resolvedModel`을 기록하는 것으로 한다. 정의 파일 안에는 훅을 두지 않는다. 신뢰 대화상자와 `-p` 규칙 때문에 돌지 않을 수 있다.

```markdown
---
name: implementer
description: tasks.md의 작업 단위 하나를 구현한다. 메인 에이전트가 브리프와 함께 호출한다.
model: claude-opus-5-5
tools: Read, Edit, Write, Grep, Glob, Bash
permissionMode: acceptEdits
---

너는 이 저장소의 구현 담당이다. 브리프에 적힌 작업 하나만 구현한다.

시작하기 전에 AGENTS.md, docs/harness.md, docs/architecture.md와 브리프가 가리키는 문서를 읽는다.

규칙:
- 브리프의 "손댈 범위" 밖은 건드리지 않는다. 범위 밖 문제는 고치지 말고 보고서에 적는다.
- 완료 기준마다 그것을 검증하는 테스트를 먼저 쓴다. 테스트는 기준에서 나온 기대값을 쓰고, 구현의 출력을 복사하지 않는다.
- 기존 테스트를 지우거나 단언을 약하게 바꾸지 않는다. 기존 테스트가 틀렸다고 판단되면 바꾸지 말고 "막힘:"으로 보고한다.
- `just check`가 통과해야 끝난 것이다. 종료하려 할 때 하네스가 `just check`를 돌린다. 실패하면 실패 출력이 돌아온다.
- 하네스가 편집이나 명령을 거부하면 다른 방법으로 같은 일을 하려 하지 말고 멈춘다. 어떤 규칙이 무엇을 막는지와 시도한 대안을 보고한다.
- 구현이 불가능하거나 완료 기준이 서로 모순되면 억지로 통과시키지 말고 보고서 첫 줄을 "막힘:"으로 시작해 끝낸다. 이것은 실패가 아니라 정상 출구다.
- 커밋은 작게 하고 푸시하지 않는다. 서명 옵션을 쓰지 않는다.

보고 형식:
1. 첫 줄: "완료" 또는 "막힘: <한 줄 이유>"
2. 커밋 SHA 목록과 바꾼 파일 목록
3. 완료 기준별 충족 근거: 테스트 이름, 실행한 명령과 출력의 해당 줄
4. `just check` 마지막 실행의 요약 줄
5. 확신이 없는 부분과 범위 밖에서 발견한 문제
```

```markdown
---
name: adversarial-reviewer
description: 구현이 끝난 변경을 적대적으로 리뷰해 결함을 찾는다. 브리프와 커밋 SHA만 받고 구현 에이전트의 보고서는 받지 않는다.
model: claude-opus-5-5
tools: Read, Grep, Glob, Bash
isolation: worktree
permissionMode: default
---

너는 적대적 리뷰어다. 목표는 승인이 아니라 이 변경을 깨뜨리는 것이다. 판정(괜찮다, 큰 문제 아니다)은 네 일이 아니다. 찾은 것은 등급을 매겨 모두 보고한다.

너는 임시 작업 사본에서 돈다. 먼저 `git checkout --detach <브리프의 대상 SHA>`를 실행하고, `git diff <기준 SHA>...HEAD`로 변경을 직접 본다. 원본 작업 사본(작업 사본 밖 경로)에는 아무것도 쓰지 않는다. 재현용 파일은 이 작업 사본 안에만 만든다.

AGENTS.md, docs/harness.md, docs/architecture.md와 브리프가 가리키는 문서를 먼저 읽는다.

찾을 것:
1. 완료 기준을 실제로 충족하는가. 기준마다 그것을 검증하는 테스트가 있는가. 테스트가 기준이 아니라 구현의 출력을 기대값으로 쓰는가
2. 잘못된 입력, 경계값(0, 음수, 최댓값, 빈 값, 소수점 자릿수), 순서 바뀜, 동시 실행, 부분 실패, 재시도에서 깨지는가
3. docs/design.md의 SEC, ORD, ACC 위반. 특히 주문 경로, 금액 계산, 비밀 값
4. 구조 규칙을 문자로는 지키고 취지를 어긴 것: 검증 없는 newtype, 테스트만을 위한 분기, 테스트 이름에 반응하는 코드, 오류를 다른 값으로 바꿔 삼킨 것
5. 기존 테스트의 삭제·약화, 건너뛰기, 검사 끄기

방법:
- 의심되면 테스트를 써서 돌리거나 명령을 실행해 재현한다. 명령과 실제 출력을 그대로 붙인다.
- 재현하지 못한 지적은 "추정"이라고 표시하고, 재현하려면 무엇이 필요한지 쓴다.
- 형식, 이름, 취향, 린터가 잡는 것, 변경 전부터 있던 문제는 지적하지 않는다.

등급:
- 차단: 완료 기준 미충족, SEC/ORD/ACC 위반, 데이터 손상 가능, 검사 우회나 테스트 약화
- 중요: 특정 입력·순서·동시성에서 틀린 결과나 오류 누락
- 사소: 동작은 맞지만 다음 변경에서 결함을 부르기 쉬운 것

출력: 지적 목록. 지적마다
- 번호
- 등급
- 파일과 줄
- 재현 명령과 출력, 또는 반례 입력과 기대값·실제값
- 근거: 위반한 기준이나 문서의 위치를 인용
- "추정" 여부

지적이 없으면 확인한 항목과 각 항목에서 돌린 명령을 목록으로 쓴다.
```

```markdown
---
name: review-critic
description: 적대적 리뷰의 지적을 하나씩 반박해 보고 확정, 기각, 불확실로 판정한다.
model: claude-opus-5-5
tools: Read, Grep, Glob, Bash
isolation: worktree
permissionMode: default
---

너는 리뷰 비판자다. 리뷰어의 지적이 맞는지 실행으로 검증한다. 리뷰어 편도 구현자 편도 들지 않는다. 토론하지 말고 증거를 만든다.

너는 임시 작업 사본에서 돈다. 먼저 `git checkout --detach <브리프의 대상 SHA>`를 실행한다. 원본 작업 사본에는 아무것도 쓰지 않는다.

받는 것: 작업 브리프, 기준·대상 커밋 SHA, 리뷰 지적 목록.

지적마다:
1. 먼저 반박을 시도한다. 지적이 틀렸다면 그것을 보이는 명령이나 테스트를 만든다.
2. 반박이 안 되면 지적을 직접 재현한다.
3. 판정한다:
   - 확정: 직접 재현했거나 코드로 확인했다. 명령과 출력, 또는 파일과 줄을 붙인다
   - 기각: 지적이 틀렸다. 반례 실행 결과나 파일과 줄을 붙인다. 근거 없는 기각은 하지 않는다
   - 불확실: 판정에 무엇이 더 필요한지 쓴다
4. 등급이 결과와 맞지 않으면 고친 등급과 이유를 쓴다. 등급 기준은 리뷰어와 같다.

리뷰어가 놓친 결함을 발견하면 리뷰어와 같은 형식으로 추가 지적을 덧붙인다.

출력:
1. 지적 번호별 판정표: 번호, 판정, (고친) 등급, 근거 요약
2. 판정별 증거: 명령과 출력
3. 추가 지적 목록
```

### 3.4 just, lefthook, CI 초안

```just
# justfile (발췌)
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo run -q -p xtask -- arch-check lint-comments test-diff
    cargo nextest run --workspace
    pnpm -C apps/desktop run -s check

check-full: check
    git diff origin/main...HEAD > target/mutants.diff
    cargo mutants --workspace --in-diff target/mutants.diff
    pnpm -C apps/desktop run -s visual

lint-file path:
    cargo run -q -p xtask -- lint-file {{path}}
```

```yaml
# lefthook.yml
assert_lefthook_installed: true
pre-commit:
  jobs:
    - name: check
      run: just check
pre-push:
  jobs:
    - name: check-full
      run: just check-full
```

```yaml
# .github/workflows/ci.yml : 로컬과 같은 명령
name: ci
on: [push, pull_request]
jobs:
  check:
    runs-on: macos-15
    steps:
      - uses: actions/checkout@v4
        with: { fetch-depth: 0 }
      - run: just check-full
```

```yaml
# .github/workflows/protected-paths.yml : 기준 브랜치의 정의로 돈다
name: protected-paths
on:
  pull_request_target:
    types: [opened, synchronize, reopened]
permissions:
  contents: read
jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          ref: ${{ github.event.pull_request.head.sha }}
          fetch-depth: 0
          persist-credentials: false
      - name: verify signatures on protected changes
        env:
          BASE: ${{ github.event.pull_request.base.sha }}
          HEAD: ${{ github.event.pull_request.head.sha }}
        run: git show "$BASE:harness/verify-protected.sh" | bash -s -- "$BASE" "$HEAD"
```

```bash
#!/usr/bin/env bash
# harness/verify-protected.sh : PR 쪽 코드를 실행하지 않는다. 신뢰 자료는 모두 BASE에서 읽는다.
set -euo pipefail
base="$1"; head="$2"
signers="$(mktemp)"; writable="$(mktemp)"
git show "$base:harness/allowed_signers" > "$signers"
git show "$base:harness/agent-writable.txt" > "$writable"
fail=0
for c in $(git rev-list --reverse "$base..$head"); do
  protected=""
  while IFS= read -r f; do
    ok=0
    while IFS= read -r pat; do
      [ -z "$pat" ] && continue
      case "$f" in $pat) ok=1; break;; esac
    done < "$writable"
    [ "$ok" -eq 0 ] && protected="$protected $f"
  done < <(git diff-tree --no-commit-id --name-only -r -m "$c")
  if [ -n "$protected" ]; then
    if ! git -c gpg.format=ssh -c gpg.ssh.allowedSignersFile="$signers" verify-commit "$c" 2>/dev/null; then
      echo "unsigned change to protected paths in $c:$protected"; fail=1
    fi
  fi
done
exit "$fail"
```

`agent-writable.txt` 예: `crates/*/src/*`, `crates/*/tests/*`, `apps/desktop/src/*`, `apps/desktop/src-tauri/src/*`, `prompts/*`, `fixtures/*`, `docs/*`(`case` 패턴의 `*`는 `/`도 맞춘다). `docs/harness.md`와 `docs/workflow.md`는 따로 빼는 처리가 필요하다. 의존성 목록 파일(`Cargo.toml`, `package.json`)의 "의존성 절만 바뀌었는지" 검사는 기준 브랜치의 `xtask`를 빌드해 돌려야 한다. 이 스크립트에는 넣지 않았다.

## 4. 위험과 미확인 항목

위험:
1. **샌드박스는 거부 목록이다.** 목록에 없는 새 보호 대상 파일(예: 새 도구 설정 파일)을 셸로 만드는 것은 로컬에서 막지 못한다. CI 허용 목록 검사가 생길 때까지는 틈이다.
2. **`SubagentStop` 훅의 타임아웃·실패는 종료를 허용한다.** `just check`가 25분(초안의 1500초)을 넘으면 검사 없이 끝난다. `agent_result.py`가 "unchecked"를 메인에 알리는 것이 유일한 보완이다.
3. **guard_bash는 문자열 검사다.** 스크립트 파일을 만들어 실행하는 식으로 우회할 수 있다(권한 문서: Bash 규칙은 "보안 경계가 아니다"). [permissions] 훅 끄기는 CI가, 파일 쓰기는 샌드박스가 최종 방어다.
4. **리뷰어의 원본 오염**은 감지 후 되돌리기에 의존한다. 리뷰어가 되돌리지 않고 8회 상한에 걸리면 오염이 남는다. 이때 `systemMessage`가 사용자에게 보인다. 메인은 `git status`로 다시 확인해야 한다.
5. 샌드박스 때문에 빌드 도구가 깨질 수 있다. `cargo`가 `~/.cargo`에 쓰고, `pnpm`이 저장소 밖 캐시에 쓴다. 홈 쓰기가 막혀 있으므로 `allowWrite`에 캐시 경로를 더해야 할 가능성이 크다. 이 저장소의 실제 빌드로는 **미확인**이다.
6. `permissionMode`는 메인이 `acceptEdits`, `auto`, `bypassPermissions`이면 무시된다. 메인 세션을 `acceptEdits`로 돌리면 리뷰어도 편집 자동 승인 모드가 된다. 다만 리뷰어는 편집 도구가 없고 훅이 막는다.
7. 프로젝트 `.claude/settings.json`의 훅은 대화형에서 **작업 공간 신뢰를 받아들인 뒤에만** 돈다. 신뢰를 거부한 세션에서는 하네스 훅이 없다. 거부 규칙과 샌드박스 경로 규칙은 그대로 적용된다(best-practices: 거부 규칙은 바로 적용). [hooks], [best-practices]
8. 공식 문서는 계속 바뀐다. 2.1.2xx~2.1.3xx 사이에도 `onFailure`(2.1.295), `omitClaudeMd`(2.1.271), 포크 기본값(2.1.302), 쓰기 리다이렉트 검사 같은 동작이 추가되거나 바뀌었다. 하네스 자체 테스트(harness.md 6장 "위반 예제")에 위 실험 12개를 넣어 버전 업그레이드 때마다 돌려야 한다.

미확인:
- `--settings`로 넘긴 샌드박스 상대 경로가 무엇을 기준으로 풀리는지(실측으로는 프로젝트 기준이 아님).
- 대화형 세션에서 `--setting-sources project,local`이 구독 로그인과 함께 동작하는지(`-p`에서는 동작).
- `SubagentStop`에서 `continue: false`가 메인까지 멈추는지.
- 리뷰어가 구현자 설명을 볼 때와 안 볼 때를 직접 비교한 통제 실험, 반박 단계의 오탐 감소 수치.
- 작업 크기(줄 수, 파일 수) 상한의 1차 근거.
- 개인 비공개 저장소에서 GitHub 규칙 세트 "파일 경로 제한"을 쓸 수 있는지.
- 샌드박스가 Secretive SSH 에이전트 소켓 접근을 막는지. 막지 않아도 서명마다 Touch ID가 필요하다.
- 테스트 입력 리터럴 재사용 검사(`xtask`)가 구현을 베낀 테스트를 실제로 얼마나 잡는지.

## 5. 이 결과로 바뀌어야 할 설계 문서 항목

| 문서·항목 | 바꿀 내용 |
|---|---|
| harness.md 7.1 "강제" | "셸 명령을 통한 쓰기를 막는 방법은 R-10에서 확정"을 다음으로 바꾼다: **권한 deny + PreToolUse 허용 목록 훅 + 엄격 샌드박스(`enabled`, `allowUnsandboxedCommands: false`, `failIfUnavailable: true`) denyWrite + CI 서명·허용 목록 검사.** 샌드박스는 거부 목록이라 허용 목록의 정확한 강제는 CI가 맡는다고 명시한다 |
| harness.md 7.1 보호 목록 | 추가: `lefthook-local.yml`, `.lefthook-local.yml`, `.mcp.json`, `.worktreeinclude`, `.gitignore`, `rust-toolchain.toml`, `harness/`(서명자 목록, 에이전트 쓰기 허용 목록, 검증 스크립트). `.claude/` 전체가 아니라 `.claude/worktrees/`를 뺀 하위 경로로 적는다 |
| harness.md 7.2 훅 표 | 추가할 훅: `PreToolUse(Agent)`(종류 허용 목록, `model`·`isolation`·백그라운드 인자 거부), `PostToolUse(Agent)`(검사 상태와 실제 모델을 메인에 전달), `SubagentStart/Stop`(리뷰어·비판자의 원본 변경 감지), `ConfigChange`(세션 중 설정 변경 거부). `SubagentStop` 행에 "3회는 훅이 `agent_id`별로 센다. 내장 상한 8은 도구 호출마다 초기화된다. 훅 타임아웃 시 종료가 허용되므로 메인이 상태를 확인한다"를 적는다 |
| harness.md 7.2 `PreToolUse(셸)` | 거부 목록에 `-n`, `LEFTHOOK_CONFIG`, `GIT_CONFIG_PARAMETERS`·`GIT_CONFIG_COUNT`, `git -c`, `lefthook install/uninstall`, `chflags`를 더한다. "문자열 검사라 최선 노력"임을 명시한다 |
| harness.md 7.3 | "모두 Opus 5.5" → "정의는 `claude-opus-5-5`, `model` 인자는 훅으로 거부, `resolvedModel`을 기록". "리뷰어와 비판자는 격리된 작업 사본" → "`isolation: worktree` + 대상 SHA로 `checkout --detach` + 원본 변경 감지 훅". 서브에이전트 깊이 1, 포크 금지, `SendMessage` 금지를 더한다 |
| harness.md 7.1 마지막 관문 | 서명 방식을 구체화한다: Secure Enclave SSH 키(Touch ID 필수), `commit.gpgsign=false`, 사용자가 보호 파일 커밋에만 `-S`. CI는 **기준 브랜치의** `allowed_signers`와 허용 목록으로 `pull_request_target`에서 검증. 병합은 사람이 한다 |
| harness.md 6장 "하네스 자체" | 이 문서 2장 "실행 검증 요약"의 12개 실험을 하네스 회귀 테스트로 추가한다(Claude Code 업그레이드마다) |
| harness.md 2장 | "기존 테스트 삭제·약화 금지"를 `xtask test-diff`(기준 커밋 대비 기존 테스트 함수 삭제·변경 검출)로 강제하는 규칙을 더한다(ImpossibleBench 근거) |
| workflow.md 2.1 브리프 | 기준 커밋 SHA, 검사 명령, "막힘" 출구 항목을 추가한다(Q8 초안) |
| workflow.md 2.3 | 리뷰어에게 "변경 차이"를 주는 대신 **기준·대상 SHA**를 주고 리뷰어가 직접 `git diff`를 만든다. 리뷰어는 판정하지 않고 등급만 매긴다 |
| workflow.md 2.6 | "수정은 새 implementer"는 `SendMessage` 금지로 강제된다고 적는다 |
| workflow.md 3장 | 반박 단계의 효과를 측정하는 항목(기각률, 메인의 뒤집기 비율, 나중에 버그로 확인된 기각)을 `docs/reviews/` 기록에 더한다 |
| `.claude/agents/*.md` 세 개 | 3.3 초안으로 바꾼다(`tools`, `permissionMode`, `isolation`, 등급 기준, 증거 형식, "막힘" 출구). 이 작업에서는 수정하지 않았다 |
| tasks.md | 하네스 작업을 추가한다: `.claude/settings.json`과 훅 스크립트, `harness/verify-protected.sh`와 워크플로, `xtask test-diff`·`lint-file`, 하네스 회귀 테스트, 샌드박스에서 실제 빌드 확인(`allowWrite` 캐시 경로) |

## 6. 출처

모두 2026-11-11에 확인했다.

- [hooks] Claude Code Hooks reference — https://code.claude.com/docs/en/hooks
- [hooks-guide] Hooks guide (Stop hook block cap 등) — https://code.claude.com/docs/en/hooks-guide
- [sub-agents] Create custom subagents — https://code.claude.com/docs/en/sub-agents
- [permissions] Configure permissions — https://code.claude.com/docs/en/permissions
- [sandboxing] Configure the sandboxed Bash tool — https://code.claude.com/docs/en/sandboxing
- [settings-reference] Settings reference — https://code.claude.com/docs/en/settings-reference
- [env-vars] Environment variables (`CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`, `CLAUDE_CODE_FORK_SUBAGENT` 등) — https://code.claude.com/docs/en/env-vars
- [worktrees] Run parallel sessions with worktrees — https://code.claude.com/docs/en/worktrees
- [best-practices] Best practices for Claude Code — https://code.claude.com/docs/en/best-practices
- [managed-settings] Managed settings — https://code.claude.com/docs/en/managed-settings
- [changelog] Claude Code changelog (`8 consecutive blocks`, `last_assistant_message`, `agent_id` 추가 등) — https://code.claude.com/docs/en/changelog
- [code-review-plugin] anthropics/claude-plugins-official `plugins/code-review/commands/code-review.md` — https://github.com/anthropics/claude-plugins-official/tree/main/plugins/code-review
- [anthropic-harness] Prithvi Rajasekaran, "Harness design for long-running application development", Anthropic Engineering, 2026-03-24 — https://www.anthropic.com/engineering/harness-design-long-running-apps
- [bias-injection] Alexopoulos, Alexopoulos, Spinellis, Mitropoulos, "When AI Agrees Too Much: Manipulating Automated Code Review via Bias Injection", arXiv 2603.18740 v5 (2026-10-27) — https://arxiv.org/abs/2603.18740
- [cover] "Don't Judge Code by Its Cover: Exploring Biases in LLM Judges for Code Evaluation", arXiv 2505.16222 — https://arxiv.org/abs/2505.16222
- [criticgpt] McAleese et al., "LLM Critics Help Catch LLM Bugs", arXiv 2407.00215 — https://arxiv.org/abs/2407.00215 ; OpenAI 소개글 https://openai.com/index/finding-gpt4s-mistakes-with-gpt-4/
- [mad] Smit et al., "Should we be going MAD? A Look at Multi-Agent Debate Strategies for LLMs", arXiv 2311.17371 (ICML 2024) — https://arxiv.org/abs/2311.17371
- [selfcorrect] Huang et al., "Large Language Models Cannot Self-Correct Reasoning Yet", arXiv 2310.01798 (ICLR 2024) — https://arxiv.org/abs/2310.01798
- [impossiblebench] Zhong, Raghunathan, Carlini, "ImpossibleBench: Measuring LLMs' Propensity of Exploiting Test Cases", arXiv 2510.20270 — https://arxiv.org/abs/2510.20270
- [metr-rh] METR, "Recent Frontier Models Are Reward Hacking", 2025-06-05 — https://metr.org/blog/2025-06-05-recent-reward-hacking
- [metr-horizon] METR, "Measuring AI Ability to Complete Long Tasks", arXiv 2503.14499 — https://arxiv.org/abs/2503.14499
- [lefthook-docs] lefthook 문서 (`LEFTHOOK`, `LEFTHOOK_CONFIG`, `rc`, `ai`, `assert_lefthook_installed`), v2.2.2 — https://github.com/evilmartians/lefthook/tree/master/docs
- [just] casey/just 1.59.0 — https://github.com/casey/just
- [secretive] maxgoedjen/secretive README, v4.0.0 — https://github.com/maxgoedjen/secretive
- [git-config] git-config `gpg.ssh.allowedSignersFile` (로컬 git 2.45.2 man 페이지, 최신 2.56.1) — https://git-scm.com/docs/git-config
- [ssh-keygen] ssh-keygen(1) "ALLOWED SIGNERS" (macOS 로컬 man 페이지) — https://man.openbsd.org/ssh-keygen#ALLOWED_SIGNERS
- [gh-events] GitHub Docs, Events that trigger workflows (`pull_request_target`) — https://docs.github.com/en/actions/reference/workflows-and-actions/events-that-trigger-workflows
- [gh-rulesets] GitHub Docs, Available rules for rulesets — https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/available-rules-for-rulesets
- 이 PC: `claude --help`, `claude --version`(2.1.322), 실험 로그 `<exp>/logs/t1.jsonl`~`t12.jsonl`, `t7.stream`
