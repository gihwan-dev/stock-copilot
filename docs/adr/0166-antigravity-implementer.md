# ADR 0166: H-60 구현 제공자를 Antigravity로 전환

- 상태: 확정 (2026-11-24)

- 날짜: 2026-11-23
- 관련 작업: H-60

## 사용자 결정

2026-11-23 사용자 결정과 후속 기기 이식 결정에 따라 구현·수정과 조사·비판의 제공자를 Antigravity(Gemini)로 고정하고, 적대적 리뷰는 드라이버 종류 `claudeAgent` + 모델 `claude-opus-5-5`로 맞춘다(인스턴스 이름은 기기마다 `claudeAgent`·`claudeAgent_claude2` 등으로 달라질 수 있어 고정하지 않는다). Codex(`codex`, gpt 계열)는 사용하지 않는다.

이 결정과 `docs/workflow.md`의 H-60 규칙은 [ADR 0021](0021-codex-compatible-harness.md)과 [ADR 0125](0125-implementer-models.md)(H-50)의 제공자·모델·옵션·리뷰 계정 규칙을 대체한다. 작업 사본 격리, 기준 SHA·조상 검증, 소유 스레드 제한, 구현자 보고서 격리 규칙은 그대로 유지한다. 이미 병합된 리뷰 기록과 과거 ADR의 모델 표기는 고치지 않는다(단, `docs/adr/0021-codex-compatible-harness.md` 109행의 `known-gaps:` 총계 줄만 H-51 선례에 따라 새 총계로 맞춘다).

## 결정

### 역할 표 (전 → 후)

| 역할 | 전 (H-50) | 후 (H-60) |
|---|---|---|
| 메인 | Claude (오케스트레이션·`docs/**`) | Claude (오케스트레이션·`docs/**`, 유지) |
| 구현·수정 | T3 Codex `gpt-6.1-sol`(`high`) 또는 `gpt-6.1-terra`(`max`) | T3 Antigravity `gemini-4-neon`(`thought_level=xhigh`) |
| 조사·비판 | T3 Codex `gpt-6.1-sol`(`high`) | T3 Antigravity `gemini-4-neon`(`thought_level=high`) |
| 적대적 리뷰 | T3 Claude(`claudeAgent`) `claude-opus-5-5` | T3 Claude(`driverKind=claudeAgent`, `claude-opus-5-5`, 기기별 인스턴스 이름 허용) |

### 위임 가드 계약 (`.claude/hooks/lib/orchestration.mjs`)

허용 목록을 넓히지 않고 닫힌 집합으로 고정한다.

- `t3_thread_launch`: `modelSelection.provider === "antigravity"`, `instanceId`는 생략 또는 `"antigravity"`, `model === "gemini-4-neon"`, `options`는 정확히 `{ thought_level: "xhigh" }` 단일 키 일반 객체(생략·`{}`·`[]`·`null`·다른 키·혼합 키 거부). 시작 메시지에는 정확히 한 개의 `- 역할: implementation` 줄이 줄 시작에 있어야 한다.
- `delegate_task` (Antigravity): `target.driverKind === "antigravity"`, `providerInstanceId`는 생략 또는 `"antigravity"`, `model === "gemini-4-neon"`, `isolation === "worktree"`. `role`은 반드시 `"implementation"`, `"research"`, `"review"` 중 하나여야 하며 역할 생략은 거부한다(`guard-orchestration.mjs`가 역할 생략을 `implementation`으로 기록해 리뷰 스레드 재개로 이어지는 우회를 막는다). `options`는 `implementation`일 때 `{ thought_level: "xhigh" }`, `research`·`review`(비판)일 때 `{ thought_level: "high" }`만 허용한다.
- `delegate_task` (Claude 적대적 리뷰): `target.driverKind === "claudeAgent"`, `model === "claude-opus-5-5"`, `role === "review"`, `isolation === "worktree"`. `providerInstanceId`는 기기별 차이를 수용해 생략 또는 `claudeAgent` 접두 형식(`claudeAgent`, `claudeAgent_claude2` 등 `/^claudeAgent(?:$|[._-][A-Za-z0-9._-]+)$/u`)을 허용하고 빈 문자열·`null`·비문자열·`other`·타 드라이버 이름(`codex`, `antigravity`, `myCodex`, `gpt` 등)은 거부한다. `options`는 생략 또는 `{ effort: "high" }` 단일 키 일반 객체만 허용한다(`{}`·`[]`·`null`·`effort=max`·`thought_level`·`reasoningEffort`·`ultracode`·`fastMode` 등 거부).
- 거부 대상: 모든 Codex 호출(`driverKind=codex`·`provider=codex`, `gpt-6.1-sol`·`gpt-6.1-terra`·`gpt-6.2-sol` 등), 잘못된 `providerInstanceId`(빈 문자열·`null`·타 드라이버 이름), 목록 밖 모델(`gemini-4-carbon`·`gemini-4-argon`·`gemini-4-hydrogen`·`gemini-3.8-flash`·`claude-sonnet-5-5` 등), 목록 밖 제공자·역할·옵션 키·권한 확대.
- Codex 전용 절차(프로젝트·훅 신뢰, `CODEX_HOME`, Codex 종료 훅, `just codex-regression`, `harness/codex-hooks/` 어댑터)는 삭제하지 않고 문서에서 "Codex를 다시 쓸 때만 해당"으로 구획해 되돌릴 수 있게 보존한다.

