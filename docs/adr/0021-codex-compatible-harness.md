# ADR 0021: Codex 호환 하네스와 역할 배분

- 상태: 제안
- 날짜: 2026-11-13
- 관련 작업: H-41
- 기준 커밋: 7ce72918d95d6d4d9ffce082fa361066b2b310c9
- 이 ADR의 역할·Codex 운영 규칙이 ADR 0017~0020의 Claude 전용 운영 규칙보다 앞선다.

## 실측

홈의 설정과 실제 자격 증명 파일은 읽거나 바꾸지 않았다. 모델을 쓰는 실측은 임시 저장소에서 했다.

| 항목 | 명령·방법 | 출력·관찰 |
|---|---|---|
| CLI | `codex --version` | `codex-cli 0.168.0` |
| 기능 | `codex features list` | `hooks stable true`, `shell_snapshot stable true`, `multi_agent_v2 stable false` |
| 도움말 | `codex --help`, `exec --help`, `sandbox --help`, `debug --help`, `app-server --help` | `exec`, `--ephemeral`, `--ignore-user-config`, `--json`, `--strict-config`, 훅 신뢰 옵션과 권한 프로필 지원 |
| 설치 문서 | 설치된 `@openai/codex/README.md`, `package.json` | npm 진입점은 native 바이너리를 호출한다. 패키지에 설정 스키마는 없다 |
| 로컬 스키마 | `codex app-server generate-json-schema --experimental --out <임시 폴더>` | ConfigRead, HooksList, ThreadStart, HookStarted·Completed 스키마 생성 |
| 샌드박스·승인 선택 | `codex --help`, `exec --help` | read-only·workspace-write·danger-full-access, on-request·never 지원. 프로젝트 프로필은 명시적 sandbox 선택보다 우선하지 않음 |
| AGENTS 읽기 | 공식 AGENTS 문서 | 시작 때 전역 override 또는 AGENTS, 프로젝트 루트부터 현재 폴더까지 각 폴더의 override·AGENTS·fallback 순서. 기본 32 KiB, 가까운 폴더가 뒤에 붙음 |
| 역할 기능 | features와 헤드리스 경고 | 내부 서브에이전트 기능 존재. 사용자 역할 정의 중 name 필드 없는 항목은 무시된다는 경고를 관찰. 이번 구현은 내부 역할 정의를 사용하지 않음 |
| 실행 주체 | `ps`의 실행 파일·부모만 확인 | Codex native 바이너리의 부모가 T3 Code다. 명령 인자에서 app-server 확인 |
| 현재 T3 세션 | 임시 `.codex/hooks.json`으로 이후 셸 호출 관찰 | 이벤트 기록 없음. 실행 도중 추가한 프로젝트 훅은 이 세션에서 관찰되지 않았다. 시작 당시의 로드 여부까지 증명하는 실험은 아니다 |
| 권한 | 현재 세션의 제공된 실행 정책 | danger-full-access, 승인 never. 프로젝트의 새 프로필이 현재 세션을 바꾸지 않는다 |
| 헤드리스 1 | 프로젝트 신뢰를 dotted `-c`로 전달 | 키가 무시됐다는 경고. 프로젝트 설정·훅 미적용. pnpm은 옛 shim으로 실패 |
| 헤드리스 2 | `-c 'projects={ "/임시/저장소"={trust_level="trusted"} }'`, 검토한 임시 훅에만 `--dangerously-bypass-hook-trust` | SessionStart, Bash 전후, apply_patch 전후, Stop 입력을 기록. Bash·패치 입력은 `tool_input.command` |
| 셸 환경 | 프로젝트 `shell_environment_policy.set.PATH`와 표식 | `project-loaded`, 독립 설치본 pnpm 경로, `12.18.0` |
| 회귀 | 임시 git 원본과 작업 사본, 10개 대표 시도 | 첫 실행은 작업 사본 설정 대신 원본 설정이 사용돼 기록기 미적용. 둘째는 두 사본에 같은 실험 훅을 두어 10개 모두 관찰. Update File도 시도로 판정하도록 녹화 판정을 수정 |
| 설정 검증 | 신뢰를 준 `codex --strict-config ... app-server --print-startup-config` | 종료 0 |
| 샌드박스 | `codex ... sandbox -P stock-copilot` | workspace 파일 쓰기 성공, example.com HTTPS는 `403`, `x-proxy-error: blocked-by-allowlist` |
| 시작 검사 | pnpm 설치 → lefthook 설치 → `just base=7ce72918d95d6d4d9ffce082fa361066b2b310c9 check` | 종료 0. Node 306+69+65개, Nextest 239개 통과 |

