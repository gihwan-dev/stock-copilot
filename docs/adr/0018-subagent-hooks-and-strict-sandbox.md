# ADR 0018: 서브에이전트 훅과 통제, 엄격 샌드박스

- 상태: 제안
- 날짜: 2026-11-13
- 결정한 사람: 메인 에이전트(사용자 확인 대기)
- 관련 작업: H-25, H-35
- 반영한 문서: 없음(이 ADR만 추가. harness.md 7.2~7.5와 다른 점은 "문서와 다른 점"에 모았다)

## 맥락

harness.md 7.3~7.5는 `PreToolUse`(Agent), `PostToolUse`(Agent), `SubagentStart`, `SubagentStop` 2종, 환경 변수, `SendMessage`·`Agent(fork)` 거부를, 7.2는 엄격 샌드박스를 정했다. ADR 0013은 기준 SHA 기록과 낡은 빌드(H-25), `sandbox-exec` 중첩(H-35)을, ADR 0015·0017은 셸 훅이 못 막는 쓰기(스크립트 파일, 인터프리터, 하드 링크, 다시 연결한 작업 사본, `PATH` 앞 가짜 실행 파일, `node_modules/`·`target/` 바꿔치기)를 H-35로 넘겼다. 남은 하네스 작업(H-36, H-12, H-16, H-26, H-27, H-20)이 보호 파일을 만들어야 하므로 보호 경로를 막는 `denyWrite`는 잠금(H-20)과 함께 켠다.

## 실측

환경: macOS 26 (Apple Silicon), Claude Code 2.1.322, `claude -p --model haiku --setting-sources project --permission-mode acceptEdits`(구독 로그인). 모든 실험은 이 판을 복제한 임시 저장소(`<exp>/stock-copilot`, 작업 사본 `<exp>/stock-copilot-wt/a`)에서 했다. 헤드리스 실행 6회(E1 훅 입력·샌드박스 경로 형식, E1b 허용 경로 형식, E2 잠금 설정 우회 시도와 정상 작업, E3a·E3b·E3c 서브에이전트 훅 끝까지와 정상 작업). 훅 쪽 샌드박스(`sandbox-exec`)는 헤드리스 없이 같은 복제본에서 직접 돌렸다.

### 훅 입력 (E1, E3)

| 이벤트 | 받은 것 |
|---|---|
| `PreToolUse`(Agent) | `tool_input`: `description`, `prompt`, `subagent_type`(넘긴 인자만), `tool_use_id`, `session_id`. 메인에는 `agent_type`이 없다 |
| `SubagentStart` | `agent_id`, `agent_type`, `session_id`, `transcript_path`(메인 기록). **프롬프트가 없다.** 서브에이전트 기록 `<세션>/subagents/agent-<id>.jsonl`은 이 훅이 끝난 뒤에 쓰인다(5초 기다려도 없음, E3a) |
| `SubagentStop` | `agent_transcript_path`(첫 줄이 받은 프롬프트), `last_assistant_message`(마지막 보고), `stop_hook_active` |
| `PostToolUse`(Agent) | `tool_response.status`, `agentId`, `content`, `resolvedModel` |
| 권한 거부 `SendMessage` | 메인의 도구 목록에서 사라진다("SendMessage is not among the tools available") |

### 샌드박스 경로 형식 (E1, E1b)

| 형식 | 결과 |
|---|---|
| `./x`, `./**/x`, `./a/*/b` (deny) | 프로젝트 기준. `**`는 0단계 이상, 폴더를 적으면 그 아래 전부 |
| `../stock-copilot-wt` (allow), `../stock-copilot-wt/*/harness` (deny) | 프로젝트 기준 상위 폴더로 풀린다 |
| `/private/tmp/…`, `//…`, `~/…` | 절대 경로와 홈 |
| allow의 `x_*` 글롭 | 그 이름 자체만 열리고 그 아래는 열리지 않는다. `x_*/*`로 한 단계 아래를 연다. `x_*/**`는 열리지 않았다 |
| 권한 거부 `Edit(/harness/claude-hooks/lock.txt)` | 샌드박스 쓰기 거부에도 합쳐진다(지금 설정에서 `python3`로 잠금 파일 쓰기 실패) |
| 기본 보호 | 원본의 `.claude/settings*.json`, `.claude/hooks`, `.git/hooks`, `.git/config`. 작업 사본(`../stock-copilot-wt/*`)의 `.claude/`는 기본 보호가 아니다 |

