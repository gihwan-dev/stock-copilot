# R-04 Rust MCP 서버와 전송 방식

- 조사일: 2026-11-11 (모든 출처의 확인 날짜도 같다)
- 로컬 환경: macOS (Darwin 25.5.0), rustc 1.88.0, Claude Code 2.1.322, Codex CLI 0.167.1
- 실험 코드: `/private/tmp/claude-501/-Users-choegihwan--t3-worktrees-sure-t3-7f34ca39-5542c437df4f301a/d672a99d-9dd8-4133-b4ab-9b52494be3de/scratchpad/research/R-04/mcp-demo/` (저장소 밖)

## 1. 요약

1. 공식 Rust SDK `rmcp` 3.9.0(2026-11-05 배포, MSRV 1.88)이 현재 규격 2026-07-28과 2025-11-25 이하를 함께 지원한다. 도구 하나짜리 서버를 만들어 Claude Code 2.1.322와 Codex 0.167.1 양쪽에서 **HTTP와 stdio 브리지(Unix 소켓) 두 방식 모두** 실제 호출에 성공했다.
2. 두 CLI의 프로토콜 세대가 다르다. Claude Code는 2026-07-28(stateless, `server/discover`)로, Codex는 2025-06-18(`initialize` 핸드셰이크)로 접속했다. 서버는 두 세대를 모두 받아야 하고, rmcp는 이를 기본으로 처리한다.
3. 추천은 **(나) Unix 소켓 + 작은 stdio 브리지**다. TCP 포트를 열지 않으므로 브라우저·DNS 재바인딩 경로가 아예 없고, 소켓 파일 권한과 접속자 uid·pid 확인, 실행별 토큰을 함께 쓸 수 있다. 설계 문서 SEC-3("로컬 소켓으로만")과도 맞는다.
4. 실행 중 발견한 설계 영향 두 가지. (a) Claude Code는 MCP 서버 접속에 실패해도 종료 코드 0으로 끝나므로 init 메시지의 `mcp_servers[].status`를 반드시 검사해야 한다. (b) 5만 자를 넘는 도구 결과는 `--no-session-persistence`를 줘도 `~/.claude…/projects/…/tool-results/`에 **평문 파일로 저장**된다. 도구 결과는 페이지 단위로 5만 자 미만으로 잘라야 한다.
5. 토큰은 CLI 자식 프로세스의 환경 변수로만 넘긴다. 설정 파일에는 `${VAR}` 참조만 쓰고 argv에는 넣지 않는다. 두 CLI 모두 이 방식으로 동작하는 것을 확인했다.

## 2. 질문별 답

### Q1. Rust MCP 서버 라이브러리의 현재 상태