설정 검색과 신뢰는 [공식 설정 문서](https://learn.chatgpt.com/docs/config-file/config-advanced#hooks), 이벤트와 입출력은 [공식 Hooks 문서](https://learn.chatgpt.com/docs/hooks), 권한 프로필은 [공식 Permissions 문서](https://learn.chatgpt.com/docs/permissions), 지침 검색은 [공식 AGENTS 문서](https://learn.chatgpt.com/docs/agent-configuration/agents-md#how-codex-discovers-guidance)를 확인했다. 공식 문서는 설치 버전보다 앞설 수 있으므로 실제 입력과 종료 코드로 교차 확인한다.

## 결정

| 결정 | 이유 |
|---|---|
| 구현·수정·조사·비판은 새 Codex gpt-6.1-sol, 적대적 리뷰는 새 Claude Opus 5.5 | 사용자 배분. 메인 Claude는 오케스트레이션과 docs 편집 |
| Codex는 T3의 별도 git worktree에서 시작 | 원본 주 작업 트리와 분리. 역할 이름 대신 git 등록 경로로 메인·구현 담당 구별 |
| 판정 코어를 옮기거나 복제하지 않는다 | `.claude/hooks/lib`의 기존 편집·셸·경로·결과 코어를 Codex가 가져온다. 기존 원소 빼기 테스트와 공통 예제 비교를 함께 실행 |
| Claude에 새 오케스트레이션 진입 가드를 둔다 | 내부 implementer·review-critic 호출을 거부. T3 delegate는 codex·gpt-6.1-sol·worktree·기준 브리프를 확인. SendMessage 재개는 계속 거부 |
| apply_patch 전체 대상을 편집 전에 검사 | 추가·수정·삭제·이동의 전후 대상 모두 검사. 절대 경로의 비정규 조각을 정규화해 숨기지 않는다 |
| `.codex/**`와 새 하네스는 보호 대상 | 허용 목록 밖, 해시 고정, 잠금 조각, config.toml·hooks.json 위치 검사로 보호 |
| 권한 프로필은 기존 sandbox 정책에서 생성 | 캐시 쓰기, 홈 자격 증명 읽기 거부, 패키지 도메인 허용을 공유. 지원하지 못하는 경로는 아래에 명시 |
| 로그인 셸과 셸 스냅숏을 끄고 PATH를 생성 | 사용자 로그인 셸이 PATH를 덮는 차이 제거. 독립 pnpm·실행 중 Node·cargo·Homebrew·시스템 순서. 사용자 홈 설정은 바꾸지 않는다 |
| `just check-pnpm`은 실제 `pnpm --version`을 packageManager와 비교 | Node 버전 확인만으로는 오래된 Corepack shim을 찾지 못한다. 실패하면 PATH 수정 방법을 출력 |
| Stop·편집 뒤 린트는 기존 검사 샌드박스 재사용 | 훅 프로세스가 빌드·테스트를 샌드박스 밖에서 실행하지 않게 한다. Stop은 offline fresh 단계 뒤 전체 검사 |
| 구현 지침 원본은 docs/agents/implementer.md | Claude 안내 파일은 원본을 링크. 내용 이중 관리 방지 |

## 강제 수준

| 항목 | Claude | Codex |
|---|---|---|
| 보호 경로 | 잠금 off 경고, on 거부 | 같은 판정. 권한 프로필의 `.codex` 읽기 전용은 off에도 적용 |
| 금지 셸 | 같은 코어 | 같은 코어 |
| 편집 뒤 검사 | Edit·Write 후 lint-file | apply_patch 후 lint-file, 이미 끝난 편집을 되돌리지는 않음 |
| 설정 변경 | ConfigChange | 전용 이벤트 없음 |
| 종료 | SubagentStop + 상태 기록 | Stop + 전체 검사. T3 결과에 Claude 상태 기록을 붙일 수 없음 |
| 훅 실행 실패 | onFailure block, Stop은 예외 | 프로세스 실행 실패·시간 초과를 강제 차단하지 못함 |
| 샌드박스 | H-20 전 비활성 | 프로필이 선택된 새 세션·sandbox 명령에서 적용. 현재 T3 세션에는 미적용 |

## 알려진 빈틈: Codex에서 강제되지 않는 것

- 현재 T3 세션에서 새 프로젝트 훅이 실행되는 것은 관찰하지 못했다. 사용자가 신뢰한 뒤 새 세션을 시작하고 실제 차단 시도를 확인해야 한다. 현재 세션에 적용됐다고 보고하지 않는다.
- 프로젝트 신뢰와 훅 정의 해시 신뢰는 별개다. 신뢰하지 않은 훅은 건너뛴다. 변경 뒤에도 다시 신뢰해야 한다. 프로젝트 파일만으로 사용자 신뢰를 강제할 수 없다.
- T3가 sandbox_mode나 turn 권한을 덮으면 프로젝트 권한 프로필보다 우선한다. 특히 danger-full-access는 홈 읽기·네트워크 제한을 제거한다. T3 실행 설정을 사용자가 확인해야 한다.
- ConfigChange 이벤트가 없다. 설정 파일의 세션 반영 거부를 Claude와 같은 방식으로 구현하지 못한다.
- 훅 실행 파일 누락·시작 실패·시간 초과는 차단을 보장하지 않는다. 정상 실행 중 예외는 exit 2와 거부 JSON으로 처리한다.
- hosted 도구와 일부 특수 도구는 훅을 거치지 않는다. 임의 MCP나 node REPL의 내부 파일 쓰기는 Bash·apply_patch 판정 밖이다. write_stdin은 새 PreToolUse 없이 이미 시작한 명령에 입력한다.
- 파일 편집은 apply_patch를 검사한다. 셸로 실행하는 프로그램의 간접 쓰기는 기존 Claude와 마찬가지로 문자열 판정만으로 막지 못한다. 권한 프로필과 CI를 함께 쓴다.
- 기존 sandbox 정책의 `../stock-copilot-wt`와 그 아래 글롭은 T3의 별도 현재 작업 루트로 대체한다. pnpm `_tmp_*`, `~/.claude-*/projects` 글롭은 생성 프로필에 넣지 않는다. 이 경로에 대한 같은 수준의 허용·거부는 미확인이다.
- Codex Stop의 기준은 master와 HEAD의 병합 기준이다. T3 브리프 SHA·모델·상태에 결합된 위조 방지 기록은 없다. 메인이 브리프 SHA로 다시 검사하고 실제 대상 HEAD·모델을 확인한다.
- T3가 쓰는 git worktree의 프로젝트 설정은 원본 공통 저장소의 설정을 사용할 수 있다. 실험에서 작업 사본에만 둔 기록기는 실행되지 않았다. 병합 전 작업 사본 설정만으로 운영 적용을 보장하지 않는다.
- Codex 비판자의 읽기 전용과 원본 오염 감지는 T3 실행 권한·메인 확인에 맡긴다. 현재 위임 가드는 제공자·모델·격리와 브리프만 검사한다.
- 회귀 10개는 PreToolUse의 실제 시도와 판정만 확인한다. PostToolUse 린트와 Stop 전체 검사의 생산 설정 끝까지 실행은 별도 검증이 필요하다.
- 권한 프로필의 공통 git 관리 폴더 쓰기는 추가 실측에서 차단됐다. 별도 작업 사본의 git add·commit은 index.lock, merge는 ORIG_HEAD.lock을 만들지 못해 모두 종료 128이다. 정상 커밋·병합의 샌드박스 호환성은 미해결이다. 샌드박스를 끄는 대안으로 처리하지 않는다.

## 사용자가 할 일

| 순서 | 일 |
|---|---|
| 1 | 이 작업의 변경과 기존 테스트 기대값을 검토해 메인이 커밋한다. 그 뒤 전체 검사를 다시 통과시킨다 |
| 2 | 원본 저장소에 병합한 뒤 `node harness/codex-hooks/config.mjs --write`로 현재 컴퓨터 경로를 생성하고 해시·잠금 조각을 다시 만든다. 다른 컴퓨터 경로를 그대로 쓰지 않는다 |
| 3 | 사용자가 홈의 config.toml에서 `[projects."<원본 절대 경로>"]` 아래 `trust_level = "trusted"`를 설정한다. 에이전트는 홈 설정을 고치지 않는다 |
| 4 | 원본에서 `codex`를 열고 `/hooks`로 정확한 정의를 검토·신뢰한다. 작업 사본 신뢰가 별도로 필요하면 같은 방식으로 확인한다 |
| 5 | T3의 Codex 실행 권한이 프로젝트 프로필을 덮지 않는지 확인하고 새 세션을 연다. 로그의 실제 차단을 확인한다 |
| 6 | Codex 갱신 뒤 `just codex-regression`을 수동 실행한다. 미확정·실패가 하나라도 있으면 완료로 보지 않는다 |

사용자 셸에서 도구 확인:

```sh
export PATH="$HOME/Library/pnpm:$PATH"
pnpm --version
which -a pnpm corepack node just cargo lefthook
```

프로젝트 신뢰를 부여한 임시 실험에서는 `codex exec --ignore-user-config --ephemeral --dangerously-bypass-hook-trust -c 'projects={"<임시 절대 경로>"={trust_level="trusted"}}' -m gpt-6.1-sol ...`을 썼다. 훅 신뢰 우회 옵션은 검토한 임시 실험 전용이며 일상 운영에 쓰지 않는다.

## 결과

헤드리스 실측은 총 5회다(기능 조사 2회, 회귀 실행 3회). 마지막 회귀는 10개 모두 통과했다.

기존 기대값 변경은 메인이 커밋할 변경이다: `harness/claude-hooks/tests/settings.test.mjs`(새 가드와 T3 허용), `apps/desktop/tests/check-list.test.mjs`(pnpm 버전 확인 단계와 순서), `apps/desktop/tests/known-gaps.test.mjs`(현재 빈틈 개수는 새 ADR에 기록). 하네스 잠금은 켜지 않았다. 실제 키·계좌 데이터는 다루지 않았다. 검사 끄기는 추가하지 않았다.

현재 빈틈 목록의 요약:

```
known-gaps: 63건 (eslint 7건, hook 6건, adr 50건(문서만(예제 없음)))
```

이전 회차의 전체 검사는 `review-records`에서 H-41의 `진행 중` 상태를 거부해 종료 1이다. 기존 검사는 이 상태를 명시적으로 금지한다. 완료·리뷰 결과를 만들지 않고 멈췄다. 변경 후 마지막 Rust 검사는 실행되지 않았다. 시작 기준의 전체 검사는 통과했다.

후속 수정: `check`의 선언은 기존 `check-node require-base check-ts`로 복원했다. pnpm 확인은 `check-ts`의 선행 단계로 유지한다. 기준이 없으면 pnpm 실행 전에 기준 누락으로 거부한다. Rust의 기존 기준 누락 테스트는 바꾸지 않았다. 진행 표에는 리뷰가 끝난 완료 작업만 넣고 진행 중인 작업은 한 줄 메모로 쓴다.

추가 실측은 헤드리스 0회다. 작업 공간 안에 임시 원본과 worktree를 만들고 `codex sandbox -P stock-copilot -C <worktree>`로 git add·commit·merge를 실행했다. 공통 git 폴더의 잠금 파일 쓰기가 모두 Operation not permitted로 실패했다. 임시 저장소는 지웠다. 생산 설정의 Codex 종료 훅까지의 전체 실측은 계속 미실행이다.

후속 전체 검사: `just base=7ce72918d95d6d4d9ffce082fa361066b2b310c9 check` 종료 0. Node 453개(306+69+13+65), Nextest 239개 통과. 기존 `check_refuses_to_run_without_an_explicit_base`도 통과했다. coverage-check·test-diff·hooks-installed 위반 없음, 새 검사 끄기 0건이다. 커밋하지 않고 모든 변경을 스테이징해 메인에게 넘긴다.