### S-03: 샌드박스가 막은 것과 못 막은 것 (E1, E2 잠금 설정, E3b 지금 설정)

| 시도 | 지금 설정 | 잠금 설정 |
|---|---|---|
| `python3 -c "open('harness/claude-hooks/lock.txt','a')"` | 막힘(권한 거부 규칙이 합쳐짐) | 막힘 |
| `node -e` / `python3` 스크립트 파일 / `bash` 스크립트 파일로 `justfile`, `crates/xtask/src`, `harness/` 쓰기 | 허용 | 막힘 |
| 스크립트 파일로 `.claude/hooks/x`, `.claude/settings*.json` 쓰기 | 막힘(기본 보호) | 막힘 |
| `ln harness/agent-writable.txt docs/hl` | 허용 | 막힘 |
| **이미 있던** 하드 링크(`docs/prelinked.txt` ↔ `harness/…`)로 쓰기 | 허용 | **허용(못 막음)** |
| 지운 작업 사본 자리를 원본 `harness/`로 링크한 경로로 쓰기 | 허용 | 막힘(실제 경로로 판정) |
| `git checkout <옛 커밋> -- harness/agent-writable.txt` | 허용 | 막힘(파일은 그대로, 인덱스만 바뀜. 커밋하면 commit-guard가 거부) |
| `sed -i '' … justfile` | 허용 | 막힘 |
| `cargo fmt --all`이 원본의 `crates/xtask` 소스를 고침 | 허용 | 막힘 |
| 같은 쓰기를 작업 사본(`../stock-copilot-wt/a/harness`, `justfile`, `.claude/hooks`)에 | 허용 | **허용(못 막음)** |
| 새 최상위 설정(`vitest.config.ts`), `.github/workflows/x.yml` | 허용 | 막힘 |
| `PATH`의 모든 폴더에 파일 만들기(`~/.bun/bin`, `~/Library/pnpm`, nvm, `~/.local/bin`, Homebrew, `~/.cargo/bin` 등 20곳) | 막힘 | 막힘 |
| `~/Library/pnpm/cargo`(`PATH` 셋째 폴더) | 막힘 | 막힘 |
| `.cargo/config.toml`: 원본 루트, 작업 사본 루트, `../stock-copilot-wt/.cargo`, 그 위 폴더, `~/.cargo/config*` | 막힘 | 막힘 |
| `~/.cargo/registry/src` 쓰기 / `~/.cargo/registry/cache` 쓰기 | 막힘 / 허용 | 같음 |
| `~/.gitconfig`, `~/.config/git`, `~/.claude`, `CLAUDE_CONFIG_DIR`, `~/.rustup/settings.toml`, 상태 폴더 | 막힘 | 막힘 |
| 원본 `.git/hooks`, `.git/config`, `.git/info/grafts` | 막힘 | 막힘 |
| `node_modules/.bin/*`, `target/debug/xtask` 바꿔치기(원본·작업 사본) | **허용(못 막음)** | **허용(못 막음)** |
| `nc 1.1.1.1 443`, `example.com` 이름 풀기 | 막힘 | 막힘 |
| `curl` registry.npmjs.org, static.crates.io / github.com, example.com | 허용 / 막힘(프록시 403) | 같음 |
| 안에서 `sandbox-exec -p … true` | 실패 `sandbox_apply: Operation not permitted` | 같음 |

### 정상 작업 (E3b 지금 설정, E3c 잠금 설정)

