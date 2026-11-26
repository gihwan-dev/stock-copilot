# 채팅 eval

앱 채팅이 실제 투자 자문가처럼 답하는지(결론 → 근거 → 다음 행동 → 지켜볼 것) 재는 도구다. 모든 계좌·종목 수치는 합성 자료다.

## 구성

| 파일 | 내용 |
|---|---|
| `scenarios.json` | 시나리오 29개. 합성 계좌, 투자 성향·원칙, 시세·뉴스·투자 논리, 앱 신호와 주문 제안, 기대 조건(`expect`) |
| `portfolio-model.mjs` | 합성 계좌를 실제 도구 응답 모양으로 펼친다. 앱 배분 조회의 주의·초과는 앱과 같은 상품 종류별 한도로 계산한다 |
| `fake-mcp-server.mjs` | 분석 도구(`fixtures/mcp/tools_list.json`)와 앱 명령 도구(`fixtures/mcp/app_tools_list.json`) 60개를 실제 목록 그대로 내놓는 stdio MCP 서버. 쓰기·승인 요청은 인자와 함께 기록한다 |
| `run.mjs` | 앱과 같은 CLI 인자로 답을 받고, 결정적 검사와 LLM 판정을 붙여 보고서를 쓴다 |
| `samples/` | 결정적 검사가 걸러야 하는 나쁜 예와 통과해야 하는 좋은 예 |

변형은 `baseline`(시스템 프롬프트 없음, 이전 앱과 같음)과 `candidate`(`crates/adapters/ai-claude-code/src/chat_prompt.rs`의 `CHAT_SYSTEM_PROMPT`), `file`(`--prompt-file`로 준 파일)이다.

## 실행

모델을 부르므로 `just check`에 넣지 않는다. 에이전트 세션 안에서는 하네스가 `claude` 실행을 막으므로 사용자가 터미널에서 직접 돌린다.

```sh
just chat-eval --self-test
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

답과 판정은 입력(시나리오, 프롬프트, 모델) 해시로 `--out/cache`에 저장한다. 같은 입력을 다시 돌리면 모델을 부르지 않는다. 기본 실행은 답 58회와 판정 58회다. 고정 자료 파일이 바뀌면 캐시 키도 바뀐다. `CLAUDE_BIN`으로 CLI 경로를 바꿀 수 있다.

## 채점

결정적 검사: 내부 용어(`unclassified`, `buying_power`, 필드명, 도구명), 규칙 낭독("앱 규칙", "규칙상"), 주문 실행 주장, 결론 먼저, 길이, 행동 표, 필수 도구 호출(분석·앱 도구 어느 쪽이든), 기대한 쓰기 호출과 인자(`expect.calls`), 요청 없는 쓰기 금지, 앱에 없는 주문 제안 ID 금지, 시나리오별 포함·금지 표현.

LLM 판정(1~5): `actionable`, `grounded`, `instrument_judgment`, `plain_language`, `brevity`, `korean`, `missing_data`, `principles`, `safety`. 판정자는 모델이 실제로 부른 도구와 받은 결과, 앱의 상품 종류별 한도 기준, 좋은 답의 조건을 함께 받는다.

결과는 `--out/report.md`와 `--out/results.json`에 남는다.
