# ADR 초안: H-61 Antigravity 세션 훅으로 동일 정책 강제

- 상태: 초안
- 날짜: 2026-11-23
- 관련 작업: H-61

## 배경

H-60에서 구현·수정과 조사·비판의 제공자를 Antigravity(`gemini-4-neon`)로 전환하면서 Codex·Claude 세션 훅이 적용되지 않는 집행 빈틈이 남았다. 2026-11-23 사용자 결정("안티그래비티도 호환할 수 있게 해보자")과 메인 추가 지시에 따라 Antigravity 세션 훅(`.agents/hooks.json`)의 실제 프로토콜을 T3 `antigravity acp` 환경에서 실측하고, 기존 단일 정책 원본을 재사용하는 Antigravity 훅 어댑터(`harness/antigravity-hooks/`)를 구현한다.

## 실측으로 확정한 Antigravity ACP 훅 프로토콜

T3 Code가 실행하는 `antigravity acp` (`cli-1.5.14`, `runtimeMode: "full-access"`)에서 격리된 하위 작업 사본(`delegate_task`)과 세션 턴 전환으로 아래 동작을 직접 실측했다.

| 항목 | 실측 결과 |
|---|---|
| 설정 파일 경로 | 저장소 루트의 `.agents/hooks.json` (`pre_tool_execution` 형식이 아닌 `PreToolUse`·`PostToolUse`·`Stop` 형식) |
| 구조 | 최상위 키가 훅 이름(`stock-copilot-harness`). `PreToolUse`·`PostToolUse`는 `[{ "matcher": "*", "hooks": [{ "type": "command", "command": "...", "timeout": 30 }] }]` 묶음 배열, `Stop`은 `[{ "type": "command", "command": "...", "timeout": 120 }]` 평면 배열 |
| 훅 실행 작업 폴더 | 훅 프로세스의 `process.cwd()`는 `<작업 사본>/.agents`, 표준 입력 `cwd`는 `<작업 사본>`, `workspacePaths`는 `["<작업 사본>"]` |
| 확인된 표준 입력 필드 | 공통: `hookEventName`, `conversationId`, `cwd`, `workspacePaths`, `modelName`(`gemini-4-neon-xhigh`·`gemini-4-neon-high`), `transcriptPath`, `artifactDirectoryPath`, `lastUserInput`. `PreToolUse`: `stepIdx`, `toolCall: { id, name, args }`. `Stop`: `executionNum`, `terminationReason`, `error`, `fullyIdle`, `continuationActive` |
| 확인된 도구 이름과 인자 | `read_file({ Path })`, `write_file({ Path, Content })`, `edit_file({ Path, Target, Replacement })`, `run_command({ CommandLine, ... })`, `mcp_t3-code_*` |
| 강제력 | `PreToolUse` 표준 출력 `{"decision":"deny","reason":"..."}`가 `runtimeMode: "full-access"`에서도 별도 수동 신뢰 창 없이 `read_file`·`write_file`·`edit_file`·`run_command`·`schedule`을 즉시 차단 |
| 닫힌 실패 | 훅 명령 실행 실패(예: 실행 경로 누락) 시 해당 턴의 후속 도구 호출을 닫힌 실패로 차단. `.agents/hooks.json`은 각 턴 시작 시 다시 읽고 턴 중에는 캐시 |
| 주의 사항 | `lastUserInput`에는 사용자·하위 에이전트 메시지가 포함되므로 판정기는 `lastUserInput`을 경로 매칭에 사용하지 않고 `toolCall.name`·`toolCall.args`와 OS `process.cwd()`만 검사 |

## 결정

### 1. 단일 정책 원본 재사용 (사본 금지)

`harness/antigravity-hooks/`는 기존 정책과 판정 코어를 직접 가져다 쓴다.