| 작업 | 결과 |
|---|---|
| 작업 사본에서 `just base=<SHA> check` 전체 | 통과(E3c, 잠금 설정) |
| `git commit`(커밋 훅: fresh-build, commit-guard, check) | 통과(E3c, 2분 34초) |
| `git worktree add`, 새 작업 사본에서 `pnpm install --frozen-lockfile` | 통과(E3b) |
| `just gen-bindings`, `cargo build --locked --workspace`, `pnpm -C apps/desktop exec tauri build --debug --no-bundle` | 통과(E3b) |
| 막혔던 것과 고친 것 | pnpm 12가 `/private/tmp/pnpm-{store-operation-locks,dir-locks,modules-swaps}-501`에 잠금을 둔다 → 그 세 폴더 allow. 기존 테스트 `fresh_node_restores_a_replaced_tool`의 임시 설치가 `~/Library/pnpm/_tmp_<pid>_<hex>/<파일>`을 만든다 → `_tmp_*`, `_tmp_*/*` allow. `unix_socket_fake_server_answers`가 `$TMPDIR`(`/private/tmp/claude-501`)에 유닉스 소켓을 연다 → `allowUnixSockets`. 루프백 테스트 → `allowLocalBinding`. `cargo deny`가 `~/.cargo/advisory-dbs/db.lock`을 잡는다 → 그 파일만 allow. 처음 판의 `./**/.cargo/config*` 거부는 테스트가 `target/harness/…/.cargo/config.toml` 예제를 지우지 못하게 해서 루트·작업 사본 루트로 좁혔다 |

### 서브에이전트 훅 끝까지 (E3b, E3c)

| 시도 | 결과 |
|---|---|
| 메인이 implementer를 기준 커밋(master 끝)·브랜치 `wt-a`로 띄움 | `SubagentStart`가 PreToolUse 대기열에서 브리프를 찾아 기준을 기록하고 서브에이전트에 알림. 종료 때 훅 샌드박스에서 `just fresh-build`와 `just base=<SHA> check`가 통과, `PostToolUse`가 `상태=passed, 기준=…, HEAD=01e3b8b…, 차단=0, 모델=claude-haiku-5-5 (정의의 claude-opus-5-5가 아니다), 보호 경로 변경=…`을 메인에 붙임 |
| review-critic에 `model: "haiku"` | `[agent-argument]` 거부 |
| `general-purpose` | `[agent-type]` 거부 |
| review-critic 프롬프트에 `하네스 기록:` | `[implementer-report]` 거부 |
| 리뷰어에게 원본 쓰기 스크립트 실행 지시 | 두 번 모두 모델(haiku 리뷰어·메인)이 따르지 않아 끝까지 재현하지 못했다. 훅 입력 JSON 테스트로만 확인했다 |

### 훅 쪽 검사 샌드박스 (직접 실행)