## 집행 경계 (강제되는 것 vs 강제되지 않는 것)

| 구분 | 항목 |
|---|---|
| 여전히 기계로 강제 | 로컬 Git 커밋 훅(`lefthook`: `fresh-build`, `commit-guard`, `just check-fast`, 하네스·문서·설정 변경 시 전체 `just check` 승격, `test-diff`, `disable-check`, `hooks-installed`), 로컬 푸시 훅(`just check-full`), 원격 GitHub Actions CI(`ci.yml` Linux/macOS 전체 `just check`·변경분 변이·시각 회귀, `signatures.yml` 보호 경로 서명 사후 탐지, `nightly.yml`, `review-records`), 메인 Claude의 `orchestrationFindings` 위임 가드, 독립 적대적 리뷰·비판과 메인의 대상 스냅숏 전후 비교 |
| Antigravity 세션에서 기계 미강제 | 세션 중 보호 경로 쓰기 차단, 세션 중 홈 파일·자격 증명·하네스 상태 읽기 차단, 세션 중 금지 셸 명령 실행 시점 차단, 편집 직후 자동 `just lint-file`, Antigravity 리뷰·비판 역할의 추적 파일 쓰기·중첩 위임 차단, 세션 종료 시 자동 전체 검사(`Stop`/`SubagentStop`) |

## Antigravity 세션에서 확인한 사실과 미확인 항목

- 확인한 사실:
  1. 현재 Antigravity 세션은 저장소 루트의 `AGENTS.md`를 규칙으로 자동 주입한다.
  2. 이 세션에 번들된 내장 스킬(`agy-customizations`의 `SKILL.md`·`docs/hooks.md`·`docs/rules.md`, `antigravity-guide`의 `references/app.md`·`references/cli.md`)은 작업 공간 `.agents/hooks.json`(또는 `.agent/`·`_agents/`·`_agent/`)의 생명주기 훅(`SessionStart`, `PreToolUse`, `PostToolUse`, `PreInvocation`, `PostInvocation`, `Stop`)과 `PreToolUse`의 `decision: "allow" | "deny" | "ask" | "force_ask"`, `Stop`의 `decision: "continue"` 계약, 전역 설정 위치(`~/.gemini/config/`, CLI `~/.gemini/antigravity-cli/settings.json`), 앱의 터미널 샌드박스·비작업공간 파일 접근 정책을 설명한다.
  3. H-60 작업 시작 시점의 저장소 루트에는 `.agents/`·`.agent/`·`_agents/`·`_agent/`·`GEMINI.md`가 없다.
  4. T3 `orchestrator_capabilities`는 `antigravity` 제공자의 `gemini-4-neon`(`thought_level` 값 `low`·`medium`·`high`·`xhigh`, 기본 `xhigh`)과 `claudeAgent_claude2` 제공자의 `claude-opus-5-5`를 보고한다.