- 보호 경로·쓰기 허용 목록·잠금 스위치·규칙: `.claude/hooks/lib/project.mjs`(`loadProject`), `.claude/hooks/lib/paths.mjs`, `.claude/hooks/lib/edit-guard.mjs`(`editFindings`), `.claude/hooks/lib/bash-guard.mjs`(`bashFindings`, `WRITERS`, `DEFINITIONS`, `unwrap`), `.claude/hooks/lib/verdict.mjs`(`decide`), `harness/agent-writable.txt`, `harness/claude-hooks/rules.json`, `harness/claude-hooks/lock.txt`
- 홈 읽기 차단과 허용 네트워크 도메인: `harness/sandbox/fragment.mjs`(`readPolicy`)로 `harness/sandbox/policy.json`의 `base.filesystem.denyRead`·`base.network.allowedDomains`를 런타임에 읽고, `rules.json`의 `paths.protectedHome`과 합친 뒤 Antigravity 고유 경로(`~/.gemini`, 셸 기록, 작업 사본 밖 `MEMORY.md`)만 덧붙인다.
- 토스 운영 호스트: `harness/arch-check/rules.toml`의 `[source.toss-host]` `needle`(`tossinvest.com`)을 런타임에 읽는다(읽기 실패 시 닫힌 실패).
- 위임 대상 검증: `.claude/hooks/lib/orchestration.mjs`의 `delegateTargetFindings`를 직접 호출한다.
- 셸 패치·작업 폴더 검증·세션 해석: `harness/codex-hooks/adapter.mjs`(`patchPaths`, `shellPatches`, `toolDirectory`)와 `harness/codex-hooks/session.mjs`(`sessionProject`)를 재사용한다.

### 2. 네 가지 최소 강제 범위와 종료 훅

1. **보호 경로 쓰기 차단**: `write_file`·`edit_file`·IDE 별칭(`write_to_file`, `replace_file_content`, `multi_replace_file_content`, `Edit`, `Write`, `NotebookEdit`)과 셸 쓰기·리다이렉션·`apply_patch` heredoc에 `editFindings`·`bashFindings`를 적용한다. `.agents/hooks.json`, 하위 `.agents/**`, `GEMINI.md`, `.claude/settings.local.json`, `.cargo/config*`, `.git/**`, `docs/progress.md`, `docs/tasks.md`, `~/.gemini/**`를 포함한 홈 설정은 `lock.txt = off`에서도 즉시 거부한다. `harness/agent-writable.txt` 밖 보호 경로(`justfile`, `harness/**`, `docs/harness.md`, 추적된 `docs/adr/*` 등)는 `lock.txt = on`에서 모두 거부한다. 잠금 전(`lock.txt = off`) 구현자 역할에 한해 `docs/reviews/<ID>.md` 작성과 정적 `node harness/claude-hooks/hashes.mjs --write`·`node harness/sandbox/fragment.mjs --write`를 허용하고, 잠금 `on`에서는 모두 거부한다(`config.mjs --write`는 `cwd`·`..`·심볼릭 링크·동적 스크립트 인자까지 해석해 차단). `parity.test.mjs`로 Claude `bash-cases.json`·`edit-cases.json` 전체 거부 코퍼스 대비 약화가 없음을 고정한다.
2. **홈 설정·메모리·자격 증명 읽기 차단**: `read_file`·탐색 도구 별칭(`view_file`, `list_dir`, `find_by_name`, `grep_search`, `Read`, `Glob`, `Grep`, 추가 `*Path`/`*File`/`*Dir`/`*Root` 키 포함)과 셸 명령·`<` 리다이렉션·환경 변수 대입·`export` 값·`key=value`(`dd if=...`)·`cd`/`pushd`·`env -C`·`curl -n`/`--netrc-file`/`-d@...`·`security` 키체인 도구 전면 차단을 검사한다. `~`·`$HOME`·`${HOME}`·상대 경로·`..`·심볼릭 링크(끊긴 심볼릭 링크 포함 `lstatSync`+`readlinkSync` 체인)를 모두 해석하며 `~user`, 홈 루트 직접 탐색, 홈 첫 세그먼트 와일드카드(`~/.s?h/...`)도 거부한다.
3. **리뷰·비판 역할의 추적 파일 쓰기와 위임 제한**: `process.cwd()`(호스트가 고정한 `<작업 사본>/.agents`)로 세션 작업 사본을 판정해 `input.cwd` 위조를 막는다. 링크된 작업 사본에서 detached HEAD, `master`/`main`, `review/*`·`critic/*` 브랜치, 또는 호스트가 주는 비-`xhigh` 모델 이름(`-high`, `-medium`, `-low`)이면 `ROLE.reviewer`로 좁혀 모든 편집·쓰기 도구, 추적 파일 셸 쓰기, 변경형 `git` 하위 명령(`add`, `commit`, `rm`, `mv`, `clean`, `restore`, `stash`, `reset`, `merge`, `worktree`, `fetch`, `submodule`, `sparse-checkout`, `bisect`, `gc`, `prune`, `branch` 생성·삭제, 단독 `checkout --detach <SHA>` 외 `checkout`/`switch` 변형), 모든 하위 위임(`mcp_t3-code_delegate_task`, `invoke_subagent`)을 차단한다.
4. **실제 주문·토스 API 호출 명령 차단**: `toss-smoke`·`broker-toss-trade`·`src-tauri`(`gen-bindings` 제외) 직접 실행과 `cargo run`/`cargo r`(`--manifest-path` 및 crate 폴더 내부 실행 포함), `tauri dev`(`pnpm exec tauri dev`·직접 `tauri dev` 포함), `TOSS_*` 환경 변수 설정·내보내기, `rsync` 원격 전송, `curl`/`wget` 설정·프록시·입력 파일 플래그(`-K`/`--config`/`-x`/`--proxy`/`-i`), `openssl s_client`/`s_time`, 네트워크 도구의 `tossinvest.com` 또는 허용 도메인·루프백 밖 연결(정수 IP 등 닫힌 실패), 비읽기 명령의 `tossinvest.com` 전달을 차단한다.
5. **닫힌 실패와 위변조 방지 래퍼**: `.agents/hooks.json`은 `--git-common-dir`로 주 작업 트리의 `harness/antigravity-hooks/dispatch.mjs`를 우선 실행한다. 병합 전 브랜치에서만 주 저장소 `HEAD`에 해당 파일이 없을 때 한해 `../harness/antigravity-hooks/dispatch.mjs`로 폴백하고, 병합 후 주 파일이 사라지거나 Git·Node·JSON 파싱·도구 이름 검증이 실패하면 `PreToolUse`는 `{"decision":"deny",...}`, `Stop`은 `{"decision":"continue",...}`로 닫는다.
6. **`Stop` 종료 훅과 `PostToolUse`**: `Stop`은 `terminationReason === "NO_TOOL_CALL"`, `fullyIdle !== false`, `continuationActive !== true`일 때 리뷰어의 추적 파일 변경과 구현자의 미커밋 변경·Git 훅 미설치를 `decision: "continue"`로 막는다. `PostToolUse`는 Antigravity 계약에 따라 `{}`를 반환한다.
7. **무결성 해시와 생성물 검사**: `harness/claude-hooks/hashes.mjs`에 `ANTIGRAVITY_HASH_DIRECTORIES = [".agents", "harness/antigravity-hooks"]`를 추가해 기존 `HASH_DIRECTORIES` 계약을 유지하면서 `.agents/hooks.json`과 어댑터 전체를 `hashes.json`으로 보호한다. `justfile`의 `check-ts`·`check-fast-static`에 `node harness/antigravity-hooks/config.mjs`와 테스트 러너를 연결한다.