`sandbox-exec`에 아래 프로필을 주고 복제본 작업 사본에서 `just base=<SHA> check`를 돌렸다: 쓰기는 작업 사본, `$TMPDIR` 실제 경로, `/dev`, pnpm 잠금 폴더, `~/.cargo/registry`, `db.lock`, pnpm store, `_tmp_`만 허용하고 git 공통 폴더·원본 `.claude`·`~/.cargo/registry/src`·상태 폴더는 거부, 네트워크는 테스트 프로필과 같은 규칙. 통과(2분 11초). `fresh-build`는 네트워크를 연 같은 쓰기 제한 프로필로 통과.

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| `.claude/settings.json`의 하네스 훅은 9묶음: `PreToolUse`(Edit\|Write\|NotebookEdit, Bash, Agent\|SendMessage), `PostToolUse`(Edit\|Write, Agent), `SubagentStart`(implementer\|adversarial-reviewer\|review-critic), `SubagentStop`(implementer, adversarial-reviewer\|review-critic), `ConfigChange`. `SubagentStart`는 한 묶음이 역할로 나눠 일한다 | 7.3 표의 9종을 유지하면서 implementer 기준 기록(H-25 2)을 넣는다 | `SubagentStart` 두 묶음(10종) | harness 7.3 |
| 새 훅은 `.claude/hooks/`의 Node 진입 파일 다섯(`guard-agent`, `agent-result`, `subagent-start`, `implementer-stop`, `reviewer-stop`)과 `lib/`. 규칙 데이터는 `harness/claude-hooks/agent-rules.json`, 예제는 `agent-cases.json` | ADR 0017과 같은 방식(해시 고정, 원소 빼기 테스트) | rules.json에 합치기(기존 셸 예제 원소 빼기 테스트가 깨진다) | ADR 0017 |
| `PreToolUse`(Agent): 메인이 아니면 거부(`nested-agent`), `SendMessage` 거부, 종류 없음·`fork` 거부, 종류는 `implementer`·`adversarial-reviewer`·`review-critic`만, 인자는 `description`·`prompt`·`subagent_type`만(`model`, `isolation`, `run_in_background`(false 포함), `effort`, `name`, `mode`, `team_name` 등 거부) | 허용 목록이 새 인자에 닫혀 있다 | 금지 인자 목록 | harness 7.4 |
| implementer 브리프는 `기준 커밋:` 줄이 하나(7~40자리 소문자 16진수)여야 하고, 그 커밋이 지금 `refs/heads/master` 끝이거나 상태가 `passed`인 기록의 HEAD여야 한다. 아니면 시작을 거부한다(`brief-base`). 선택: 같은 줄의 ``브랜치 `x` `` 또는 `- 브랜치: x` 줄이 작업 사본을 가리킨다(없으면 원본 작업 사본) | "막힘" 단계의 SHA로 약화를 세탁하는 길을 닫는다. 브리프 양식에 작업 사본 칸이 없어 이 프로젝트가 써 온 "브랜치" 표기를 읽는다 | 시작 허용 후 `unchecked`(검사 없는 구현이 생긴다) | ADR 0013, workflow 2.1 |
| 기준 기록: `SubagentStart`는 프롬프트를 받지 못하므로 `PreToolUse`가 허용한 호출을 세션별 대기열(`pending-<세션>`)에 넣고, `SubagentStart`가 같은 종류의 것을 (서브에이전트 기록이 있으면 그 다이제스트로, `.meta.json`의 `toolUseId`로, 없으면 가장 오래된 것) 꺼내 기준을 다시 확인해 기록한다. `SubagentStop`은 `agent_transcript_path` 첫 줄의 다이제스트가 기록과 다르면 실패시킨다 | 실측(시작 때 기록 파일 없음) | 메인 기록 파싱 | E3a |
| 상태 파일: `~/.local/state/stock-copilot-harness/records/<종류>-<id>.json`(0600, 원자적 교체). 환경 변수 `STOCK_COPILOT_HARNESS_STATE`(절대 경로만)로 바꿀 수 있다(테스트·실측용. 에이전트는 훅의 환경을 바꿀 수 없다). 셸·편집 훅의 `protectedHome`에 상태 폴더를 더했고, 샌드박스 `denyWrite`에도 넣었다 | 7.2 "작업 사본과 임시 폴더 밖" | 서명(HMAC). 같은 사용자가 키도 읽을 수 있어 효과가 없다 | harness 7.2 |
| `SubagentStop`(implementer): (1) 기록이 없거나 기준이 없으면 `unchecked`로 두고 막지 않는다. (2) 브랜치의 작업 사본을 찾고(진짜 작업 사본만, ADR 0017), 브리프 다이제스트, 기준이 HEAD의 조상인지(grafts·replace 끔), 기준 대비 바뀐 경로 중 허용 목록 밖과 기존 ADR을 본다. (3) 문제가 없으면 훅 샌드박스에서 `just fresh-build` 뒤 `just base=<SHA> check`. (4) 통과이고 보고 첫 줄이 정확히 `완료` 또는 `막힘: <이유>`면 `passed`로 끝내게 한다. 실패면 차단 수를 세어 `decision: "block"`과 출력 끝 8000자를 돌려준다. 셋째 차단에 "마지막" 안내. 차단 3회 뒤 첫 줄이 `막힘:`이면 `blocked`로 끝내게 한다. 첫 줄 형식이 틀리면 검사 결과와 관계없이 막는다 | harness 7.3, workflow 2.2 | 차단 수를 `stop_hook_active`로 세기(상한 8과 섞인다) | R-10 Q10 |
| 보호 경로 변경: 잠금이 꺼져 있으면 기록(`protectedChanges`)과 경고만, 켜지면 검사 없이 실패 | 작업 사본의 보호 파일은 샌드박스가 막지 않는다(아래). 남은 하네스 작업이 보호 파일을 고친다 | 지금 바로 실패 | 브리프 |
| 낡은 빌드: 훅은 ADR 0015의 `just fresh-build`(작업 공간 crate 정리, `pnpm install --force`)를 먼저 돈다 | 같은 방식 재사용 | 훅 소유 target(ADR 0015에서 버림) | ADR 0013, 0015 |
| 훅의 검사는 `sandbox-exec`로 감싼다. 쓰기는 작업 사본·`$TMPDIR`·캐시만, git 공통 폴더·원본 `.claude`·레지스트리 소스·상태 폴더는 거부. `fresh-build`는 네트워크 허용, `check`는 테스트 프로필과 같은 네트워크 차단. `SANDBOX_RUNTIME=1`을 준다 | 훅은 샌드박스 밖에서 사용자 권한으로 돈다. 그 안에서 에이전트가 쓴 테스트·빌드 스크립트가 실행되므로, 감싸지 않으면 상태 파일 위조와 훅 파일 변경이 가능하다 | 감싸지 않기 | harness 7.2, R-10 Q2 |
| `SubagentStart`(리뷰어·비판자): 원본과 이 저장소의 모든 진짜 작업 사본(`.claude/worktrees/` 아래 격리 사본 제외)의 HEAD와 `git status --porcelain -uall`을 저장. `SubagentStop`이 다르면 "되돌리고 끝내라"로 막고, 3번 막은 뒤에는 `contaminated`로 기록하고 끝내게 하며 메인에 경고 | 대상 작업 사본도 보라는 H-25 3. 무한 반복을 막는다 | 원본만 | harness 7.5 |
| 리뷰어 격리(7.5) 중 훅이 하는 것: 리뷰어·비판자 프롬프트에 구현자 보고의 표식(줄 전체가 `완료`, `막힘:`으로 시작하는 줄, `하네스 기록:`, 보고 양식 제목)이나 기록된 구현자 보고의 줄(20자 이상, 브리프에 없는 줄)이 있으면 거부(`implementer-report`) | 문자열로 확인할 수 있는 범위 | - | workflow 2.3 |
| `PostToolUse`(Agent): implementer는 상태, 기준, HEAD, 차단 수, `resolvedModel`(정의의 `claude-opus-5-5`와 다르면 표시), 보호 경로 변경, 이유를 메인 맥락에 붙인다. 기록이 없거나 결과가 `completed`가 아니거나 검사 뒤 작업 사본 HEAD가 바뀌었으면 `unchecked`. 리뷰어는 원본 상태 | 7.3 | - | harness 7.3 |
| 수정 회차: `SendMessage`를 권한 거부와 훅으로 막는다. 수정은 새 implementer에게 수정 브리프(원래 브리프 + 새 기준 커밋 = 앞 단계의 `passed` 대상 SHA + 고칠 지적)로 한다. 앞 단계가 `blocked`·`failing`·`unchecked`였으면 그 SHA를 기준으로 쓸 수 없으므로, 수정 브리프의 기준은 master 끝이고 앞 단계의 커밋은 새 implementer가 `git merge`나 같은 브랜치에서 이어 받는다 | 지금까지 메인은 같은 구현자를 이어 썼다. 문서(7.4, workflow 2.6)가 금지한다 | `SendMessage` 허용 | harness 7.4 |
| 환경 변수: `CLAUDE_CODE_FORK_SUBAGENT=0`, `CLAUDE_CODE_DISABLE_BACKGROUND_TASKS=1`, `CLAUDE_CODE_MAX_SUBAGENT_SPAWN_DEPTH=1`, `CLAUDE_CODE_STOP_HOOK_BLOCK_CAP=8`. 권한: `disableBypassPermissionsMode: "disable"`, 거부 `Agent(fork)`, `SendMessage` | 7.4, R-10 3.2 | - | harness 7.4 |
| 테스트 네트워크 차단의 중첩: nextest wrapper를 `/bin/sh -c <스크립트> network-sandbox <프로필>`로 바꾼다. 스크립트는 같은 프로필로 `sandbox-exec … true`가 되면 그 프로필로 감싸고, 안 되면 `SANDBOX_RUNTIME=1`(Claude Code 샌드박스와 훅 샌드박스가 준다)일 때만 감싸지 않고 바깥 샌드박스의 네트워크 정책에 맡기며, 둘 다 아니면 실패한다. 프로필 문자열과 스크립트는 `source_check/cargo_config.rs`의 상수이고 `.config/nextest.toml`과 예제가 그대로 따른다 | 바깥이 차단하므로 "외부 연결 차단" 자체 테스트는 그대로 통과한다(E3c, 훅 샌드박스 실측) | `excludedCommands: ["cargo nextest"]`(샌드박스 밖 실행), 차단 테스트를 샌드박스 정책으로만 대체(로컬·CI의 샌드박스 밖 실행이 무방비) | ADR 0013 |
| 샌드박스 네트워크 허용: `registry.npmjs.org`, `index.crates.io`, `static.crates.io`, `allowLocalBinding: true`, `allowUnixSockets: ["/private/tmp/claude-501"]` | pnpm·cargo의 새 의존성 받기, 루프백·유닉스 소켓 테스트. github.com(권고 DB 받기, 푸시)은 넣지 않는다 | `allowAllUnixSockets`(mDNSResponder가 열려 이름 풀기 차단 테스트가 깨진다) | 실측 |
| `excludedCommands`는 두지 않는다 | 정상 작업이 모두 샌드박스 안에서 통과했다 | - | harness 7.2 |
| 샌드박스 설정의 근거는 `harness/sandbox/policy.json`(지금 켜는 것 `now`, 잠금 조각의 재료 `lock`). 잠금 조각 `harness/sandbox/lock-fragment.json`은 `node harness/sandbox/fragment.mjs --write`가 `agent-writable.txt`와 추적 파일로 만든다: 추적 파일을 폴더 단위로 묶어, 아래에 쓸 수 있는 경로가 없으면 폴더째, 아니면 보호 파일 하나씩 + 제외 줄(`!…`) + 새 최상위 경로 패턴(`*.json`, `*.mjs`, `.github`, `.claude/settings*.json` 등). 테스트가 (a) 파일이 생성 결과와 같은지, (b) 모든 추적 파일에서 "거부됨"과 "허용 목록 밖"이 같은지, (c) 보호·허용 탐침 경로, (d) 허용 목록 줄과 정책 원소를 하나씩 빼면 조각이 바뀌는지 본다 | "같은 근거에서 나오고 어긋나면 실패"(H-35 7) | 손으로 쓴 목록 | harness 7.1, 7.2 |
| 잠금 조각에 의존성 목록 파일(`Cargo.toml`, `package.json`)은 넣지 않는다 | 잠금 뒤 메인이 샌드박스 안에서 `git merge`할 때 구현 브랜치의 의존성 변경을 쓰지 못한다 | 7.2대로 넣기 | 아래 "문서와 다른 점" |
| 잠금 조각은 작업 사본(`../stock-copilot-wt/*/…`)의 보호 경로를 막지 않는다 | 막으면 `git worktree add`가 보호 파일을 만들지 못하고(E1: `cannot create directory at 'harness'`), 구현자의 `git merge master`도 실패한다 | 작업 사본 쪽 거부(정상 작업이 깨진다) | 실측 |