**규격.** 현재(current) 규격은 **2026-07-28**이다 ([Versioning](https://modelcontextprotocol.io/specification/versioning)). 2025-11-25와 비교한 주요 변경은 다음과 같다 ([Changelog](https://modelcontextprotocol.io/specification/2026-07-28/changelog)).
- 프로토콜 세션과 `Mcp-Session-Id`를 없앴다. `initialize` 핸드셰이크도 없애고, 요청마다 `_meta`에 `io.modelcontextprotocol/protocolVersion`과 `clientCapabilities`를 싣는다.
- `server/discover`를 필수로 했고, HTTP GET 스트림 대신 `subscriptions/listen`을 쓴다.
- 서버가 시작하는 요청(sampling, elicitation, roots)을 MRTR(`InputRequiredResult`)로 바꿨다. 모든 결과에 `resultType`이 붙는다.
- Streamable HTTP의 POST 요청에 `Mcp-Method`, `Mcp-Name` 헤더가 필수가 됐다. 헤더와 본문이 다르면 `-32020 HeaderMismatch`를 반환한다.
- Roots, Sampling, Logging은 폐기 예정(Deprecated)이다.

| 라이브러리 | 최신 버전 (배포일) | 유지 상태 | 전송 | 규격 |
|---|---|---|---|---|
| **rmcp** (공식, `modelcontextprotocol/rust-sdk`) | 3.9.0 (2026-11-05). 3.0.0은 2026-07-28, 2.0.0은 2026-06-29 | 공식 조직 소속. 총 다운로드 약 4,769만 회. 2026-07-31 이후 마이너 릴리스 9회 | stdio(`transport-io`), 임의 AsyncRead/AsyncWrite(`transport-async-rw`), Streamable HTTP 서버(`transport-streamable-http-server`, Tower 서비스), HTTP 클라이언트(reqwest, Unix 소켓 클라이언트), 자식 프로세스 클라이언트, worker. 2024-11-05의 HTTP+SSE는 "의도적으로 제공하지 않음". `transport-ws`는 주석 처리됨 | 2026-07-28 구현, 2025-11-25 이하 호환 |
| rust-mcp-sdk (`rust-mcp-stack`) | 2.0.0 (2026-08-27) | 커뮤니티. 다운로드 약 34만 회 | stdio, Streamable HTTP(axum/actix), 1.x는 SSE 호환 | 2.x는 2026-07-28만 지원. 1.x(LTS)는 2025-11-25 |
| pmcp (`paiml/pmcp`) | 2.23.0 (2026-10-28) | 커뮤니티. 다운로드 약 16만 회 | stdio, Streamable HTTP, WebSocket, WASM | 2025-11-25가 기본. 2026-07-28은 명시적으로 켜야 함 (문서 안에서도 서술이 엇갈림) |

출처: [crates.io rmcp](https://crates.io/api/v1/crates/rmcp), [rmcp 버전 목록](https://crates.io/api/v1/crates/rmcp/versions?per_page=40), [rust-sdk README](https://github.com/modelcontextprotocol/rust-sdk), [rmcp Cargo.toml features](https://raw.githubusercontent.com/modelcontextprotocol/rust-sdk/main/crates/rmcp/Cargo.toml), [workspace Cargo.toml (MSRV 1.88, edition 2024)](https://raw.githubusercontent.com/modelcontextprotocol/rust-sdk/main/Cargo.toml), [crates.io rust-mcp-sdk](https://crates.io/api/v1/crates/rust-mcp-sdk), [docs.rs rust-mcp-sdk 2.0.0](https://docs.rs/crate/rust-mcp-sdk/2.0.0), [crates.io pmcp](https://crates.io/api/v1/crates/pmcp), [docs.rs pmcp](https://docs.rs/crate/pmcp/latest).

**판단.** rust-mcp-sdk 2.x는 2026-07-28만 받는다. 그런데 Codex 0.167.1은 2025-06-18 `initialize`로 접속하므로(아래 실험 4) 그대로는 쓸 수 없다. 두 세대를 모두 받는 공식 rmcp가 유일하게 무리가 없는 선택이다.

**rmcp 소스에서 확인한 사실** (`rmcp-3.9.0/src/transport/streamable_http_server/tower.rs`):
- `StreamableHttpServerConfig::default()`의 `allowed_hosts`는 `["localhost","127.0.0.1","::1"]`이다. Host 헤더 검사는 기본으로 켜져 있다.
- `allowed_origins`의 기본값은 빈 목록이고, 이 상태는 **Origin 검사를 끈다**("disables Origin validation for backward compatibility"). 규격은 Origin 검증을 MUST로 요구하므로 `enforce_origin_validation()`을 반드시 불러야 한다.
- `legacy_session_mode`의 기본값은 `true`다. 끄면 구버전 클라이언트도 세션 없이 처리한다.
- 서버 측 bearer 인증 기능은 없다. `auth` 기능은 OAuth 클라이언트용이다. 인증은 axum/tower 미들웨어로 직접 붙여야 한다.

### Q2. 전송 방식 비교: (가) 127.0.0.1 HTTP, (나) stdio 브리지 + 로컬 소켓

**두 CLI의 지원 현황**
- Claude Code가 지원하는 전송은 `stdio`, `http`(별칭 `streamable-http`), `sse`(폐기 예정), `ws`, 그리고 SDK 호스트 전용 `sdk`다. Unix 소켓 전송은 문서에 없다 ([Claude Code MCP](https://code.claude.com/docs/en/mcp)).
- Codex가 지원하는 전송은 STDIO와 Streamable HTTP다. Unix 소켓은 문서에 없다 ([Codex MCP](https://learn.chatgpt.com/docs/extend/mcp?surface=cli)).
- 따라서 (나)는 CLI 입장에서 보면 "stdio 서버"다. 브리지가 stdio 바이트를 앱의 Unix 소켓으로 그대로 전달한다.
- 규격은 Unix 소켓 같은 바이트 스트림 위의 사용자 정의 전송이 stdio 프레이밍(줄 단위 JSON-RPC)을 재사용하라고 권고한다(SHOULD) ([Transports](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports), [stdio](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/stdio)). rmcp는 `(AsyncRead, AsyncWrite)` 쌍을 바로 서빙할 수 있어 앱 쪽 코드가 짧다(실험 3).

**실행 검증 결과** (모두 2026-11-11에 실제로 실행)

| 조합 | 결과 | 관찰한 프로토콜 |
|---|---|---|
| Claude Code + HTTP(bearer) | 성공 | `MCP-Protocol-Version: 2026-07-28`. `server/discover` → `tools/list` → `tools/call` |
| Claude Code + stdio 브리지 → UDS | 성공 | stdio에서도 2026-07-28 `server/discover`부터 시작 |
| Codex + HTTP(`bearer_token_env_var`) | 성공 | 첫 POST에 헤더 없음(`initialize`), 이후 `2025-06-18` |
| Codex + stdio 브리지 → UDS | `env_vars`를 줄 때만 성공 | `initialize`(protocolVersion 2025-06-18) → `notifications/initialized` → `tools/list` → `tools/call` |

**비교**

| 항목 | (가) 127.0.0.1 HTTP | (나) stdio 브리지 + Unix 소켓 |
|---|---|---|
| 같은 PC의 다른 프로세스 | 모든 로컬 프로세스가 포트에 접속할 수 있다. 토큰만이 방어선이다 | 소켓 파일 0600에 0700 디렉터리. 다른 사용자는 파일 권한에 막힌다. 같은 사용자 프로세스는 토큰과 peer pid 검사로 거른다 |
| 브라우저·DNS 재바인딩 | 경로가 있다. Host·Origin 검사와 토큰이 필요하다. rmcp는 Origin 검사를 직접 켜야 한다 | 경로가 없다. 브라우저는 Unix 소켓에 닿지 못한다 |
| 포트 | 충돌 가능. 포트 0을 쓰고 실행마다 설정 파일을 써야 한다 | 포트가 없다. 단, 경로 길이 제한이 있다(macOS `sun_path` 104바이트, 실험에서 "path must be shorter than SUN_LEN" 오류를 냈다) |
| 두 CLI 지원 | 둘 다 표준 기능 | 둘 다 표준 stdio. Codex는 `env_vars` 허용 목록이 필요하다 |
| 구현 복잡도 | 낮다. rmcp `StreamableHttpService`와 axum 인증 미들웨어 약 20줄 | 중간. 앱에는 UDS 리스너와 rmcp 서빙(약 30줄), 별도 브리지 실행 파일(약 20줄)이 필요하다. 브리지를 Tauri sidecar로 번들하고 경로를 관리해야 한다 |
| 수명 관리 | 앱이 리스너 하나를 상시 연다. CLI가 죽어도 서버 쪽에서는 알 수 없고 토큰 폐기로만 끊는다. 앱이 죽으면 CLI는 연결 거부를 받는다 | CLI가 실행마다 브리지를 띄운다. CLI가 stdin을 닫으면 브리지가 끝난다(규격 SHOULD). 앱이 죽으면 소켓이 닫혀 브리지가 끝난다. 연결 하나가 실행 하나에 대응하므로 실행 단위 정리가 자연스럽다 |
| 대용량·제한 | Claude Code는 HTTP 응답 하나가 16 MiB를 넘으면 실패한다 | stdio 자체에는 제한이 명시되지 않았다. 도구 결과 크기 제한(Q4)은 두 방식에 똑같이 적용된다 |
| 유휴 타임아웃 (Claude Code) | HTTP 5분 | stdio 30분 |
| 감사 기록 위치 | tower 레이어에서 HTTP 단위로 기록 | 핸들러(call_tool) 단위로 기록 |

시간 제한과 크기 제한의 출처: [Claude Code MCP](https://code.claude.com/docs/en/mcp).

### Q3. 로컬 MCP 서버 인증

규격은 로컬 서버에 대해 다음을 요구하거나 권고한다.
- Origin 검증(MUST)과 localhost 바인딩, 인증(SHOULD) ([Streamable HTTP](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http)).
- 로컬 실행 서버는 "stdio 전송을 써서 접근을 MCP 클라이언트로 한정"하거나, HTTP라면 "인가 토큰 요구" 또는 "접근이 제한된 unix 도메인 소켓 등 IPC"를 쓰라고 권고한다 ([Security Best Practices](https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices)).

**권장 절차 (실험으로 검증한 범위)**
1. 앱이 실행마다 OS 난수로 256비트 토큰을 만든다. 실험에서는 `openssl rand -hex 32`로 대신했다. 토큰은 실행 ID에 묶고, 실행이 끝나면 즉시 폐기한다. 비교는 상수 시간으로 한다. HTTP 데모는 상수 시간 비교를 구현했고, UDS 데모는 단순 비교라 실제 구현에서 바꿔야 한다.
2. 토큰은 **자식 프로세스 환경 변수로만** 넘긴다(`Command::env`).
   - Claude Code: 설정 파일의 `headers`(HTTP)나 `env`(stdio)에 `${STOCK_MCP_TOKEN}`만 쓰면 실행 시점에 확장된다. 실험에서 토큰 값이 설정 파일에 들어가지 않았다. 단, `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, `NPM_TOKEN` 같은 일부 자격 증명 이름은 원격 서버의 url·headers에서 빈 값으로 읽힌다. 앱 고유의 이름을 써야 한다 ([Claude Code MCP](https://code.claude.com/docs/en/mcp)).
   - Codex: HTTP는 `bearer_token_env_var`, stdio는 `env_vars = ["STOCK_MCP_TOKEN"]`를 쓴다. **`env_vars`가 없으면 브리지에 환경 변수가 전달되지 않아** "required MCP servers failed to initialize"로 실패했다(실험 4). Codex 문서의 키 목록: [Codex MCP](https://learn.chatgpt.com/docs/extend/mcp?surface=cli).
3. argv에는 넣지 않는다. 이 PC에서 `ps -ww -o args=`는 같은 사용자 프로세스의 argv를 그대로 보여 줬다. 반면 `ps -Eww`는 같은 사용자 프로세스의 환경 변수를 보여 주지 않았다. 이것은 관찰일 뿐이고 OS 보장 여부는 미확인이다(4장).
4. (나)의 추가 방어:
   - 소켓을 0700 디렉터리 안에 0600으로 만든다.
   - `accept` 직후 peer uid가 앱과 같은지 확인한다. tokio `UnixStream::peer_cred()`로 macOS에서 uid와 **pid까지** 얻었다(실험 3 로그 `peer uid=501 pid=Some(81079)`).
   - 실제 구현에서는 peer pid의 부모가 앱이 띄운 CLI pid인지 확인할 수 있다. 이 검사는 구현하지 않았으므로 미확인이다.
   - 실행마다 소켓 경로를 따로 만들고 실행이 끝나면 지우면 경로와 토큰이 모두 일회용이 된다.
5. (가)를 쓴다면 다음을 모두 켠다. 실험 2에서 각각의 결과를 확인했다.
   - `enforce_origin_validation()`: 악성 Origin을 403으로 거부했다.
   - 기본 `allowed_hosts`: `Host: evil.example`을 403으로 거부했다.
   - bearer 미들웨어: 토큰이 없으면 401.
   - `with_legacy_session_mode(false)`.
6. 같은 사용자 권한의 악성 프로세스가 CLI 프로세스 메모리나 환경을 읽는 것은 이 구조로 막을 수 없다. 이것은 R-16(로컬 보안)의 범위다.

### Q4. 도구 스키마 설계

**입력 검증.**
- 입력 타입에 `#[serde(deny_unknown_fields)]`와 `schemars::JsonSchema`를 붙이면 rmcp가 JSON Schema 2020-12와 `additionalProperties:false`를 생성한다(실험 1의 `tools/list` 출력).
- 모르는 필드를 넣으면 rmcp가 역직렬화 단계에서 `isError:true` 결과("unknown field `extra`")로 돌려줬다.
- 의미 검증(종목 코드 형식 등)은 핸들러 첫 줄에서 도메인 newtype으로 파싱한다.
- 규격상 **입력 검증 오류는 프로토콜 오류가 아니라 도구 실행 오류(`isError:true`)로 반환해야** 모델이 스스로 고칠 수 있다 ([Tools – Error Handling](https://modelcontextprotocol.io/specification/2026-07-28/server/tools)). 실험에서 `Err(ErrorData::invalid_params)`를 반환했더니 JSON-RPC 오류 `-32602`가 됐다. 이 방식은 피하고 `CallToolResult::structured_error(...)`를 쓴다.
- Claude Code는 최상위 `anyOf/oneOf/allOf`를 평탄화하고, 최상위 속성 이름은 1~64자 `[A-Za-z0-9_.-]`만 허용한다. 위반하면 그 도구를 제외한다 ([Claude Code MCP](https://code.claude.com/docs/en/mcp)). 입력 스키마의 최상위는 단순 object로 둔다.

**구조화된 출력.**
- `Json<T>`를 반환하면 rmcp가 `outputSchema`를 생성한다. 결과에는 `structuredContent`와 같은 JSON의 text 블록이 함께 들어간다(실험 1). 규격도 하위 호환을 위해 text 블록을 같이 넣으라고 권고한다(SHOULD).
- Claude Code는 모델에 text 블록을 전달했다(stream-json의 tool_result가 JSON 문자열이었다). Codex `--json`의 `mcp_tool_call` 이벤트에는 `structured_content`가 별도 필드로 왔다.
- 금액은 십진수 문자열로 둔다(설계의 기존 원칙과 같다).

**큰 결과와 페이징.** Claude Code의 제한은 다음과 같다 ([Claude Code MCP](https://code.claude.com/docs/en/mcp)).
- 1만 토큰을 넘으면 경고한다.
- 기본 2만 5천 토큰(`MAX_MCP_OUTPUT_TOKENS`)을 넘거나, 이미지가 없는 텍스트 결과가 **5만 자**를 넘으면 결과를 파일로 저장하고 대화에는 경로만 넣는다.
- 도구별로 `_meta["anthropic/maxResultSizeChars"]`를 주면 최대 50만 자까지 올릴 수 있다.
- **실험 결과**: 60,000자를 반환하는 도구를 `--no-session-persistence --tools ""`로 호출했다. 결과가 `~/.claude-second/projects/<cwd>/<session>/tool-results/toolu_….json`에 평문으로 저장됐다. 모델은 앞 2KB 미리보기만 보고 "NOT VISIBLE"이라고 답했다(파일 읽기 도구가 없어서다).
- 결론:
  - 모든 도구는 응답 하나를 **5만 자보다 충분히 작게**(예: 3만 자) 제한한다.
  - 목록형 도구는 `limit`(상한 고정)과 불투명한 `cursor`를 받고 `next_cursor`와 `total`을 돌려준다.
  - 커서는 규격의 "상태 핸들" 지침을 따른다. 추측할 수 없는 난수로 만들고, 유효 기간을 두고, 만료되면 도구 실행 오류를 반환한다 ([Tools – Stateful Tools](https://modelcontextprotocol.io/specification/2026-07-28/server/tools)).
  - `maxResultSizeChars`로 한도를 올리는 것은 피한다. 대화 맥락이 커지기 때문이다.

**오류 표현.** `structured_error({code, message, retryable, field?})`로 통일한다. `code`는 앱 고유의 열거값이다. Claude Code는 오류 텍스트가 약 1만 1천 자를 넘으면 앞뒤 5천 자만 남긴다 ([Claude Code MCP](https://code.claude.com/docs/en/mcp)).

**주석.** 읽기 도구에는 `readOnlyHint:true`를 붙인다. Claude Code는 이런 호출을 병렬로 실행하고, 연결이 끊기면 한 번 재시도할 수 있다. `propose_order`처럼 상태를 바꾸는 도구에는 붙이지 않는다. 규격상 주석은 클라이언트가 신뢰할 근거가 아니므로 서버가 직접 강제해야 한다.

**`tools/list` 순서.** 결정적으로 정렬해 돌려준다(SHOULD). 그래야 CLI의 init 도구 목록 검사(SEC-2)와 스냅샷 테스트가 안정적이다.

### Q5. 외부 뉴스 텍스트가 섞일 때 프롬프트 주입을 줄이는 표현

1. **구조로 분리한다.** 앱이 만든 사실(시각, 종목, 수치, 출처 URL)과 외부 텍스트를 다른 필드에 둔다. 예: `{"source":"연합뉴스","url":"…","fetched_at":"…","untrusted_text":"…"}`. 외부 텍스트를 도구 설명, 오류 메시지, 필드 이름에 절대 넣지 않는다.
2. **스포트라이팅.** Microsoft 연구에서 구분자만 쓰는 방식은 효과가 제한적이었다. 데이터마킹(공백을 특수 기호로 바꾸기)이나 인코딩은 GPT 계열에서 공격 성공률을 50% 이상에서 2% 미만으로 줄였다 ([Hines et al., arXiv 2403.14720](https://arxiv.org/abs/2403.14720)). 다만 Claude 모델과 현재 모델 세대에서의 효과는 미확인이다.
3. **정규화.** 제어 문자와 폭 0 문자, 양방향 제어 문자를 지우고, 길이 상한을 둔다. 마크다운 링크와 이미지는 평문으로 바꾼다. URL은 앱이 수집한 목록에 있는 것만 남긴다(design.md의 `read_article` 원칙과 같다).
4. **권한 분리가 핵심이다.** 주입을 완전히 막는 표현 방법은 없다. 외부 텍스트를 읽는 실행에서는 `propose_order` 같은 쓰기 도구를 빼고 요약만 하게 하고, 제안은 요약만 받는 다른 실행이 하게 하는 구성이 근본적인 방어다. 이 구성의 설계 근거는 [CaMeL, arXiv 2503.18813](https://arxiv.org/abs/2503.18813)이다. 이 앱은 이미 "제안은 주문을 보내지 않고 사용자가 승인한다"는 구조라서, 주입의 최대 피해는 잘못된 제안 한 건으로 제한된다.
5. Claude Code의 WebFetch는 페이지를 별도 모델 호출로 요약해 전달한다 ([Claude Code Security](https://code.claude.com/docs/en/security)). 이것은 내장 도구에만 적용되고, MCP 도구 결과에는 적용되지 않는다. 앱이 직접 처리해야 한다.

### Q6. 도구 호출 기록과 재현

- **서버 쪽 기록(정본).** `ServerHandler::call_tool`을 감싸는 래퍼 또는 rmcp 라우터 앞단에서 다음을 SQLite 추가 전용 테이블에 남긴다: `run_id`(토큰에서 파생), 요청 `id`, 도구 이름, 원본 인자 JSON, 결과 JSON(또는 해시와 본문), `isError`, 시작·종료 시각, 프로토콜 버전, 클라이언트 정보. HTTP라면 tower 레이어에서 `Mcp-Method`와 `Mcp-Name` 헤더도 쓸 수 있다(실험 2 로그). 감사 로그 무결성은 R-06과 R-16의 범위다.
- **클라이언트 쪽 기록(대조용).** Claude Code stream-json의 `tool_use`/`tool_result`와 Codex `--json`의 `mcp_tool_call`(arguments, result, status)을 같은 `run_id`로 저장해 두 기록을 대조한다(실험에서 두 이벤트 모두 확인).
- **재현.** 기록한 `tools/call`을 고정 픽스처 DB와 고정 시계로 다시 호출하고 결과를 골든 파일과 비교한다. 서버가 stateless이므로 요청 하나만으로 재현할 수 있다. 규격도 2026-07-28 프로토콜이 stateless이고 진행 중 요청은 재시도하면 된다고 적고 있다 ([stdio – Unexpected Termination](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/stdio)).
- 규격은 클라이언트에 "감사를 위한 도구 사용 기록"을 권고하고(SHOULD), 서버에는 입력 검증, 접근 통제, 호출 속도 제한, 출력 정화를 요구한다(MUST) ([Tools – Security](https://modelcontextprotocol.io/specification/2026-07-28/server/tools)).

### Q7. 실제 CLI 없이 테스트하기

1. **프로세스 안에서 테스트(검증 완료).** `tokio::io::duplex`로 서버와 rmcp 클라이언트(`features=["client"]`, 개발 의존성)를 연결해 `list_all_tools`와 `call_tool`을 검사했다. `cargo test` 결과: `test result: ok. 1 passed`. 네트워크와 프로세스가 필요 없어 가장 빠르다.
2. **원시 JSON 줄(검증 완료).** stdio 바이너리에 `server/discover`, `tools/list`, `tools/call`을 한 줄씩 넣고 응답을 스냅샷과 비교한다(실험 1). 두 세대(2026-07-28 `_meta` 방식과 `initialize` 방식)를 모두 넣어 호환성을 검사한다.
3. **HTTP 경계(검증 완료).** curl로 401, 403(Origin, Host), 400(-32020 헤더 불일치), 405(GET)를 확인했다(실험 2). (가)를 택하면 이 항목을 회귀 테스트로 둔다.
4. **공식 도구(실행하지 않음).** MCP Inspector `@modelcontextprotocol/inspector` 2.14.0 ([npm](https://registry.npmjs.org/@modelcontextprotocol/inspector/latest))과 공식 적합성 테스트 `@modelcontextprotocol/conformance`(`npx @modelcontextprotocol/conformance server --url <url>`, [README 0.1.16](https://cdn.jsdelivr.net/npm/@modelcontextprotocol/conformance@0.1.16/README.md))가 있다. 적합성 테스트는 URL 기반이라 HTTP 서버를 띄워야 한다. 현재 버전과 2026-07-28 시나리오 범위는 미확인이다.
5. **`tools/list` 스냅샷.** 도구 이름, 스키마, 주석을 골든 파일로 고정하면 도구 목록 변경이 리뷰에 드러난다. SEC-2 검사가 기대하는 목록과 같은 원천을 쓴다.

## 3. 선택지 비교표와 추천

### 라이브러리

| 기준 | rmcp 3.9.0 | rust-mcp-sdk 2.0.0 | pmcp 2.23.0 |
|---|---|---|---|
| 공식 여부 | 공식 | 아님 | 아님 |
| Codex(2025-06-18 initialize) 수용 | 예 (실험) | 아니오 (2.x는 2026-07-28 전용) | 예 (기본 v1) |
| Claude Code(2026-07-28) 수용 | 예 (실험) | 예 (문서 기준) | 켜야 함 |
| 임의 바이트 스트림(UDS) 서빙 | 예 (`transport-async-rw`, 실험) | 미확인 | 미확인 |
| 변경 속도 | 빠름 (메이저 2.0→3.0이 한 달 간격) | 중간 | 빠름 |

**추천: rmcp 3.9.0, 정확한 버전 고정(`=3.9.0`).** 두 CLI 세대를 모두 받는 것을 직접 확인한 유일한 선택지다. 공식 SDK라 규격 변경을 가장 먼저 따라간다.

### 전송

**추천: (나) Unix 소켓 + stdio 브리지.**

이유:
1. 네트워크 공격면(브라우저, DNS 재바인딩, 포트 스캔)이 0이다.
2. 파일 권한과 peer uid·pid, 실행별 토큰으로 방어를 세 겹 둘 수 있다.
3. 두 CLI 모두 가장 성숙한 stdio 경로를 쓴다.
4. 실행 하나가 연결 하나라 수명 관리와 감사 귀속이 단순하다.
5. design.md SEC-3과 일치한다.

비용은 브리지 실행 파일 하나다. 브리지는 workspace의 다른 crate에 의존하지 않는 약 20줄짜리 tokio 바이너리다.

**추천이 틀린 것으로 판명될 조건**
- Claude Code나 Codex가 stdio MCP 서버를 실행할 때 환경 변수 전달을 막거나, 샌드박스로 Unix 소켓 접속을 막도록 바뀌는 경우. Codex `-s read-only`에서는 접속이 됐다.
- Tauri sidecar 번들, 서명, 경로 해석 때문에 브리지 배포가 HTTP보다 훨씬 복잡해지는 경우(R-05에서 확인).
- 소켓 경로를 104바이트 안에 안정적으로 둘 수 없는 경우.
- 앱이 MCP 서버를 CLI가 아닌 다른 클라이언트(예: 원격 기기)에도 열어야 하는 경우. 그때는 (가)가 맞다.

**(가)를 택해야 할 때의 필수 구성**: `127.0.0.1` 바인딩, 포트 0, `enforce_origin_validation()`, 기본 `allowed_hosts`, `with_legacy_session_mode(false)`, bearer 미들웨어(상수 시간 비교), 실행별 토큰 폐기.

## 4. 위험과 미확인 항목

**위험**
- **Claude Code가 MCP 실패를 성공처럼 끝낸다.** 토큰이 틀리거나 소켓이 없으면 init에 `{"status":"failed"}`와 `tools: []`가 오는데도 종료 코드가 0이고 `result.subtype`이 `success`였다. 모델은 존재하지 않는 도구 호출을 텍스트로 지어냈다(실험 5). SEC-2의 init 검사가 이 경우를 반드시 잡아야 한다. Codex는 `required = true`로 즉시 실패시킬 수 있다(실험 4).
- **큰 결과의 평문 디스크 저장.** Q4에서 본 대로다. 5만 자를 넘는 결과는 `--no-session-persistence`여도 CLI 설정 폴더에 저장된다. 보유 내역이 섞이면 design.md 4.2의 "대화 원본은 앱 SQLite에만" 원칙이 깨진다. 이미지 결과도 파일로 저장한다고 문서에 적혀 있다. 단, `--no-session-persistence`에서는 이미지 파일을 쓰지 않는다고 한다 ([Claude Code MCP](https://code.claude.com/docs/en/mcp)).
- **rmcp 변경 속도.** 2.0.0(2026-06-29)부터 3.0.0(2026-07-28)까지 한 달이었고, 3.0.0 이후 3개월여 동안 릴리스가 16회(마이너 9회, 패치 7회) 나왔다. 정확한 버전을 고정하고 의도한 시점에만 올린다.
- **rmcp의 Origin 기본값이 꺼져 있다.** (가)를 택하고 이것을 놓치면 규격 MUST를 위반한다.
- **두 CLI의 프로토콜 세대가 갈라져 있다.** 서버에서 2025-x 경로를 지원 목록에서 빼면 Codex가 끊긴다. rmcp는 `supported_protocol_versions`로 세대를 제한할 수 있다.
- **Codex 격리 누수(R-03 관련).** `--ignore-user-config --ignore-rules`를 줘도 Codex는 `~/.codex/agents/*.toml`을 읽고 오류 이벤트 28건을 냈다. MCP 연결과는 별개지만 SEC-2 격리 판단에 영향을 준다.
- **주입 방어의 한계.** 표현 기법은 확률적 방어일 뿐이다. 권한 분리(Q5-4)가 없으면 남는 위험이 크다.

**미확인**
- macOS에서 같은 사용자의 다른 프로세스가 CLI 자식 프로세스의 환경 변수를 읽을 수 있는지에 대한 OS 수준 보장. `ps -Eww`로는 보이지 않았지만 `sysctl KERN_PROCARGS2` 같은 경로는 조사하지 않았다.
- 브리지 peer pid의 부모 체인 확인(libproc `proc_pidinfo`)의 구현성과 경쟁 조건.
- Claude Code가 `tool-results` 파일을 언제 지우는지, 경로를 바꾸거나 끌 수 있는지.
- Claude Code가 `structuredContent`와 `outputSchema`를 검증하는지, 모델에 무엇을 넘기는지에 대한 문서 근거. 실험에서는 text 블록이 전달되는 것만 관찰했다.
- Codex CLI가 2026-07-28을 지원할 계획이 있는지와 그 시점.
- MCP Inspector와 공식 적합성 테스트의 실제 실행 결과(실행하지 않았다).
- Codex가 stdio MCP 서버 프로세스에 샌드박스를 적용하는 범위. read-only에서는 UDS 접속이 됐다는 관찰만 있다.
- 스포트라이팅이 Claude 모델에 주는 효과.

## 5. 이 결과로 바뀌어야 할 설계 문서 항목

| 문서 | 위치 | 변경 |
|---|---|---|
| architecture.md | 2.1 그림의 "MCP 서버 (127.0.0.1, 실행별 토큰)" | "MCP 서버 (Unix 소켓, 실행별 토큰) ← stdio 브리지 ← CLI"로 바꾼다. 현재 그림은 SEC-3과 충돌한다 |
| architecture.md | 2.1의 "MCP 전송 방식은 R-04에서 확정" 문장 | 결정 내용으로 바꾸고 ADR(`docs/adr/`, 예: "MCP 전송: UDS + stdio 브리지, rmcp =3.9.0")을 연결한다 |
| architecture.md | 3.1 배치와 3.2 의존 규칙 | `crates/mcp-bridge/`(바이너리, workspace crate 의존 0, tokio만 사용)를 추가한다. `xtask arch-check`가 이 crate의 의존을 0으로 강제한다. `mcp-server`는 `rmcp`(정확한 버전)에 의존한다 |
| architecture.md | 7장 R-04 행 | 해결로 표시한다 |
| design.md | SEC-3 | 구체화한다: 0700 디렉터리 안의 0600 소켓, 경로 104바이트 이내, 실행별 소켓과 토큰, peer uid 일치 검사, 토큰은 환경 변수로만 전달(argv와 설정 파일 금지), 실행 종료 시 폐기 |
| design.md | 4.1 실행 방식 표 | Claude: `--mcp-config <실행별 파일>`(stdio, `command`=브리지, `env.STOCK_MCP_TOKEN="${STOCK_MCP_TOKEN}"`) `--strict-mcp-config`. Codex: `-c mcp_servers.app.command=…`, `-c mcp_servers.app.args=[…]`, `-c 'mcp_servers.app.env_vars=["STOCK_MCP_TOKEN"]'`, `-c mcp_servers.app.required=true` |
| design.md | 4.1 / SEC-2 init 검사 | 도구 목록만 보는 것으로는 부족하다. `mcp_servers[]`에서 앱 서버의 `status=="connected"`도 확인하고, 아니면 중단해야 한다 |
| design.md | 3장 MCP 도구 목록 | 공통 규칙을 추가한다: 응답 하나에 3만 자 상한, 목록 도구는 `limit`과 `cursor`, 오류는 `isError`와 구조화 오류 코드, 읽기 도구에 `readOnlyHint`, `structuredContent`와 text 블록 병행, 결정적 정렬 |
| design.md | `search_news`, `read_article` 설명 | 외부 텍스트는 `untrusted_text` 필드로만 전달한다. 정규화(제어·폭 0 문자 제거, 링크 평문화, 길이 상한)를 적용한다. 본문 읽기 실행과 제안 실행을 분리하는 방안을 R-12에서 검토한다 |
| design.md | 4.2 대화 기록 | 큰 도구 결과가 CLI 폴더에 평문으로 저장되는 문제를 기록하고 위 크기 상한과 연결한다(R-02와 함께 다룬다) |
| harness.md | Rust 테스트 계층 | MCP 계약 테스트를 추가한다: duplex 프로세스 내 테스트, 두 프로토콜 세대의 JSON 줄 스냅샷, `tools/list` 골든 파일 |

## 6. 부록: 실행 검증 기록

### 의존성 (`Cargo.toml`)

```toml
[dependencies]
anyhow = "1.0.104"
axum = "0.8.9"
rmcp = { version = "3.9.0", features = ["server", "macros", "transport-io", "transport-streamable-http-server", "schemars"] }
schemars = "1.2.3"
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.154"
tokio = { version = "1.54.1", features = ["full"] }

[dev-dependencies]
rmcp = { version = "3.9.0", features = ["client"] }
```

rustc 1.88.0에서 처음 빌드에 성공했다(약 23초). cargo는 `uuid`를 Rust 1.88과 호환되는 1.26.1로 고정했다.

### 핵심 코드 (`src/main.rs` 발췌, 전체 270줄)

```rust
#[derive(Debug, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct QuoteParams {
    /// 종목 코드. 6자리 숫자(국내) 또는 1~5자 대문자(미국).
    symbol: String,
}

#[derive(Debug, Serialize, schemars::JsonSchema)]
struct QuoteResult { symbol: String, price: String, currency: String, as_of: String, source: String }

#[derive(Clone)]
struct DemoServer;

#[tool_router]
impl DemoServer {
    #[tool(description = "Returns the last price of one symbol from the app's stored data.",
           annotations(read_only_hint = true))]
    fn get_quote(&self, Parameters(QuoteParams { symbol }): Parameters<QuoteParams>)
        -> Result<Json<QuoteResult>, ErrorData> {
        eprintln!("[audit] tools/call get_quote symbol={symbol}");
        if !valid_symbol(&symbol) {
            // 실제 구현에서는 규격대로 CallToolResult::structured_error(isError:true)를 쓴다
            return Err(ErrorData::invalid_params(format!("symbol must be ... got {symbol:?}"), None));
        }
        Ok(Json(QuoteResult { symbol, price: "71200".into(), currency: "KRW".into(),
            as_of: "2026-11-11T06:30:00Z".into(), source: "demo-fixture".into() }))
    }
}

#[tool_handler(name = "stock-copilot-demo", version = "0.0.1")]
impl ServerHandler for DemoServer {}

// (가) HTTP
async fn run_http(port: u16, token: String) -> anyhow::Result<()> {
    let config = StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .enforce_origin_validation();
    let service = StreamableHttpService::new(|| Ok(DemoServer), LocalSessionManager::default().into(), config);
    let app = Router::new()
        .nest_service("/mcp", service)
        .layer(middleware::from_fn_with_state(Arc::new(token), bearer_auth)); // Bearer 상수 시간 비교, 실패 시 401
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// (나) 앱 쪽: Unix 소켓에서 rmcp를 직접 서빙
async fn run_uds(path: String, token: String) -> anyhow::Result<()> {
    let listener = tokio::net::UnixListener::bind(&path)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    loop {
        let (stream, _) = listener.accept().await?;
        let cred = stream.peer_cred()?;               // macOS에서 uid와 pid 모두 얻음
        if cred.uid() != my_uid { continue; }
        let token = token.clone();
        tokio::spawn(async move {
            let (r, w) = stream.into_split();
            let mut reader = BufReader::new(r);
            let mut first = String::new();
            if reader.read_line(&mut first).await.is_err() || first.trim_end() != format!("AUTH {token}") {
                return;                                // 실제 구현은 상수 시간 비교
            }
            if let Ok(running) = DemoServer.serve((reader, w)).await { let _ = running.waiting().await; }
        });
    }
}

// (나) 브리지: CLI가 stdio 서버로 실행. 토큰은 환경 변수에서만 읽는다
async fn run_bridge(path: String) -> anyhow::Result<()> {
    let token = std::env::var("STOCK_MCP_TOKEN")?;
    let stream = tokio::net::UnixStream::connect(&path).await?;
    let (mut sr, mut sw) = stream.into_split();
    sw.write_all(format!("AUTH {token}\n").as_bytes()).await?;
    let (mut stdin, mut stdout) = (tokio::io::stdin(), tokio::io::stdout());
    let up = async { tokio::io::copy(&mut stdin, &mut sw).await?; sw.shutdown().await };
    let down = tokio::io::copy(&mut sr, &mut stdout);
    tokio::select! { r = up => { r?; } r = down => { r?; } }
    Ok(())
}

// Q7: CLI 없이 테스트
#[tokio::test]
async fn in_process_client_lists_and_calls_tool() -> anyhow::Result<()> {
    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    tokio::spawn(async move { DemoServer.serve(server_io).await?.waiting().await?; anyhow::Ok(()) });
    let client = ().serve(client_io).await?;
    let names: Vec<_> = client.list_all_tools().await?.iter().map(|t| t.name.to_string()).collect();
    assert_eq!(names, vec!["get_big", "get_quote"]);
    // call_tool로 정상 결과의 structuredContent.price == "71200"을 확인하고, 잘못된 symbol은 거부되는지 확인
    client.cancel().await?;
    Ok(())
}
```

### 실험 1: stdio 원시 JSON (2026-07-28 형식)

```
→ server/discover
← {"result":{"resultType":"complete","supportedVersions":["2024-11-05","2025-03-26","2025-06-18","2025-11-25","2026-07-28"],"capabilities":{"tools":{}},...}}
→ tools/list
← inputSchema: {"$schema":"https://json-schema.org/draft/2020-12/schema","additionalProperties":false,"properties":{"symbol":{...}},"required":["symbol"],"type":"object"}, outputSchema: {...}, annotations: {"readOnlyHint":true}
→ tools/call get_quote {"symbol":"005930"}
← {"content":[{"type":"text","text":"{\"as_of\":...}"}],"structuredContent":{...,"price":"71200"},"isError":false}
→ tools/call get_quote {"symbol":"x; rm"}
← {"error":{"code":-32602,"message":"symbol must be 6 digits or 1-5 uppercase letters, got \"x; rm\""}}
→ tools/call get_quote {"symbol":"AAPL","extra":1}
← {"content":[{"type":"text","text":"failed to deserialize parameters: unknown field `extra`, expected `symbol`"}],"isError":true}
```

### 실험 2: HTTP 경계 (curl)

```
토큰 없음                              → 401
올바른 토큰                            → 200, structuredContent 포함
Origin: http://evil.example           → 403 "Forbidden: Origin header is not allowed"
Host: evil.example:38917              → 403 "Forbidden: Host header is not allowed"
Mcp-Name: other (본문은 get_quote)     → 400 {"code":-32020,"message":"Mcp-Name header `other` does not match body value `get_quote`"}
GET /mcp                              → 405
```

### 실험 3: Claude Code 2.1.322

명령(빈 폴더에서 실행. HTTP 설정 파일 `{"mcpServers":{"stock":{"type":"http","url":"http://127.0.0.1:38918/mcp","headers":{"Authorization":"Bearer ${STOCK_MCP_TOKEN}"}}}}`):

```
claude -p "Use the get_quote tool for symbol 005930 and reply with only the price." \
  --mcp-config mcp-http.json --strict-mcp-config --tools "" --allowedTools "mcp__stock__get_quote" \
  --setting-sources "" --no-session-persistence --output-format stream-json --verbose --model haiku
```

HTTP 결과:

```
INIT tools= ['mcp__stock__get_quote'] mcp= [{'name': 'stock', 'status': 'connected', 'source': 'dynamic'}] ver= 2.1.322
TOOL_USE mcp__stock__get_quote {'symbol': '005930'}
TOOL_RESULT {"as_of":"2026-11-11T06:30:00Z","currency":"KRW","price":"71200","source":"demo-fixture","symbol":"005930"}
TEXT 71,200 KRW
RESULT success False 71,200 KRW
서버 로그:
[http] POST /mcp proto=2026-07-28 method=server/discover ...
[http] POST /mcp proto=2026-07-28 method=tools/list ...
[http] POST /mcp proto=2026-07-28 method=tools/call name=get_quote ...
```

stdio 브리지 결과(설정 `{"type":"stdio","command":".../mcp-demo","args":["bridge","../app.sock"],"env":{"STOCK_MCP_TOKEN":"${STOCK_MCP_TOKEN}"}}`):

```
INIT tools= ['mcp__stock__get_quote'] mcp= [{'name': 'stock', 'status': 'connected', ...}]
TOOL_USE mcp__stock__get_quote {'symbol': '005930'}
TEXT 71200 KRW
앱 로그: [uds] peer uid=501 pid=Some(81079) / [audit] tools/call get_quote symbol=005930
stdio 첫 메시지들: server/discover 2026-07-28 → tools/list 2026-07-28 → tools/call 2026-07-28
잘못된 토큰으로 브리지 실행 시: [uds] rejected: bad auth line
```

### 실험 4: Codex CLI 0.167.1

```
codex exec --json --ephemeral --ignore-user-config --ignore-rules --skip-git-repo-check -s read-only \
  -c 'mcp_servers.stock.url="http://127.0.0.1:38919/mcp"' \
  -c 'mcp_servers.stock.bearer_token_env_var="STOCK_MCP_TOKEN"' \
  -c 'mcp_servers.stock.required=true' \
  -c 'mcp_servers.stock.default_tools_approval_mode="approve"' \
  "Call the get_quote MCP tool with symbol 005930 and reply with only the price."
```

```
item.completed mcp_tool_call {"server":"stock","tool":"get_quote","arguments":{"symbol":"005930"},"status":"completed","result":{"content":[...],"structured_content":{...}}}
item.completed agent_message {"text":"71200"}
서버 로그: 첫 POST proto=-(initialize), 이후 proto=2025-06-18, ua=codex-mcp-client/0.167.1
```

stdio 브리지(`mcp_servers.stock.command=<브리지>`, `args=["bridge","../app.sock"]`):

```
env_vars 없음 → ERROR ... required MCP servers failed to initialize: stock: handshaking with MCP server failed: connection closed: initialize response
env_vars=["STOCK_MCP_TOKEN"] → mcp_tool_call completed, agent_message "71200"
stdio 메시지: initialize(protocolVersion 2025-06-18) → notifications/initialized → tools/list → tools/call
```

### 실험 5: 실패와 한계

```
# 잘못된 토큰(HTTP)으로 Claude Code 실행: 종료 코드 0
INIT tools= [] mcp= [{'name': 'stock', 'status': 'failed', 'source': 'dynamic'}]
TEXT I'll look up the AAPL quote now. <invoke name="get_quote">... Error: No such tool available: get_quote
RESULT success False ...

# 60,000자 결과 (--no-session-persistence --tools "")
TOOL_RESULT <persisted-output> Output too large (58.7KB). Full output saved to: ~/.claude-second/projects/<cwd>/<session>/tool-results/toolu_016g8PoDgt4pTeJXvrVGfCgT.json
TEXT ... I have no tool here to open the saved file ... NOT VISIBLE

# 소켓 경로 160바이트
Error: path must be shorter than SUN_LEN
```

## 7. 출처 목록 (모두 2026-11-11 확인)

- MCP 규격 Versioning (current 2026-07-28): https://modelcontextprotocol.io/specification/versioning
- MCP 2026-07-28 Changelog: https://modelcontextprotocol.io/specification/2026-07-28/changelog
- MCP 2026-07-28 Transports 개요(사용자 정의 전송): https://modelcontextprotocol.io/specification/2026-07-28/basic/transports
- MCP 2026-07-28 stdio: https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/stdio
- MCP 2026-07-28 Streamable HTTP: https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http
- MCP 2026-07-28 Tools: https://modelcontextprotocol.io/specification/2026-07-28/server/tools
- MCP Security Best Practices: https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices
- rmcp GitHub (공식 Rust SDK): https://github.com/modelcontextprotocol/rust-sdk
- rmcp crates.io 메타데이터: https://crates.io/api/v1/crates/rmcp , 버전 목록: https://crates.io/api/v1/crates/rmcp/versions?per_page=40
- rmcp features: https://raw.githubusercontent.com/modelcontextprotocol/rust-sdk/main/crates/rmcp/Cargo.toml , workspace(MSRV): https://raw.githubusercontent.com/modelcontextprotocol/rust-sdk/main/Cargo.toml
- rmcp 3.9.0 소스(로컬 레지스트리): `~/.cargo/registry/src/index.crates.io-*/rmcp-3.9.0/src/transport/streamable_http_server/tower.rs`, `src/handler/server/wrapper/json.rs`, `src/model.rs`
- rust-mcp-sdk: https://crates.io/api/v1/crates/rust-mcp-sdk , https://docs.rs/crate/rust-mcp-sdk/2.0.0
- pmcp: https://crates.io/api/v1/crates/pmcp , https://docs.rs/crate/pmcp/latest
- Claude Code MCP 문서: https://code.claude.com/docs/en/mcp
- Claude Code Security 문서: https://code.claude.com/docs/en/security
- Codex MCP 문서: https://learn.chatgpt.com/docs/extend/mcp?surface=cli (developers.openai.com/codex/mcp에서 308 리디렉션)
- Codex CLI 0.167.1 `codex exec --help` (로컬 실행 결과)
- Claude Code 2.1.322 `claude --help` (로컬 실행 결과)
- MCP Inspector npm: https://registry.npmjs.org/@modelcontextprotocol/inspector/latest
- MCP Conformance README: https://cdn.jsdelivr.net/npm/@modelcontextprotocol/conformance@0.1.16/README.md
- Hines et al., Defending Against Indirect Prompt Injection Attacks With Spotlighting: https://arxiv.org/abs/2403.14720
- Debenedetti et al., Defeating Prompt Injections by Design (CaMeL): https://arxiv.org/abs/2503.18813
