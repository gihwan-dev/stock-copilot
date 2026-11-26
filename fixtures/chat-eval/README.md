# 채팅 eval

앱 채팅이 실제 투자 자문가처럼 답하는지(결론 → 근거 → 다음 행동 → 지켜볼 것) 재는 도구다. 모든 계좌·종목 수치는 합성 자료다.

## 구성

| 파일 | 내용 |
|---|---|
| `scenarios.json` | 시나리오 22개. 합성 계좌, 투자 원칙, 시세·뉴스·투자 논리, 기대 조건(`expect`) |
| `portfolio-model.mjs` | 합성 계좌를 실제 MCP 도구 응답 모양(`get_portfolio`, `get_allocation` 등)으로 펼친다 |
| `fake-mcp-server.mjs` | `fixtures/mcp/tools_list.json`의 실제 도구 목록을 그대로 내놓는 stdio MCP 서버 |
| `run.mjs` | 앱과 같은 CLI 인자로 답을 받고, 결정적 검사와 LLM 판정을 붙여 보고서를 쓴다 |
| `samples/` | 결정적 검사가 걸러야 하는 나쁜 예와 통과해야 하는 좋은 예 |

변형은 `baseline`(시스템 프롬프트 없음, 이전 앱과 같음)과 `candidate`(`crates/adapters/ai-claude-code/src/chat_prompt.rs`의 `CHAT_SYSTEM_PROMPT`), `file`(`--prompt-file`로 준 파일)이다.

## 실행

모델을 부르므로 `just check`에 넣지 않는다. 에이전트 세션 안에서는 하네스가 `claude` 실행을 막으므로 사용자가 터미널에서 직접 돌린다.

```sh
node fixtures/chat-eval/run.mjs --self-test
CLAUDE_CONFIG_DIR="$HOME/.claude-second" node fixtures/chat-eval/run.mjs
CLAUDE_CONFIG_DIR="$HOME/.claude-second" node fixtures/chat-eval/run.mjs --only index-etf-heavy-cash,order-request --variants candidate
CLAUDE_CONFIG_DIR="$HOME/.claude-second" node fixtures/chat-eval/run.mjs --variants candidate,file --prompt-file /tmp/try.md
```

| 옵션 | 기본값 |
|---|---|
| `--model` | `claude-opus-5-5` |
| `--effort` | 없음(CLI 기본) |
| `--judge-model`, `--judge-effort` | `claude-opus-5-5`, `medium` |
| `--variants` | `baseline,candidate` |
| `--repeat` | 1 |
| `--concurrency` | 3 |
| `--out` | `$TMPDIR/stock-copilot-chat-eval` |
| `--no-judge` | 결정적 검사만 |
| `--fresh` | 캐시 무시 |

답과 판정은 입력(시나리오, 프롬프트, 모델) 해시로 `--out/cache`에 저장한다. 같은 입력을 다시 돌리면 모델을 부르지 않는다. 기본 실행은 답 44회와 판정 44회다. `CLAUDE_BIN`으로 CLI 경로를 바꿀 수 있다.

## 채점

결정적 검사: 내부 용어(`unclassified`, `buying_power`, 필드명, 도구명), 규칙 낭독("앱 규칙", "규칙상"), 주문 실행 주장, 결론 먼저, 길이, 행동 표, 필수 도구 호출, 시나리오별 포함·금지 표현.

LLM 판정(1~5): `actionable`, `grounded`, `instrument_judgment`, `plain_language`, `brevity`, `korean`, `missing_data`, `principles`, `safety`. 판정자는 시나리오의 합성 자료와 좋은 답의 조건을 함께 받는다.

결과는 `--out/report.md`와 `--out/results.json`에 남는다.