### 지금 켜는 것과 H-20으로 미루는 것

| 항목 | 지금 | H-20 |
|---|---|---|
| 서브에이전트 훅 5개 진입 파일(9묶음), 환경 변수, `Agent(fork)`·`SendMessage` 거부, `disableBypassPermissionsMode` | 켬 | - |
| `sandbox.enabled`, `failIfUnavailable: true`, `allowUnsandboxedCommands: false`, 네트워크 허용 목록, `allowWrite`(작업 사본 폴더, 캐시, pnpm 잠금) | 켬 | - |
| `denyWrite`: 원본·작업 사본 루트·작업 사본 상위의 `.cargo/config*`, `.git/info`, `~/.cargo/registry/src`, `~/.gitconfig`, `~/.config/git`, `~/.claude`, `~/.rustup/settings.toml`, `~/.cargo/config*`, 상태 폴더 | 켬(보호 파일을 만들 작업이 쓰지 않는 곳) | - |
| 종료 훅의 보호 경로 변경 | 경고·기록만 | 실패 |
| 잠금 조각 `denyWrite` 63개(`./harness`, `./.claude/{agents,hooks,settings*.json}`, `./crates/xtask`, `./justfile`, 설정 파일, `./design`, 보호 문서, 새 최상위 패턴) | 준비·테스트 | 합침 |
| 편집·셸 훅의 보호 경로 거부, 긴 권한 `deny`(R-10 3.2), 훅 해시를 서명 커밋과 비교 | - | 켬(ADR 0017) |