### 3. 훅 미적용 세션 탐지 절차

- Antigravity 세션 훅은 매 `PreToolUse`마다 상태 폴더(`$STOCK_COPILOT_HARNESS_STATE` 또는 `~/.local/state/stock-copilot-harness`)에 `antigravityaudit-<sha256>.json` 감사 기록을 남긴다(쓰기 불가 환경에서는 판정을 깨뜨리지 않고 생략).
- 메인은 리뷰 기록 `## 측정`의 `세션 훅` 행에 실제 적용 여부(`적용(.agents/hooks.json)` 또는 `미적용(사유)`)와 Git 커밋 훅 실행 결과를 적는다. 훅이 적용되지 않은 세션(예: 비신뢰 폴더, 전역 설정이 훅을 끈 환경, 훅 도입 전 브랜치)은 사전 차단이 없는 것으로 간주하고 Git 커밋 훅·대상 스냅숏 전후 비교·원격 CI 전체 검사 결과를 필수로 확인한다.

## 실제 Antigravity 세션 실측 결과

(H-61 커밋 직후 이 브랜치 기준 `delegate_task` 격리 작업 사본에서 실측한 결과를 아래에 추가한다.)

## 알려진 빈틈

- Antigravity `PostToolUse` 훅은 표준 출력으로 `{}`만 허용해 `just lint-file` 실패를 모델에 차단 피드백으로 되돌리지 못한다. 편집 직후 자동 린트 대신 Git 커밋 훅(`lefthook`), `just check-fast`, 원격 CI가 검증한다.
- Antigravity 훅은 도구 호출 인자와 셸 구문을 검사하며 Codex Seatbelt 같은 OS 커널 샌드박스로 임의 컴파일 바이너리나 스크립트 파일 내부 파일·네트워크 접근까지 감싸지 않는다. 보호 경로 변경과 테스트 약화는 Git 커밋 훅·원격 CI 서명 검사·독립 리뷰가 사후 탐지한다.
- 사용자 홈 전역 설정(`~/.gemini/**`)의 훅 비활성화나 작업 공간 신뢰 거부는 저장소 파일만으로 강제할 수 없다. 리뷰 기록에 훅 적용 여부를 남기고 미적용 세션은 커밋 훅·스냅숏 비교·원격 CI로 확인한다.
- Antigravity `Stop` 훅은 턴 종료 시 리뷰어 추적 파일 무변경, 구현자 작업 트리 깨끗함, Git 커밋 훅 설치 여부만 빠르게 확인하며 전체 `just check`는 동기 실행하지 않는다. 전체 검사와 변경분 변이는 2026-11-23 결정에 따라 원격 GitHub Actions CI가 수행한다.