- 미확인 항목 (H-60 범위에서 추측하지 않음. 이어지는 H-61에서 실측):
  1. T3 Code의 `antigravity-acp` 통합 경로(`runtimeMode: "full-access"` 포함)에서 `.agents/hooks.json`이 실제로 로드·실행돼 도구 호출을 차단하고 종료를 막는지.
  2. worktree 세션에서 훅 정의의 출처(주 작업 트리 대 worktree)와 폴더 신뢰 요구 여부.
  3. 실제 `PreToolUse` 도구 이름·인자 키와 훅 오류·시간 초과 시 닫힌 실패 여부.
  4. Antigravity 터미널 샌드박스와 비작업공간 파일 접근 정책이 프로젝트 파일로 강제 가능한지.
  5. `PostToolUse`가 `just lint-file` 실패 피드백을 모델에 돌려줄 수 있는지.
  6. 사용자 홈 설정(`~/.gemini/...`)의 실제 병합 동작(홈 파일 열람 금지 규칙에 따라 읽지 않음).

## H-60 시점 빈틈 (H-61에서 해소)

아래 항목은 H-60 단독 시점의 미강제 목록이며, 같은 브랜치의 H-61(`.agents/hooks.json`, `harness/antigravity-hooks/`)에서 해소되었으므로 `just known-gaps`에는 H-61의 잔여 빈틈만 집계한다.

- Antigravity 세션에는 Codex·Claude 세션 훅이 적용되지 않아 세션 중 보호 경로 쓰기 차단(`PreToolUse` 편집 가드·권한 `deny`·샌드박스 `denyWrite`)이 기계로 강제되지 않는다. Git 커밋 훅·원격 CI 서명 검사·독립 리뷰가 사후 탐지한다.
- Antigravity 세션에서는 홈 파일·자격 증명·하네스 상태 폴더 읽기 차단(샌드박스 `denyRead`, Claude `Read` 거부)이 기계로 강제되지 않는다. 규칙 준수와 사후 리뷰에 의존한다.
- Antigravity 세션에서는 금지 셸 명령 실행 시점 차단(`PreToolUse` Bash 가드)과 편집 직후 자동 린트(`PostToolUse` `just lint-file`)가 기계로 강제되지 않는다. 커밋 훅(`lefthook`)과 원격 GitHub Actions CI가 커밋·병합 이후를 검증한다.
- Antigravity 세션의 리뷰·비판 역할에서는 추적 파일 쓰기와 중첩 위임이 세션 훅으로 차단되지 않는다. T3 worktree 격리와 메인의 대상 작업 사본 전후 스냅숏 비교로 오염을 확인한다.
- Antigravity 세션에서는 종료 시 전체 검사(`Stop`·`SubagentStop` 훅)가 기계로 강제되지 않는다. 구현자가 `just check-fast`를 직접 실행하고 원격 GitHub Actions CI가 전체 `just check`와 변경분 변이를 수행한다.

## 인계

- H-61: 번들 문서에서 확인한 Antigravity `.agents/hooks.json`의 실제 동작을 T3 `antigravity-acp` 세션에서 실측하고, 기존 판정 코어를 재사용하는 Antigravity 훅 어댑터를 구현해 위 빈틈을 메운다(메인 추가 지시로 같은 스레드에서 이어서 수행).
- 메인: 병합 시 ADR 번호 확정, `docs/progress.md`의 운영 메모 및 완료 행 갱신, 원격 CI 전체 검사 확인.

## 손 변형

`target/H-60/mut-sandbox` 격리 복사본에서 `.claude/hooks/lib/orchestration.mjs`의 30개 변형을 `node --check`로 문법 검증한 뒤 실제 Node 테스트(`implementer-models.test.mjs`, `orchestration.test.mjs`, `t3-r1.test.mjs`)로 실행했고, 실행 직후 복사본을 삭제했다. 첫 실행에서는 28/30 검출(2개 생존: 배열 속성 전달과 프로토타입 상속 옵션 키)이 나와 회귀 단언을 추가한 뒤 재실행해 **30/30 검출, 생존 0건**을 확인했다.