### H-20이 할 일

사용자가 Claude Code 밖의 터미널에서(보호 파일이라 세션 안에서는 쓸 수 없다):

```sh
node harness/sandbox/fragment.mjs                       # 조각이 최신인지(0 failure)
node harness/sandbox/fragment.mjs --locked-settings > "$TMPDIR/settings.json"
mv "$TMPDIR/settings.json" .claude/settings.json        # 지금 설정 + 조각의 denyWrite
printf 'on\n' > harness/claude-hooks/lock.txt
node harness/claude-hooks/hashes.mjs --write
```

그다음 `harness/claude-hooks/tests/settings.test.mjs`의 기대 설정을 `lockedSandbox(policy, fragment)`로, 잠금 스위치 테스트를 `on`으로 바꾸고(기존 테스트 변경), `just base=<master> check` 통과를 확인한 뒤 `git commit -S`로 서명한다. 허용 목록·추적 파일이 바뀌어 조각이 어긋나면 첫 명령이 실패한다.

## 결과

- 병합 뒤 이 저장소에서 여는 세션(작업 공간 신뢰를 받아들인 경우):
  - 모든 Bash가 샌드박스에서 돈다. 쓰기는 저장소, `../stock-copilot-wt`, `$TMPDIR`, 캐시뿐이고 홈 설정·`PATH` 폴더·상태 폴더에는 못 쓴다. 네트워크는 npm·crates 레지스트리만(대화형에서는 다른 도메인을 물어보고, `-p`에서는 거부). `github.com`이 없어 `git push`·`just advisory-db-fetch`는 안에서 실패한다. `lefthook install`(`.git/hooks`)과 권고 DB 받기는 사용자가 밖에서 한다.
  - 백그라운드 실행이 꺼진다(`CLAUDE_CODE_DISABLE_BACKGROUND_TASKS`는 셸의 `run_in_background`도 끈다).
  - 메인은 세 종류만, 인자 `description`·`prompt`·`subagent_type`만으로 띄울 수 있다. implementer 브리프에는 `- 기준 커밋: <master 끝 또는 passed 대상 SHA>, 브랜치 \`<작업 브랜치>\``가 있어야 한다. `SendMessage`는 도구 목록에서 사라진다.
  - implementer가 끝내려 할 때마다 훅이 `fresh-build`와 `just check`를 돈다(약 3~5분).
  - 리뷰어·비판자 프롬프트에 구현자 보고의 줄이나 `하네스 기록:`을 넣으면 거부된다.
  - `allowWrite`의 `/private/tmp/…-501`, `allowUnixSockets`의 `claude-501`은 사용자 ID 501 기준이다. 다른 ID면 pnpm 설치와 유닉스 소켓 테스트가 실패한다.