## 인계

- 메인: 병합 시 H-60·H-61 ADR 번호 확정, `docs/progress.md` 갱신, 원격 CI 전체 검사 확인.
- H-20(로컬 잠금): `harness/claude-hooks/lock.txt`를 `on`으로 전환하면 Antigravity 훅의 `gated: true` 보호 경로와 잠금 전 생성기 예외도 자동으로 모두 거부로 전환된다.

## 손 변형

`target/H-61/mut-sandbox` 격리 복사본에서 `policy.mjs`·`adapter.mjs`·`dispatch.mjs`의 30개 변형을 `node --check`로 문법 검증한 뒤 실제 Node 테스트(`adapter.test.mjs`, `dispatch.test.mjs`)로 실행했고, 실행 직후 복사본을 삭제했다. 첫 실행에서는 27/30 검출(3개 생존: 공백 `needle`, detached HEAD의 `TypeError` 은닉, `hasToss` 중복 경로)이 나와 회귀 단언을 추가한 뒤 재실행해 **30/30 검출, 생존 0건**을 확인했다.

| 번호 | 바꾼 것 | 결과와 잡은 테스트 |
|---|---|---|
| 1 | `policy.mjs`: `tossHostFromRules` 빈 `needle` 허용 | 검출: `single policy source changes propagate directly into Antigravity verdicts` |
| 2 | `policy.mjs`: `expandHome` 다른 사용자 홈(`~user`) 차단 제거 | 검출: `Antigravity pre-tool enforces the allow and deny matrix across all four scopes` |
| 3 | `policy.mjs`: `expandHome` `$HOME` 치환 제거 | 검출: `Antigravity pre-tool enforces the allow and deny matrix across all four scopes` |
| 4 | `policy.mjs`: `matchesHomeEntry` `/**` 상위 디렉터리 자체 매칭 제거 | 검출: `evasion attempts using relative paths dotdot symlinks redirection tool aliases and shell nesting are denied` |
| 5 | `policy.mjs`: `resolveLinkChain` 심볼릭 링크 추적 제거 | 검출: `evasion attempts using relative paths dotdot symlinks redirection tool aliases and shell nesting are denied` |
| 6 | `policy.mjs`: `isDeniedReadPath` 홈 루트 직접 탐색 차단 제거 | 검출: `Antigravity pre-tool enforces the allow and deny matrix across all four scopes` |
| 7 | `policy.mjs`: `isDeniedReadPath` 작업 사본 밖 `MEMORY.md` 차단 제거 | 검출: `Antigravity pre-tool enforces the allow and deny matrix across all four scopes` |
| 8 | `policy.mjs`: `antigravityPathFindings` `.agents/hooks.json` 쓰기 차단 제거 | 검출: `Antigravity pre-tool enforces the allow and deny matrix across all four scopes` |
| 9 | `policy.mjs`: `antigravityPathFindings` `GEMINI.md`·하위 `.agents` 위치 차단 제거 | 검출: `Antigravity pre-tool enforces the allow and deny matrix across all four scopes` |
| 10 | `adapter.mjs`: `isReviewModelName` `-high` 모델 리뷰 역할 분류 제거 | 검출: `Antigravity pre-tool enforces the allow and deny matrix across all four scopes` |
| 11 | `adapter.mjs`: `isReviewBranch` detached HEAD(`undefined`) 리뷰 역할 분류 제거 | 검출: `single policy source changes propagate directly into Antigravity verdicts` |
| 12 | `adapter.mjs`: `isReviewBranch` `master`/`main` 브랜치 리뷰 역할 분류 제거 | 검출: `Antigravity pre-tool enforces the allow and deny matrix across all four scopes` |
| 13 | `adapter.mjs`: `isReviewBranch` `review/`·`critic/` 접두사 리뷰 역할 분류 제거 | 검출: `Antigravity pre-tool enforces the allow and deny matrix across all four scopes` |
| 14 | `adapter.mjs`: `TOSS_` 환경 변수 차단 제거 | 검출: `Antigravity pre-tool enforces the allow and deny matrix across all four scopes` |
| 15 | `adapter.mjs`: `security` 키체인 조회 명령 차단 제거 | 검출: `evasion attempts using relative paths dotdot symlinks redirection tool aliases and shell nesting are denied` |
| 16 | `adapter.mjs`: `cargo run -p toss-smoke` 차단 제거 | 검출: `Antigravity pre-tool enforces the allow and deny matrix across all four scopes` |
| 17 | `adapter.mjs`: `cargo run -p src-tauri` 실앱 실행 차단 제거 | 검출: `Antigravity pre-tool enforces the allow and deny matrix across all four scopes` |
| 18 | `adapter.mjs`: `curl -n`/`--netrc` 차단 제거 | 검출: `evasion attempts using relative paths dotdot symlinks redirection tool aliases and shell nesting are denied` |
| 19 | `adapter.mjs`: 네트워크 도구 토스 호스트 차단 제거 | 검출: `single policy source changes propagate directly into Antigravity verdicts` |
| 20 | `adapter.mjs`: 네트워크 도구 허용 목록 밖 호스트 차단 제거 | 검출: `Antigravity pre-tool enforces the allow and deny matrix across all four scopes` |
| 21 | `adapter.mjs`: `reviewerGitFindings` `git add`/`commit` 차단 제거 | 검출: `Antigravity pre-tool enforces the allow and deny matrix across all four scopes` |
| 22 | `adapter.mjs`: `lockedConfigGeneratorFindings` 잠금 `on` `config.mjs --write` 차단 제거 | 검출: `Antigravity pre-tool enforces the allow and deny matrix across all four scopes` |
| 23 | `adapter.mjs`: `delegateTaskFindings` 리뷰어 위임 차단 제거 | 검출: `unknown tools malformed inputs and disallowed delegations fail closed` |
| 24 | `adapter.mjs`: `delegateTaskFindings` `worktree` 격리 필수 검사 제거 | 검출: `unknown tools malformed inputs and disallowed delegations fail closed` |
| 25 | `adapter.mjs`: `manage_task` `send_input` 차단 제거 | 검출: `unknown tools malformed inputs and disallowed delegations fail closed` |
| 26 | `adapter.mjs`: 알 수 없는 도구 기본 거부 제거 | 검출: `unknown tools malformed inputs and disallowed delegations fail closed` |
| 27 | `adapter.mjs`: `antigravityStop` 리뷰어 추적 파일 변경 시 `continue` 차단 제거 | 검출: `dispatchEvent handles PreToolUse PostToolUse Stop and records audit` |
| 28 | `adapter.mjs`: `antigravityStop` 구현자 미커밋 변경 시 `continue` 차단 제거 | 검출: `dispatchEvent handles PreToolUse PostToolUse Stop and records audit` |
| 29 | `adapter.mjs`: `antigravityStop` 훅 미설치 시 `continue` 차단 제거 | 검출: `dispatchEvent handles PreToolUse PostToolUse Stop and records audit` |
| 30 | `dispatch.mjs`: 알 수 없는 이벤트 닫힌 실패 제거 | 검출: `dispatchEvent handles PreToolUse PostToolUse Stop and records audit` |
