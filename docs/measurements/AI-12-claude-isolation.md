# AI-12 실측 기록: 실제 `claude` 격리 확인 (S-04, SEC-2)

- 작업: [tasks.md](../tasks.md) AI-12, 실측 [open-issues.md](../open-issues.md) S-04
- 기록일: 2026-11-25 (fin-measure 묶음)
- 상태: **에이전트 실행 막힘, 사용자 실행 대기**

## 무엇을 확인하나

앱의 수집 실행과 같은 격리 옵션으로 `claude`를 빈 임시 폴더에서 띄웠을 때 다음이 모두 성립하는지.

| 항목 | 통과 조건 |
|---|---|
| init `skills` | `[]` |
| init `slash_commands` | `[]` |
| 사용자·프로젝트 훅 | `hook_*` 이벤트 0건, 작업 폴더의 훅 표식 파일이 생기지 않음 |
| 사용자 메모리·전역 CLAUDE.md | 모델이 "NONE"이라고 답함(전역 지침 첫 줄을 인용하지 않음) |
| 프로젝트 CLAUDE.md | 카나리 `CANARY-PROJECT-4417`이 답에 없음 |
| 구독 인증 | `apiKeySource`가 `none`이고 결과가 오류가 아님 |
| `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1` | 위 조건을 지키면서 실행이 끝까지 감 |

앱이 쓰는 옵션은 `crates/adapters/ai-claude-code/src/command.rs`의 `build_profile_args`다. 수집 실행은 `-p --verbose --input-format stream-json --output-format stream-json --include-partial-messages --restricted --disable-slash-commands --tools WebSearch --allowedTools WebSearch --permission-mode dontAsk --permission-prompts none --mcp-config <빈 mcp.json> --strict-mcp-config --no-session-persistence --model <설정>`이고, 환경은 `HOME PATH USER LOGNAME TMPDIR`와 `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1`만 넘긴다.

## 이번 묶음에서 한 것

- 사용자 지시로 이 묶음에 한해 `claude` 실행이 허용됐지만, 세션 훅이 `claude` 명령을 거부했다: `[denied-commands] claude는 하네스가 막는다 (docs/harness.md 7.3, ADR 0017)`. 훅이 "다른 방법으로 우회하지 말라"고 하므로 스크립트로 감싸 실행하지 않았다.
- 대신 사용자가 그대로 돌리면 되는 점검 스크립트 [claude-isolation-check.sh](claude-isolation-check.sh)를 만들었다. 빈 임시 폴더에 카나리 `CLAUDE.md`와 프로젝트 훅(표식 파일 생성)을 두고, 앱과 같은 옵션·환경으로 한 턴을 돌린 뒤 init 이벤트와 결과를 요약한다. 사용자 홈의 설정 파일은 읽지도 고치지도 않는다.

## 사용자가 할 일

```sh
sh docs/measurements/claude-isolation-check.sh app-collection
```

출력의 `init.skills`, `init.slash_commands`, `hook_events`, `project_hook_file_created`, `init.apiKeySource`, `result.text`를 아래 표에 옮긴다. 하나라도 통과 조건을 어기면 `command.rs`의 격리 옵션을 고치는 AI 작업을 새로 만든다(AI 경로이므로 작게, 적대적 리뷰 포함).

| 날짜 | `claude` 버전 | skills | slash_commands | 훅 이벤트 | 훅 표식 | 메모리·전역 지침 | apiKeySource | 판정 |
|---|---|---|---|---|---|---|---|---|
| (사용자 실행 후) | | | | | | | | |

## 참고

- 2026-11 R-02 실측: `--restricted --disable-slash-commands`에서 `skills=[]`, 훅 미실행, 프로젝트 CLAUDE.md 미로드, 구독 인증 유지([R-02](../research/R-02-claude-code-runtime.md) 3장). 전역 `~/.claude/CLAUDE.md`와 auto-memory 차단은 당시 미확인이었고 이번 점검의 핵심이다.
- 하네스가 에이전트의 `claude` 실행을 막는 것은 의도된 동작이다. 에이전트가 이 실측을 하려면 사용자가 훅 정책을 바꾸는 서명 커밋이 먼저 필요하다.