- 기존 자료 변경: `harness/source-checks/cargo-config-cases.toml`의 nextest 예제 16개와 `workspaces/nextest-config-*` 예제 두 개의 `command`를 새 wrapper 형태로 바꿨다(기대 규칙은 그대로). `harness/claude-hooks/tests/settings.test.mjs`의 기대 설정은 바뀐 설정에 맞춰야 한다(기존 테스트 변경, 사용자 승인).
- 원리상 훅과 샌드박스가 못 막는 것:
  - 훅이 돌지 않는 세션: 작업 공간 신뢰 거부, `--setting-sources`에서 project를 뺀 실행. `SubagentStop`의 시간 초과·실패는 막지 못한다(기록이 `unchecked`로 남아 메인에 보인다).
  - 메인이 `PostToolUse` 맥락을 무시하거나, 구현자 보고를 바꿔 써서 리뷰어에게 넘기는 것(문자열 일치만 본다). 메인이 `master`를 옮겨 기준 검사를 통과시키는 것.
  - 검사는 작업 사본의 `justfile`·xtask로 돈다. 작업 사본의 보호 파일은 샌드박스가 막지 않으므로, 잠금 전에는 그것을 고쳐 통과시킬 수 있다(기록·경고에 보호 경로 변경으로 남는다). 잠금 뒤에는 종료 훅이 실패시키고, CI(H-26)가 사후에 잡는다.
  - 이미 있던 하드 링크, `node_modules/`·`target/`의 서드파티 산출물(커밋 훅과 종료 훅의 `fresh-build`, CI의 깨끗한 빌드가 덮는다. 의존 crate의 `target/` 산출물은 `fresh-cargo`가 지우지 않는다), pnpm store(`--force`가 그 store에서 다시 가져온다), 레지스트리 캐시(`registry/cache`).
  - 편집 도구는 샌드박스 밖이다(편집 훅이 맡는다). 훅·MCP 프로세스도 샌드박스 밖이다.
  - Claude Code 샌드박스 안의 테스트는 바깥 정책을 따르므로 허용 도메인에는 프록시로 닿는다. 직접 연결과 이름 풀기는 막힌다.
  - 상태 파일은 같은 사용자가 샌드박스 밖에서 고칠 수 있다(서명 없음).