| 번호 | 바꾼 것 | 결과와 잡은 테스트 |
|---|---|---|
| 1 | `plainObject`에서 `!Array.isArray` 제거 | 검출: `Antigravity requires an explicit thought_level option and rejects omission or empty values` |
| 2 | `keys`에서 허용 키 검사 제거 | 검출: `R8 T3 launch rejects cross project environment elevated mode arbitrary keys and options` |
| 3 | `exactKey`에서 키 개수 1 검사 제거 | 검출: `Claude2 Opus review requires claudeAgent_claude2 and accepts only omission or effort high` |
| 4 | `exactKey`에서 `Object.hasOwn` 제거 | 검출: `Antigravity requires an explicit thought_level option and rejects omission or empty values` |
| 5 | 구현 `thought_level` `xhigh`를 `high`로 변경 | 검출: `delegate exact model gemini-4-neon role implementation` |
| 6 | 조사·비판 `thought_level` `high`를 `xhigh`로 변경 | 검출: `delegate exact model gemini-4-neon role research` |
| 7 | `research` 역할 허용 제거 | 검출: `delegate exact model gemini-4-neon role research` |
| 8 | `review` 역할 허용 제거 | 검출: `delegate exact model gemini-4-neon role review` |
| 9 | 미지 역할의 기본 `thought_level`을 `high`로 완화 | 검출: `delegate exact model gemini-4-neon role undefined` |
| 10 | `antigravityOptions` 값 일치 검사 제거 | 검출: `Antigravity thought_level implementation low` |
| 11 | `claudeReviewOptions` `effort=high`를 `max`로 변경 | 검출: `Claude2 Opus review requires claudeAgent_claude2 and accepts only omission or effort high` |
| 12 | `claudeReviewOptions` 옵션 생략 허용 제거 | 검출: `Claude2 Opus review requires claudeAgent_claude2 and accepts only omission or effort high` |
| 13 | `claudeReviewOptions` `exactKey`를 `keys`로 완화 | 검출: `Claude2 Opus review requires claudeAgent_claude2 and accepts only omission or effort high` |
| 14 | Antigravity `instanceId` 검증 제거 | 검출: `Antigravity boundaries reject non-worktree, elevated mode, wrong provider, wrong instance, and foreign option keys` |
| 15 | Antigravity `instanceId` 생략 허용 제거 | 검출: `Antigravity boundaries reject non-worktree, elevated mode, wrong provider, wrong instance, and foreign option keys` |
| 16 | Antigravity 모델 정확 일치 제거 | 검출: `delegate exact model gemini-4-carbon role implementation` |
| 17 | Antigravity 모델 접두사 허용 | 검출: `delegate exact model gemini-4-neon-extra role implementation` |
| 18 | `CLAUDE_INSTANCE_PATTERN` 허용 목록을 임의 비빈 문자열로 완화 | 검출: `Claude Opus review matches driverKind claudeAgent and model claude-opus-5-5 across instance names and accepts only omission or effort high` |
| 19 | Claude 리뷰 `providerInstanceId` 생략 허용 제거 | 검출: `Antigravity boundaries reject non-worktree, elevated mode, wrong provider, wrong instance, and foreign option keys` |
| 20 | Claude2 모델 검사 제거 | 검출: `T3 Opus review requires the exact model role provider and worktree` |
| 21 | Claude2 `review` 역할 제한 제거 | 검출: `Claude2 Opus review requires claudeAgent_claude2 and accepts only omission or effort high` |
| 22 | `launchRole` 비문자열 방어 제거 | 검출: `Antigravity launch requires one exact implementation role line` |
| 23 | `launchRole` 줄 시작 앵커 제거 | 검출: `Antigravity launch role marker must start its own line` |
| 24 | `launchRole` 중복 역할 허용 | 검출: `Antigravity launch requires one exact implementation role line` |
| 25 | `delegate` Codex 명시 거부 제거 | 검출: `Codex provider and models are rejected for both delegate and launch` |
| 26 | `launch` Codex 명시 거부 제거 | 검출: `Codex provider and models are rejected for both delegate and launch` |
| 27 | `launch` `instanceId` 검증 제거 | 검출: `Antigravity boundaries reject non-worktree, elevated mode, wrong provider, wrong instance, and foreign option keys` |
| 28 | `launch` 역할 `implementation` 검사 제거 | 검출: `Antigravity launch requires one exact implementation role line` |
| 29 | `launch` 모델 검사 제거 | 검출: `launch exact model gemini-4-carbon role implementation` |
| 30 | `launch` `thought_level` 옵션 검사 제거 | 검출: `Antigravity thought_level implementation low` |
