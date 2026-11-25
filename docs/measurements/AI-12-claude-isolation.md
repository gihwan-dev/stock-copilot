# AI-12 실측 기록: 실제 `claude` 격리 확인 (S-04, SEC-2)

- 작업: [tasks.md](../tasks.md) AI-12, 실측 [open-issues.md](../open-issues.md) S-04
- 기록일: 2026-11-25 (fin-measure 묶음), 결과 기록 2026-11-25 (fin-aiiso 묶음)
- 상태: **실측 완료. 앱 수집 실행 격리 통과, 계정 문맥 주입은 남은 위험으로 기록**

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

## 점검 방법 (Claude Code 버전을 올릴 때 다시 돌린다)

```sh
sh docs/measurements/claude-isolation-check.sh app-collection
```

출력의 `init.skills`, `init.slash_commands`, `hook_events`, `project_hook_file_created`, `init.apiKeySource`, `result.text`를 아래 표에 옮긴다. 하나라도 통과 조건을 어기면 `command.rs`의 격리 옵션을 고치는 AI 작업을 새로 만든다(AI 경로이므로 작게, 적대적 리뷰 포함).

| 날짜 | `claude` 버전 | skills | slash_commands | 훅 이벤트 | 훅 표식 | 메모리·전역 지침 | apiKeySource | 판정 |
|---|---|---|---|---|---|---|---|---|
| 2026-11-25 | 2.1.332 (모델 haiku) | `[]` | `[]` | 0 | 생기지 않음 | 주입 없음. 프로젝트 카나리 없음 | `none` | 통과 (아래 남은 위험 참고) |

## 결과 (main 실행, 2026-11-25)

메인 세션이 [claude-isolation-check.sh](claude-isolation-check.sh)를 세 조합으로 돌렸다. 에이전트 세션의 `claude` 실행은 여전히 하네스가 막는다.

| 조합 | init tools | skills·slash_commands | plugins | 훅 | CLAUDE.md 카나리 | 사용자 메모리·전역 지침 | apiKeySource·permissionMode | WebFetch |
|---|---|---|---|---|---|---|---|---|
| `app-collection` (`--restricted`, WebSearch) | `[WebSearch]` | `[]`·`[]` | `cc-plugin-agents-md`, `cc-plugin-privacy`, `cc-plugin-plugin-authoring` | 이벤트 0, 표식 없음 | 없음 | 없음 | `none`·`dontAsk` | 없음 |
| `restricted-webfetch` (`--restricted`, WebSearch+WebFetch) | 위와 같음에 WebFetch | `[]`·`[]` | 위와 같음 | 이벤트 0, 표식 없음 | 없음 | 없음 | `none`·`dontAsk` | 동작함 (example.com 제목 "Example Domain") |
| `sources-none-webfetch` (`--setting-sources=` + `disableAllHooks`) | WebSearch+WebFetch | - | 위와 같음 | 없음 | 없음 | **사용자 메모리 지침이 주입됨** | - | 동작함 |

판정:

- `app-collection`은 위 통과 조건을 모두 지킨다. S-04의 "`--restricted`가 사용자 메모리·전역 CLAUDE.md를 막는가"는 막는다로 닫는다.
- `plugins`의 셋은 모두 Claude Code 내장 `cc-plugin-*`다. R-02 실측(빈 설정 폴더에서도 같은 내장 4개가 나옴)과 같고, 사용자가 설치한 플러그인은 아니다. 앱의 사전 점검은 init `plugins`가 내장 허용 목록(`apps/desktop/src-tauri/src/main.rs`의 `ALLOWED_BUILTIN_PLUGINS`) 밖이거나 `plugin_errors`가 있으면 요청 전에 끝낸다(`isolation.rs`). 따라서 격리 구멍으로 보지 않고 실행 옵션은 바꾸지 않는다. 판단은 [fin-aiiso 결정](../adr/fin-aiiso-decisions.md)에 적었다.
- `sources-none-webfetch`는 사용자 메모리를 막지 못한다. `--restricted`를 빼는 조합은 쓰지 않는다([ADR 0178](../adr/0178-collection-web-reading.md)).

## 남은 위험: 계정 문맥 주입

세 조합 모두에서 모델이 "로그인 계정 정보(이메일)가 시스템 알림 문맥으로 들어왔다"고 답했다. 구독 로그인에서 Claude Code가 넣는 문맥이고, `--restricted`로 막히지 않는다. 이를 끄는 옵션은 이번에 확인하지 못했다(에이전트 세션은 `claude --help`도 하네스가 막는다). 판단:

- 수집 실행이 아는 것은 종목명, 종목 코드, 주제어와 이 계정 식별자뿐이다. 보유 수량·금액·계좌는 모른다(design 4.4).
- 수집 실행의 도구는 WebSearch 하나다. 임의 주소로 요청을 보낼 도구가 없어서, 웹 결과 속 주입 문장이 계정 식별자를 바깥 서버로 보내게 할 길이 검색어뿐이다. 검색은 같은 구독 제공자의 서버가 실행한다.
- 그래서 지금 위험은 받아들이고, 수집 실행에 임의 주소를 읽는 도구(WebFetch)를 주지 않는 이유 하나로 더한다([ADR 0178](../adr/0178-collection-web-reading.md)).
- Claude Code 버전을 올릴 때 계정 문맥을 끄는 옵션이 생겼는지 확인하고, 생기면 `build_profile_args`에 넣는다.

## 참고

- 2026-11 R-02 실측: `--restricted --disable-slash-commands`에서 `skills=[]`, 훅 미실행, 프로젝트 CLAUDE.md 미로드, 구독 인증 유지([R-02](../research/R-02-claude-code-runtime.md) 3장). 전역 `~/.claude/CLAUDE.md`와 auto-memory 차단은 당시 미확인이었고 이번 점검의 핵심이다.
- 하네스가 에이전트의 `claude` 실행을 막는 것은 의도된 동작이다. 에이전트가 이 실측을 하려면 사용자가 훅 정책을 바꾸는 서명 커밋이 먼저 필요하다.