- 바뀌면 다시 볼 조건: Claude Code가 `SubagentStart` 입력에 프롬프트를 넣을 때, 서브에이전트 기록 경로·`.meta.json` 형식, 샌드박스 경로 형식(allow 글롭), `sandbox-exec` 중첩 규칙, pnpm의 잠금 폴더, `Agent` 도구 인자가 바뀔 때. H-36이 이 ADR의 실측을 회귀 테스트로 옮긴다.

## 문서와 다른 점, 모호한 점

- harness 7.3 표의 `SubagentStart`는 리뷰어용뿐이다. H-25 2가 implementer 기준 기록을 요구해 한 묶음의 매처를 셋으로 넓혔다(9종 유지).
- "기준 커밋이 없으면 시작을 거부하거나 `unchecked`": 둘 다 한다. `PreToolUse`가 거부하고, 그래도 시작된 경우(기록을 못 읽음) `unchecked`.
- 브리프 양식에 작업 사본 칸이 없다. 이 프로젝트의 브리프가 써 온 ``브랜치 `x` ``로 작업 사본을 찾는다. 없으면 원본 작업 사본.
- `run_in_background: false`도 거부한다(인자 허용 목록).
- harness 7.2는 의존성 목록 파일을 `denyWrite`에 넣으라고 했다. 잠금 뒤 메인의 병합이 깨지므로 넣지 않았다(위 결정).
- harness 7.5는 원본의 스냅숏만 말한다. 대상 작업 사본까지 보라는 H-25 3에 따라 모든 진짜 작업 사본을 본다.
- harness 7.3은 차단 훅에 `onFailure: "block"`을 붙이라고 했지만 `SubagentStop`에는 효과가 없어(R-10 Q1) 붙이지 않았다.
- R-10의 "격리된 리뷰어가 원본에 쓸 수 있다"는 이번 판에서 다시 재현하지 못했다(모델이 지시를 따르지 않음). 감지 훅은 그대로 둔다.
