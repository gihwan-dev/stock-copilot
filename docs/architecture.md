# 아키텍처 (v0.2, 2026-11-11)

제품 요구사항은 [design.md](design.md), 강제 규칙은 [harness.md](harness.md), 사용자가 확정한 결정은 [open-issues.md](open-issues.md)에 있다. 이 문서는 구조와 의존 방향만 정한다. 이번 판에서 확정한 구조 결정은 [ADR 0002](adr/0002-architecture-decisions.md)에 표로 모았다.

v0.1에서 바뀐 큰 줄기:

| 바뀐 것 | 내용 | 근거 |
|---|---|---|
| AI의 역할 | AI는 확률과 근거만 낸다. 방향, 수량, 가격은 코드가 정한다. `propose_order`가 없어졌다 | [open-issues.md](open-issues.md) 1장 |
| AI 실행 | 수집 실행(웹 있음, 계좌 모름)과 분석 실행(계좌 앎, 웹 없음)으로 나눈다 | open-issues.md 3장 |
| MCP 전송 | Unix 소켓과 stdio 브리지. 포트를 열지 않는다 | I-01, [R-04](research/R-04-rust-mcp-server.md), [R-16](research/R-16-local-security.md) |
| 상주 | 창을 닫아도 메뉴 막대에 남아 돈다 | I-09, [R-05](research/R-05-tauri-architecture.md) |
| 주문 차단 | 승인 명령을 별도 crate로 꺼내고, `src-tauri`도 `order-execution`에 직접 의존하지 않는다 | [R-09](research/R-09-rust-strictness.md), [R-15](research/R-15-tauri-testing.md) |

## 1. 설계 목표

1. **사람 승인 없이 주문이 나가는 경로가 구조적으로 없다.** 주문 전송 코드는 승인 증거 없이는 컴파일되지 않고, AI가 닿는 코드는 주문 전송 코드를 컴파일 단위로 볼 수 없다.
2. **AI는 확률만 낸다.** AI의 출력 타입에는 방향, 수량, 가격 필드가 없다. 임계값 비교와 주문 내용은 Rust `domain`이 계산한다.
3. **바깥 세계를 갈아 끼울 수 있다.** 증권사, AI 제공자, 뉴스 소스, 저장소, 운영체제 기능은 포트 뒤에 있다.
4. **계산은 한 곳에서 한다.** 돈, 비중, 확률, 임계값 계산은 Rust `domain`에만 있다. TypeScript는 표시만 한다.
5. **의존 방향을 도구가 검사한다.** 문서의 규칙이 아니라 빌드 실패로 지킨다.
6. **내부의 복잡함은 사용자에게 보이지 않는다.** 실행 분리, 격리, 작업 큐, 보정 같은 장치의 이름은 화면에 나오지 않는다(P-8). 모든 설정에 기본값이 있다(P-4).

## 2. 시스템 구조

### 2.1 프로세스

```
┌─ 앱 프로세스 (Tauri, 메뉴 막대 상주, 단일 인스턴스) ─────────────────────┐
│  WebView (React)  ⇄  IPC 명령 · ID만 담은 타입 이벤트 · 요청별 Channel  ⇄ 코어 │
│                                                                          │
│  코어: 유스케이스 · 스케줄러(벽시계) · 승인 서비스 · AI 러너               │
│        MCP 서버 (실행마다 Unix 소켓과 토큰, 실행 종류별 도구 묶음)          │
│        SQLite (SQLCipher, 쓰기 스레드 1개) · 데이터 보호 키체인             │
│        네이티브 주문 요약 창 + macOS 본인 확인 (WebView가 그리지 않음)       │
└──────────────────────────────────────────────────────────────────────────┘
   │ HTTPS             │ HTTPS                       │ 자식 프로세스 (프로세스 그룹)
 토스 Open API     공시·RSS·미 재무부·FRED        claude / codex CLI
                                                   ├ 분석 실행: stdio ⇄ mcp-bridge ⇄ Unix 소켓 ⇄ MCP 서버
                                                   └ 수집 실행: 내장 웹 검색 ⇄ 인터넷 (앱 MCP 없음, 웹 읽기는 S-07)
```

| 항목 | 결정 | 근거 |
|---|---|---|
| 앱 프로세스 | 하나. Tauri 2.12.5 | R-05 |
| 창 닫기 | `CloseRequested`에서 `prevent_close` 후 창을 숨긴다. 메뉴 막대 아이콘(`tray-icon`)과 `ActivationPolicy::Accessory`로 상주한다 | I-09, R-05 Q6 |
| 자동 시작 | `tauri-plugin-autostart` 2.7.0 (Launch Agent). 기본값은 켬([design.md](design.md) 8장, P-4). 재부팅 뒤에도 예약 작업이 이어진다. 사용자가 끄면 빠진 날의 일별 평가는 다음 시작 때 다시 계산한다 | R-05, I-09 |
| 중복 실행 | `tauri-plugin-single-instance` 2.5.5. 스케줄러와 승인 서비스가 둘이 되지 않게 한다. 토스 토큰이 하나뿐이라는 점(API-3)과도 맞다 | R-05 |
| 앱이 꺼진 날 | 예약 작업은 돌지 않는다. 일별 평가는 다음 시작 때 일봉과 거래내역으로 다시 계산한다 | I-09 |
| 앱 샌드박스, 업데이터, 공증 | 쓰지 않는다. 앱 샌드박스는 자식 CLI에 상속되어 CLI가 자기 인증에 닿지 못하게 만든다 | R-05 Q5·Q8, R-16 Q3 |

창을 숨긴 상태에서 macOS App Nap이 타이머에 주는 영향은 미확인이다(R-05, S-10). 스케줄러는 벽시계 비교 방식이라 늦게 깨더라도 놓친 작업을 판정할 수 있다(3.7).

### 2.2 AI 실행의 두 종류

보유 내역을 아는 AI에게 웹 검색을 주면 검색어로 수량과 금액이 새어 나갈 수 있다(R-12). 그래서 실행을 둘로 나눈다. 이 구분은 화면에 나오지 않는다.

| 항목 | 수집 실행 | 분석 실행 |
|---|---|---|
| 아는 것 | 종목명과 코드, 주제어. 수량, 금액, 계좌는 모른다 | 보유 내역 전체, 수집 실행이 모은 자료, 코드가 낸 추정 과제 |
| 도구 | CLI 내장 웹 검색만. 웹 읽기는 S-07 | 앱 MCP 도구만. 웹 도구 없음 |
| 앱 MCP | 연결하지 않는다 | 연결한다. 실행 토큰이 "분석" 도구 묶음에만 묶인다 |
| 입력 | 코드가 종목 코드와 주제 목록의 항목으로 프롬프트를 만든다. 분석 실행은 코드가 정한 주제 목록에서 고를 수만 있고, 분석 실행이 쓴 자유 문장은 들어가지 않는다. 예외로 사용자가 채팅에 직접 쓴 문장은 코어가 저장된 메시지 원문을 그대로 넣는다 | 프롬프트 파일 + MCP 도구 결과 |
| 출력 | 구조화 출력(Claude `--json-schema`, Codex `--output-schema`)으로 찾은 항목 목록. 웹 항목의 본문이 필요하면 이 실행 안에서 요약까지 한다. 코어가 파싱해 비신뢰 `CollectedItem`으로 저장한다 | MCP 쓰기 도구 호출(`submit_estimate`, `update_thesis`, `request_web_research`)과 텍스트 |
| 바깥으로 나가는 통로 | 검색 제공자. 새어 나갈 비밀을 갖고 있지 않다 | 웹 도구는 없다. `read_article`은 본문 읽기가 켜진 언론사 RSS 소스 도메인의 주소만 코드가 대신 가져오고, 수집 실행이 웹에서 가져온 주소는 거부한다. 남는 것은 낮은 대역 통로다: `request_web_research`에서 고르는 종목과 주제가 검색어에 드러난다 |
| 누가 띄우나 | 스케줄러(종목별 뉴스 수집), 분석 실행의 `request_web_research` | 스케줄러(브리핑, 감시, 심층 연구), 채팅 |

- 채팅은 분석 실행이다. 사용자가 채팅에서 웹 검색을 시키면 분석 실행이 `request_web_research(종목 코드 목록, 주제 항목)`를 부르고, 코어가 수집 실행을 따로 띄워 결과만 분석 실행에 돌려준다. 사용자가 직접 쓴 문장으로 조사할 때는 코어가 저장된 사용자 메시지 원문을 넘긴다.
- `request_web_research`의 종목은 코어가 이미 아는 종목(보유, 관심, 코드가 뽑은 후보)으로, 주제는 코드가 정한 주제 목록으로 제한하고 호출 빈도를 제한한다. 코드가 만든 검색어는 SEC-9 검사를 통과해야 하고 감사 로그에 남는다. 종목과 주제의 선택으로 정보를 흘리는 낮은 대역 통로는 남는 위험으로 기록한다([design.md](design.md) 9.1).
- 공시(KIND, DART)와 언론사 RSS 수집은 지금처럼 코드가 한다. 소스별로 본문 읽기를 끌 수 있다(`allow_ai_read`).
- Claude Code의 `--restricted`는 웹 읽기(WebFetch)를 없앤다(R-02). 수집 실행에서 웹 읽기를 살리면서 훅·플러그인 차단을 유지하는 옵션 조합은 미확인이다(S-07). 확인될 때까지 수집 실행은 검색 결과의 제목, 주소, 요약만 낸다.
- Codex는 웹 검색에 페이지 읽기가 포함된다. 수집 실행은 `web_search="cached"`, 분석 실행은 `web_search="disabled"`로 띄운다(R-03).

### 2.3 AI CLI 자식 프로세스

AI CLI는 사용자가 이미 설치하고 로그인한 것을 그대로 쓴다(P-2, I-03). 앱 전용 설정 폴더(`CLAUDE_CONFIG_DIR`, `CODEX_HOME`)는 별도 로그인이 필요해서 쓰지 않는다.

**실행 하나의 수명**

| 순서 | 하는 일 | 근거 |
|---|---|---|
| 1 | 실행 기록을 만든다: 실행 ID, 종류(수집·분석), 프롬프트 해시, CLI 버전, 모델 | R-12 |
| 2 | 분석 실행이면 256비트 토큰과 실행별 소켓(0700 폴더 안 0600, 경로 104바이트 이내)을 만든다 | R-04 Q3 |
| 3 | 빈 작업 폴더(0700)를 만든다 | R-16 Q3 |
| 4 | `tokio::process::Command`로 띄운다. `env_clear` 뒤 허용 목록의 환경 변수만 넣는다(분석 실행은 `STOCK_MCP_TOKEN` 추가). 새 프로세스 그룹(`process_group(0)`), `kill_on_drop`, stdin 파이프를 계속 쥔다 | R-05 Q5, R-16 Q3 |
| 5 | 격리 옵션: Claude는 `--restricted --disable-slash-commands`와 R-02의 권한 옵션, Codex는 R-03의 플래그와 셸 스냅샷 금지 | I-03, R-02, R-03 |
| 6 | 시작 점검: Claude는 첫 `/usage` 턴으로 받은 init에서 도구 목록, `mcp_servers[].status == "connected"`, 플러그인·스킬이 비었는지 확인한다. Codex는 `required = true`와 `mcpServerStatus`를 본다. 다르면 실행을 끝낸다 | R-02, R-03, R-04 |
| 7 | 스트림 이벤트를 `AiEvent`로 바꿔 전달한다 | 3.3 |
| 8 | 끝나면 토큰 폐기, 소켓과 작업 폴더 삭제, CLI가 이 작업 폴더 이름으로 만든 기록 폴더 삭제 | I-04 |

**앱 수준의 수명 관리**

- 실행 중인 프로세스 그룹 ID를 기록한다. 앱이 시작할 때 남아 있는 그룹을 정리한다. 부모가 SIGKILL로 죽으면 자식이 살아남는 것을 실험으로 확인했기 때문이다(R-05).
- 앱이 정상 종료할 때(`RunEvent::ExitRequested`) 모든 그룹에 SIGTERM을 보내고 시간 제한 뒤 SIGKILL을 보낸다. 진행 중인 감사 로그 쓰기가 끝날 때까지 종료를 잠깐 미룬다.
- 앱이 죽으면 stdin이 닫힌다. CLI가 stdin EOF에서 곧바로 끝나는지는 미확인이다(R-05, S-11). 시작 시 정리가 이것을 덮는다.
- 셸 플러그인과 sidecar 실행 권한은 WebView에 주지 않는다. CLI는 Rust 코어에서만 띄운다.
- `sandbox-exec` 프로필(앱 데이터 폴더 읽기 금지 등)은 "없어도 안전이 성립하는 추가 겹"으로만 둔다. 폐기 예정 표시가 있고, 샌드박스 안에서 구독 인증이 도는지는 S-04에서 확인한다(R-16).
- 대화 원본은 앱 SQLite에만 둔다. 예약 작업과 채팅은 기록을 남기지 않는 옵션(`--no-session-persistence`, Codex `ephemeral`)으로 돌린다. 채팅은 대화 동안 프로세스 하나를 유지하고, 앱을 다시 켜면 앱이 저장한 대화를 새 프로세스에 다시 넣는다. CLI의 `--resume`에 기대지 않는다(I-04).

### 2.4 MCP 연결

| 항목 | 결정 | 근거 |
|---|---|---|
| 라이브러리 | `rmcp` `=3.9.0`. 두 CLI의 프로토콜 세대(2026-07-28, 2025-06-18)를 모두 받는다 | R-04 |
| 전송 | CLI가 `mcp-bridge`를 stdio MCP 서버로 띄우고, 브리지가 실행별 Unix 소켓으로 바이트를 그대로 넘긴다 | I-01 |
| 인증 | 브리지가 첫 줄로 토큰을 보낸다. 코어는 상수 시간 비교 후 실행 ID에 묶는다. 실행이 끝나면 폐기한다 | R-04 Q3 |
| 보조 확인 | `accept` 직후 상대 uid가 같은지, 상대의 부모 PID가 앱이 띄운 CLI인지, 상대 실행 파일이 앱 번들 안의 브리지인지 본다. PID 재사용 경쟁이 있어 보조 수단이다 | R-16 Q5 |
| 토큰 전달 | 자식 환경 변수로만. MCP 설정 파일에는 `${STOCK_MCP_TOKEN}` 참조만 쓴다. argv에 넣지 않는다. Codex는 `env_vars` 허용 목록이 필요하다 | R-04 |
| 도구 묶음 | 서버가 토큰의 실행 종류를 보고 도구 목록을 정한다. CLI 플래그에만 기대지 않는다 | 2.2 |
| 도구 공통 규칙 | 응답 하나 3만 자 이하, 목록 도구는 `limit`과 불투명 `cursor`, 입력 검증 오류는 `isError:true` 구조화 오류, 읽기 도구에 `readOnlyHint`, `structuredContent`와 text 블록 병행, `tools/list` 결정적 정렬 | R-04 Q4, I-04 |
| 외부 텍스트 | `untrusted_text` 필드로만 넣는다. 제어·폭 0·양방향 문자 제거, 링크 평문화, 길이 상한. 도구 설명, 오류 메시지, 필드 이름에 넣지 않는다 | R-04 Q5 |
| 호출 기록 | 서버의 `call_tool` 앞단에서 실행 ID, 도구, 인자, 결과 해시, 시각을 추가 전용 테이블에 남긴다. CLI 쪽 이벤트와 같은 실행 ID로 대조한다 | R-04 Q6 |

분석 실행의 도구 분류(정확한 목록과 스키마는 design.md 3장):

| 분류 | 예 | 상태 변경 |
|---|---|---|
| 읽기 | 포트폴리오, 거래, 시세, 시장, 뉴스·공시, `read_article`, 수집 결과, 논거 노트, 투자 성향, 성적표(`get_scorecard`), 추정 과제(`get_estimate_task`) | 없음 |
| 추정 제출 | `submit_estimate(task_id, p, 근거, 반대 논거, 출처)` | 추정 기록 하나 |
| 노트 | `update_thesis` | 노트 이력 하나 |
| 웹 조사 요청 | `request_web_research(종목 코드 목록, 주제어)` | 수집 실행 하나 |

주문 전송, 주문 제안 생성, 승인, 설정 변경, 키 접근은 MCP에 없다.

### 2.5 신뢰 경계

[R-16](research/R-16-local-security.md) Q8의 위협 모델을 따른다. T-n은 그 표의 공격자 번호다.

| 영역 | 신뢰 | 할 수 있는 것 | 막는 장치 |
|---|---|---|---|
| 사용자의 물리적 조작 (네이티브 확인 창, Touch ID·암호) | 신뢰 | 주문 승인, 위험 설정 변경 승인 | - |
| 코어 (Rust) | 신뢰 | 전부 | - |
| WebView 안의 스크립트 | 비신뢰 (T-5) | 조회, 일반 설정, 승인 **요청**(`proposal_id`만), 위험 설정 변경 **요청** | 앱 ACL 명령 허용 목록, CSP, 본인 확인 창은 코어가 띄움 |
| 분석 실행 AI CLI | 비신뢰 (T-1, T-2) | 분석 도구 묶음의 MCP 호출 | 웹 도구 없음, 환경 변수 허용 목록, 빈 작업 폴더, 시작 점검, 데이터 보호 키체인 |
| 수집 실행 AI CLI | 비신뢰 (T-1, T-2) | 웹 검색 (웹 읽기는 S-07) | 계좌 정보를 받지 않음, 앱 MCP 없음, 환경 변수 허용 목록, 빈 작업 폴더 |
| `mcp-bridge` | 비신뢰 (CLI가 띄움) | 바이트 전달 | 토큰, 소켓 권한, 상대 확인 |
| 뉴스·웹·공시 내용, 수집 실행 결과 | 비신뢰 (T-1) | 없음. 데이터로만 다룬다 | `untrusted_text`, 정규화 |
| 토스 응답 | 검증 후 신뢰 | 스키마 검증을 통과한 것만 도메인 타입이 된다 | 생성 타입 + 변환 계층 |
| 같은 사용자의 다른 프로세스 | 비신뢰 (T-4) | 없음 | 소켓 0600, 토큰, 데이터 보호 키체인 접근 그룹, 릴리스 서명의 Hardened Runtime |
| 브라우저 웹 페이지 | 비신뢰 (T-3) | 없음 | 열린 포트가 없다 |
| 디스크·백업 사본 | 비신뢰 (T-7) | 없음 | SQLCipher, 키는 `ThisDeviceOnly` |
| root, 커널 | 범위 밖 (T-9) | - | - |

AI CLI 경계의 일부로 다음을 기록한다(R-02, R-03): 자식 환경 변수 허용 목록, 빈 작업 폴더, 상속되는 파일 디스크립터 없음, Codex 셸 스냅샷 금지. 개발용 ad-hoc 빌드는 Hardened Runtime과 접근 그룹이 없으므로 실제 토스 키를 쓰지 않는다(R-16).

### 2.6 핵심 흐름

**동기화**: 스케줄러 → `SyncPortfolio` → `BrokerRead` → 도메인 계산 → 저장 → `PortfolioSynced` → UI가 다시 조회.

**수집**: 스케줄러 또는 `request_web_research` → AI 러너가 수집 실행을 띄움 → 구조화 출력 → 코어가 `CollectedItem`으로 저장(비신뢰).

**추정**: 코어가 코드가 정한 대상(보유, 관심, 코드가 뽑은 후보)마다 `EstimateTask`(T, S, H, 기준 확률, 기준가)를 만든다. `c/(T+S)`가 5%p를 넘거나 변동성 표본이 60거래일 미만이면 만들지 않는다 → 분석 실행이 `get_estimate_task(task_id)`로 과제를 받고 자료를 읽은 뒤 `submit_estimate`로 p와 근거를 낸다 → `SubmitEstimate`가 저장 → `EstimateSubmitted`.

**신호**: `EvaluateSignals`가 p를 보정하고(판정 30건 이후) 임계값과 비교한다(3.4) → 매수 또는 매도 신호면 `SignalRaised` → 주 화면에 카드.

**주문 제안**: 해당 시장이 열려 있으면 `DraftProposal`이 신호와 투자 성향, 시세로 방향, 수량, 지정가를 계산해 `Proposal<Draft>`를 저장한다. 장이 닫혀 있으면 신호만 남고, 장이 열린 뒤 추정이 아직 유효하면 그때 만든다. 제안이 만료돼도 신호는 남고, 사용자가 주문을 누르면 최신가로 다시 만든다(추정 기준가에서 변동성 기준 범위 안일 때만).

**승인**: 사용자가 카드의 승인을 누름 → WebView가 `request_order_approval(proposal_id)` → `approval-command`가 4.4의 순서로 처리 → 감사 로그.

**판정**: 마감 후 작업이 일봉과 1분봉으로 모든 추정(거래하지 않은 것 포함)을 판정한다 → `EstimateResolved` → 성적표와 보정표 갱신.

**채팅**: UI → `SendChatMessage` → 분석 실행 → MCP 도구 → 요청별 `Channel`로 UI에 스트림. 카드는 텍스트에서 파싱하지 않고 코어 객체 ID로만 그린다.

## 3. 코드 구조

### 3.1 저장소 배치

```
stock-copilot/
├ Cargo.toml                가상 작업 공간. [workspace.dependencies], [workspace.lints]
├ rust-toolchain.toml       툴체인 1.99.1 고정 (I-17, harness.md 1.3. Tauri 2.12는 1.90 이상)
├ crates/
│  ├ domain/                순수 타입과 계산. IO, async, 시계, 직렬화 없음
│  ├ application/           유스케이스와 포트(trait). 주문 게이트의 사실 수집. 어댑터를 모름
│  ├ order-execution/       BrokerTrade 포트, 승인 토큰, ApprovedOrder, approve_and_submit (4장)
│  ├ approval-command/      승인 서비스. WebView 요청을 받아 4.4 순서를 실행한다. tauri 비의존
│  ├ adapters/
│  │  ├ broker-toss-schema/ 토스 OpenAPI의 components.schemas에서 typify로 만든 생성물 (보호된 생성 경로)
│  │  ├ broker-toss/        BrokerRead, MarketInfo, 토큰 관리, 속도 제한. GET과 토큰 발급만 한다
│  │  ├ broker-toss-trade/  BrokerTrade 구현(주문 생성만). 쓰기 메서드의 주문 경로는 여기에만 있다
│  │  ├ sources/            NewsSource, Fundamentals(DART), MacroSeries(미 재무부, FRED)
│  │  ├ ai-claude-code/     AiProvider 구현
│  │  ├ ai-codex/           AiProvider 구현 (어댑터 유지, 기본은 꺼짐)
│  │  ├ store-sqlite/       저장소 포트, AuditLog 구현
│  │  ├ secrets-keychain/   SecretStore 구현 (데이터 보호 키체인)
│  │  └ platform-macos/     UserPresence 구현 (네이티브 요약 창 + LocalAuthentication). `unsafe`의 보호된 예외를 둘 수 있는 유일한 crate (4.5)
│  ├ mcp-server/            인바운드 어댑터. 분석 실행이 호출
│  ├ mcp-bridge/            CLI가 띄우는 stdio↔Unix 소켓 브리지 실행 파일
│  ├ scheduler/             예약 작업 레지스트리와 벽시계 트리거
│  ├ testkit/               application 포트의 가짜(가짜 본인 확인 ScriptedPresence 포함), SteppedClock, 가짜 AI CLI 실행 파일
│  ├ trade-fakes/           RecordingBroker (BrokerTrade 가짜). 테스트와 e2e-app 전용
│  ├ toss-smoke/            실제 키를 표준 입력으로 받아 읽기 호출만 해 보는 확인 명령. 키를 저장하지 않는다
│  └ xtask/                 하네스 명령 (arch-check, capability-check, gen-tokens 등)
├ apps/desktop/
│  ├ src-tauri/             lib: build_app<R: Runtime>(builder, Ports, ctx)
│  │  │                     bin main: 실제 어댑터를 만들고 build_app만 부른다
│  │  │                     bin gen-bindings: TS 바인딩 생성 진입점
│  │  ├ ipc/                IPC DTO와 명령 (domain → DTO 변환)
│  │  └ platform/           메뉴 막대, 창 수명, 자식 프로세스 정리, Notifier 구현
│  ├ e2e-app/               E2E 전용 앱. 자동화 플러그인과 가짜 증권사·본인 확인으로 조립
│  └ src/                   React (5장)
├ design/                   Claude Design 산출물 (보호)
├ prompts/                  AI 프롬프트 (제공자 중립)
├ fixtures/                 녹화한 외부 응답 (비식별화), 녹화한 CLI 스트림
└ docs/
```

- `src-tauri`를 lib와 bin으로 나누는 이유: 경로 고정 테스트가 제품과 **같은** `build_app`을 불러야 하기 때문이다(R-15). `main.rs`는 실제 어댑터 생성과 `build_app` 호출만 담는다.
- Tauri CLI가 루트 가상 작업 공간의 `target/`을 제대로 다루는지는 미확인이다(R-05, S-12). H-01에서 확인한다.

### 3.2 의존 규칙

의존 검사는 `xtask arch-check`가 `cargo metadata`(`cargo_metadata` 0.23.1)의 그래프로 단독으로 한다. cargo-deny는 작업 공간 crate 사이의 간선과 전이 경로를 막지 못한다는 것을 실측했다(R-09 Q3). cargo-deny는 서드파티 허용 목록, 라이선스, 취약점, 빌드 스크립트를 맡는다.

**허용 간선 (일반·빌드 의존)**

표에 없는 간선은 실패다. 허용 표 자체가 비순환이므로 전이 의존도 표 안에서만 생긴다.

| crate | 의존할 수 있는 작업 공간 crate |
|---|---|
| `domain` | 없음 |
| `application` | `domain` |
| `order-execution` | `application`, `domain` |
| `approval-command` | `order-execution`, `application`, `domain` |
| `broker-toss-schema` | 없음 |
| `broker-toss` | `broker-toss-schema`, `application`, `domain` |
| `broker-toss-trade` | `broker-toss`, `broker-toss-schema`, `order-execution`, `application`, `domain` |
| `sources`, `ai-claude-code`, `ai-codex`, `store-sqlite`, `secrets-keychain`, `platform-macos` | `application`, `domain` |
| `mcp-server` | `application`, `domain` |
| `mcp-bridge` | 없음 |
| `scheduler` | `application`, `domain` |
| `testkit` | `application`, `domain` |
| `trade-fakes` | `order-execution`, `application`, `domain` |
| `src-tauri` | `domain`, `application`, `approval-command`, `broker-toss`, `broker-toss-trade`, `sources`, `ai-*`, `store-sqlite`, `secrets-keychain`, `platform-macos`, `mcp-server`, `scheduler` |
| `e2e-app` | `src-tauri`(lib), `testkit`, `trade-fakes` |
| `toss-smoke` | `broker-toss`, `application`, `domain` |
| `xtask` | 없음 |

**개발 의존 (별도 표로 검사)**

| crate | 개발 의존으로 허용 |
|---|---|
| `src-tauri` | `testkit`, `trade-fakes`, `tauri`의 `test` 기능 |
| `order-execution` | `testkit`(시계, 저장소, 가짜 본인 확인 `ScriptedPresence`) |
| 그 밖의 crate | `testkit` |

**가짜의 위치 (I-14)**

- `UserPresence`는 `application`의 포트다. 그래서 가짜 본인 확인 `ScriptedPresence`는 `testkit`에 둔다. `testkit`은 `order-execution`에 의존하지 않으므로 `ApprovedOrder`를 만들 수 없다. `order-execution`의 단위 테스트와 `src-tauri`의 경로 고정 테스트가 같은 가짜를 쓴다.
- 보낸 주문을 기록하는 `RecordingBroker`는 `trade-fakes`에 있고 `ApprovedOrder`를 받기만 한다.
- `testkit`과 `trade-fakes`는 `cfg`나 `[features]` 없이 보통 crate로 만든다. harness.md 3.2의 `cfg` 금지와 충돌하지 않는다.
- 제품 실행 파일에 가짜가 들어가지 않는 근거: 개발 의존은 bin 대상에서 쓸 수 없고, `arch-check`가 제품 bin(`src-tauri` main)의 일반 의존 그래프에 `testkit`·`trade-fakes`가 있으면 실패시킨다.
- 승인 없이 주문이 나가지 않는 근거: `ApprovedOrder`는 `approve_and_submit` 안에서 `UserPresence::confirm`이 `Confirmed`를 돌려준 뒤에만 생긴다(4.3). `UserPresence` 구현(`impl UserPresence`)은 `platform-macos`와 `testkit`에만 둘 수 있고 `xtask`가 검사한다. 이 검사를 피하지 못하도록 `UserPresence`를 다른 이름으로 가져오는 것(`use … as`)과 다시 내보내는 것(`pub use`)을 금지하고 `xtask`가 검사한다. `main.rs`는 `platform-macos`의 구현만 넘긴다(RV-6).

**의존할 수 없는 것 (직접·전이 모두)**

| 대상 | 직접 의존할 수 있는 것 | 전이 그래프에도 나타나면 안 되는 crate |
|---|---|---|
| `order-execution` | `approval-command`, `broker-toss-trade`, `trade-fakes` | `domain`, `application`, `mcp-server`, `mcp-bridge`, `scheduler`, `ai-*`, `sources`, `store-sqlite`, `secrets-keychain`, `platform-macos`, `broker-toss`, `broker-toss-schema`, `testkit`, `toss-smoke`, `xtask` |
| `approval-command` | `src-tauri` | `src-tauri`, `e2e-app`를 뺀 전부 |
| `broker-toss-trade` | `src-tauri` | `src-tauri`, `e2e-app`를 뺀 전부 |
| `trade-fakes` | `e2e-app`(일반), `src-tauri`(개발 의존만) | 제품 bin(`src-tauri` main)의 일반 의존 그래프 |
| `testkit` | `e2e-app`(일반), 모든 crate(개발 의존만) | 제품 bin(`src-tauri` main)의 일반 의존 그래프 |
| `broker-toss-schema` | `broker-toss`, `broker-toss-trade` | 다른 crate는 생성 타입을 보지 못한다 |
| 어댑터끼리 | 위 표의 `broker-toss-trade → broker-toss`, `broker-toss* → broker-toss-schema`만 | - |
| `mcp-bridge`, `xtask` | 아무도 의존하지 않는다 | - |

`src-tauri`는 `order-execution`에 **직접 의존하지 않는다**. Rust는 직접 의존이 없는 crate를 이름으로 부를 수 없으므로, 조립 루트 안의 어느 모듈도 `ApprovedOrder`나 `approve_and_submit`을 참조할 수 없다. v0.1의 "승인 명령 모듈 하나로 제한" 검사는 이 그래프 규칙으로 바뀐다(R-09 Q3). 이 규칙이 새지 않도록 `approval-command`, `broker-toss-trade`, `trade-fakes`는 `order-execution`의 항목을 다시 내보내지(`pub use`) 않는다. `xtask arch-check`가 이것을 검사한다.

**외부 crate의 위치**

| 외부 crate | 의존할 수 있는 곳 | 이유 |
|---|---|---|
| `tauri`, `tauri-plugin-*`, `tauri-specta`, `specta`, `specta-typescript` | `src-tauri`, `e2e-app` | 핵심 crate 테스트가 WebView 스택 없이 돈다(R-05) |
| `tauri-plugin-wdio-webdriver` 등 자동화 플러그인 | `e2e-app`만 | HTTP로 앱을 조종할 수 있다(R-15) |
| `rmcp` | `mcp-server`. `testkit`은 클라이언트 기능만 | R-04 |
| `rusqlite`, `libsqlite3-sys` | `store-sqlite` | R-06 |
| `security-framework` | `secrets-keychain` | R-16 |
| `objc2-local-authentication`, `rfd` | `platform-macos` | R-16 |
| HTTP 클라이언트 | `broker-toss`, `broker-toss-trade`, `sources` | 기본 URL을 주입받는다(R-15) |
| `tokio` 프로세스 기능 | `ai-*`, `src-tauri/platform`, `testkit` | R-05 |
| `serde` | `domain`을 뺀 곳 | domain 타입에 직렬화를 붙이지 않고 각 경계가 변환한다(R-05) |
| `rust_decimal`(`maths` 기능), 시간 타입 crate | `domain` 포함 전부 | domain이 의존하는 외부 crate는 이 둘뿐이다 |
| `cargo_metadata`, `syn`, `proc-macro2`, `ra-ap-rustc_lexer`, `toml` | `xtask`만 | 하네스 검사용(harness.md 3.2, 3.5, R-09 6.3). `xtask`는 작업 공간 crate에는 의존하지 않는다 |

**그 밖의 규칙**

- `domain`에는 `async`, 파일·네트워크 접근, 현재 시각 호출, `f32`/`f64`가 없다. 시각은 인자로 받는다. 제곱근, 거듭제곱 같은 통계 연산도 `rust_decimal`의 `MathematicalOps`로 한다(R-11).
- 작업 공간 안의 `build.rs`와 proc-macro crate는 금지한다. 예외는 `apps/desktop/src-tauri/build.rs`(Tauri 앱 ACL 생성) 하나이고 보호 파일이다(R-05, R-09). 이 파일은 명령 목록을 손으로 적지 않고 생성 파일에서 읽는다(3.6).
- 토스 주문 경로는 문자열이 아니라 HTTP 메서드와 경로의 조합으로 검사한다. 쓰기 메서드(POST, PUT, PATCH, DELETE)와 주문·조건주문의 생성·정정·취소 경로(`/api/v1/orders`, `/api/v1/orders/{orderId}/modify`, `/api/v1/orders/{orderId}/cancel`, `/api/v1/conditional-orders`, `/api/v1/conditional-orders/{conditionalOrderId}`, `/api/v1/conditional-orders/{conditionalOrderId}/modify`)는 `broker-toss-trade`에만 나타날 수 있다. 예외는 `broker-toss`의 토큰 발급(`POST /oauth2/token`)이다. 주문·조건주문 조회(GET)는 `broker-toss`가 한다. 1차에서 `broker-toss-trade`는 주문 생성(`POST /api/v1/orders`)만 구현한다. `xtask`가 검사한다.
- 현재 시각과 환경 변수 읽기는 보호된 파일 허용 목록(시계 어댑터 파일 하나, 조립 지점)에서만 할 수 있다. 서드파티 crate의 현재 시각 함수와 `env::var_os`도 막는다. `xtask`가 검사한다([harness.md](harness.md) 3.1).

### 3.3 포트

`application`이 정의하고 어댑터가 구현한다. `BrokerTrade`만 `order-execution`이 정의한다.

| 포트 | 역할 | 현재 구현 | 비고 |
|---|---|---|---|
| `BrokerRead` | 계좌, 보유, 거래내역, 시세, 랭킹, 매수가능금액 | `broker-toss` | 증권사 기능 차이는 `BrokerCapabilities`로 |
| `BrokerTrade` | 주문 생성만. `ApprovedOrder`만 받는다. 정정·취소는 1차 범위 밖이다([design.md](design.md) 6.5) | `broker-toss-trade` | `order-execution`이 정의 |
| `MarketInfo` | 장 운영 시간, 환율, 국내 지수 | `broker-toss` | 미국 지수는 토스에 없다. 장중은 토스의 미국 ETF 시세로 대신한다(I-18) |
| `MacroSeries` | 미국 금리·지수의 일별 값 | `sources`(미 재무부, FRED) | FRED 키가 없으면 그 값만 빠진다(P-5) |
| `NewsSource` | 한 소스의 새 항목 | `sources` | 메타데이터: `retention`, `allow_ai_read`, `supports_conditional_get`, `requires_user_agent` (R-07) |
| `Fundamentals` | 재무제표 | `sources`(DART) | DART 키는 선택 사항 |
| `AiProvider` | `run(RunSpec)`, `resume`, `usage`, `interrupt`, `verify_isolation` | `ai-claude-code`, `ai-codex` | 실행 파일 경로를 주입받는다(R-15). `RunSpec`에 실행 종류가 들어간다 |
| 저장소 포트 | 도메인 객체 저장·조회 | `store-sqlite` | - |
| `AuditLog` | 추가만 되는 기록, 변조 탐지 | `store-sqlite` | 6.3 |
| `SecretStore` | 비밀 값 | `secrets-keychain` | 비밀은 `secrecy` 래퍼. `Debug`, `Display`, `Serialize`, `Clone` 없음(R-16 Q4) |
| `Clock` | 현재 벽시계 시각 | 시스템 | 테스트는 `SteppedClock` |
| `Notifier` | macOS 알림 | `src-tauri/platform`(`tauri-plugin-notification` 2.8.1, Rust에서만 호출) | - |
| `UserPresence` | `confirm(ConfirmationRequest) → Confirmed / Denied / Unavailable` | `platform-macos` | 요청은 주문 요약 또는 위험 설정 변경. 주문은 네이티브 요약 창 + macOS 본인 확인, 위험 설정 변경은 네이티브 확인 창만(SEC-12, P-9). 4.5 |

- 스트림 이벤트 `AiEvent`: `TextDelta`, `ToolCall{server, tool, args, result, status}`, `WebSearch{query, action}`, `TokenUsage`, `Quota`, `RateLimited{resets_at}`, `ModelRerouted`, `McpUnavailable`, 그리고 모르는 type을 담는 명시 변형 `Unknown`(R-02, R-03). 녹화한 스트림을 테스트 고정 자료로 쓴다.
- 뉴스 소스와 예약 작업은 레지스트리에 등록한다. 새 소스나 작업은 구현 하나와 등록 한 줄로 추가된다. 경로 고정 테스트는 이 레지스트리에서 대상 목록을 가져온다(4.6).

#### 읽기 포트의 메서드 계약 (ADR 0055)

현재 `BrokerRead`는 `accounts`, `holdings`, `transactions`, `quotes`, `candles`, `buying_power`를, `MarketInfo`는 `fx_rate`, `order_fx_rates`를 정의한다. 입출력은 domain 타입이고 오류는 본문과 비밀을 담지 않는 `PortError`(닫힌 원인, 메서드별 엔드포인트, HTTP 상태 분류, 재시도 대기 시간)다. 보유와 현금은 따로 조회하고 `Portfolio` 조립은 A-06이 맡는다. 거래내역은 완전 증거 또는 받은 행과 격리 원문을 보존한 불완전 결과를, 시세는 종목별 결과(한 번에 1~200개)를 돌려준다. 페이지, 재시도, 속도 제한은 어댑터 안의 일이다. 포트는 `Send + Sync`이고 메서드는 `Send` Future를 돌려주며 `dyn`을 지원하지 않는다. 조립 루트가 제네릭으로 주입한다.

매도가능수량, 주문 조회, 매수 유의사항, 종목 마스터, 호가, 가격 제한, 수수료율, 지수, 랭킹, 수급, 섹터, 시장을 닫힌 값으로 받는 `calendar`는 **정의 대기**다. domain 타입을 만드는 선행 작업(D-33~D-37) 뒤에 A-20이 더한다. 그때까지 `quotes`와 `candles`의 종목 입력, `accounts`의 계좌 행은 임시 계약이다.


### 3.4 도메인 모델

| 묶음 | 주요 타입 |
|---|---|
| 값 | `Money`(통화 + 십진수), `Quantity`, `Price`, `Ratio`, `Probability`(0~1 십진수. AI 입력은 1~99 정수 퍼센트만 받는다), `Currency`, `Market`, `Symbol`(영숫자 문자열, R-07), `InstrumentId`(코드 변경과 분리, R-11), `AccountId`, `ClientOrderId` |
| 포트폴리오 | `Holding`, `Portfolio`, `Allocation`, `Concentration`, `SectorAssignment` |
| 원가와 현금 | `Lot`, `CostBasisMethod{MovingAverage, Fifo}`, `CashLedger`(신뢰 상태 포함), `ExternalFlow` |
| 거래 | `Transaction`, `TransactionKind`, `SettledRowSet`(같은 행의 개수까지 맞추는 멀티셋, 사라진 행은 표시, I-06), `CorporateAction{Split, ReverseSplit, BonusIssue, StockDividend, SymbolChange, Delisting}` |
| 평가 | `ValuationSnapshot`(일별, 환율 포함), `Performance`(TWR, MWR), `BenchmarkSeries` |
| 시장 | `Quote`, `Candle`, `MarketSession`, `FxRate{kind: Mid / TossBuy / Transaction, as_of, source}`(합산 기본은 `Mid`), `Volatility` |
| 정보 | `NewsItem`, `Disclosure`(키는 `(source, receipt_no)`), `Cluster`, `Thesis`, `CollectedItem`(수집 실행 산출, 비신뢰) |
| 투자 성향 | `InvestorProfile{Horizon, RiskAppetite, Exclusions, WeightCaps}`, `Policy`, `PolicyViolation` |
| 추정 | `EstimateTask{symbol, target T, stop S, horizon H, base_probability, base_price, asked_at}`, `Estimate{task, p, rationale, counterpoints, sources, model, run}`, `EstimateOutcome{Hit, StoppedOut, Expired}`, `Calibration`(모델별·기간별 적중표) |
| 신호 | `TradeCost`(왕복 수수료와 세금 c), `Margin`, `Thresholds{base, breakeven, buy, sell}`, `SignalDecision{Buy, Sell, None}`, `Signal` |
| 주문 | `PositionSize`, `OrderIntent`, `Proposal<Draft>`, `Proposal<Validated>`, `ValidationReport`, `OrderSummary`, `SubmittedOrder`, `OrderOutcome{Accepted, Rejected, Unknown}` |
| 성적 | `Scorecard`(Brier 점수, 구간별 적중률, 신호를 따랐을 때의 수익률) |
| 이벤트 | `DomainEvent` (열거형) |

v0.1의 `Idea`, `ProposalOutcome`은 없앤다. 장이 닫혀 있을 때의 신호가 아이디어의 자리를 대신하고, 성적은 추정 단위로 매긴다.

**확률 신호 계산 (전부 `domain`의 순수 함수)**

| 계산 | 입력 | 결과 | 근거 |
|---|---|---|---|
| 추정 과제 만들기 | 코드가 정한 대상 종목, `InvestorProfile`, 종목의 `Volatility`, `MarketSession`, 기준가, `TradeCost` | `EstimateTask`(T = a·σ√H, S = b·σ√H, H, 기간 제한을 넣은 기준 확률 p₀) 또는 과제 없음(`c/(T+S)` > 5%p, 변동성 표본 60거래일 미만) | open-issues 1.1, 1.2 |
| 임계값 | 과제, `TradeCost`, `Margin` | 손익분기 `p₀ + c/(T+S)`, 매수 = 손익분기 + 여유분, 매도(보유 종목) = `p₀ − 여유분`(하한 5%) | open-issues 1.2 |
| 여유분 | 판정된 추정 수(같은 종목에서 기간이 겹치지 않는 것만), 모델, `RiskAppetite` | 그 모델의 판정이 30건 미만이면 15%p, 그 뒤는 성향별 값 | open-issues 1.4 |
| 보정 | p, `Calibration` | 30건 이후 실제 적중률로 보정한 p. 표본이 적을 때 기준 확률 쪽으로 당기는 방식(E-91의 ADR) | open-issues 1.4 |
| 신호 판정 | 보정한 p, `Thresholds`, 보유 여부, `Exclusions` | `SignalDecision`. 매도는 보유 종목에만 | open-issues 1.2 |
| 수량과 가격 | 신호, 포트폴리오, `WeightCaps`, `RiskAppetite`, 시세 | `OrderIntent`(방향, 수량, 지정가). 30건 미만이면 한 번에 사는 비중을 작게 | open-issues 1.3, 1.4 |
| 결과 판정 | 과제, 기준 시각 이후의 수정주가 일봉, 기준일과 목표·손절에 모두 닿은 날의 1분봉 | `EstimateOutcome`. 기준가는 같은 수정 계수로 맞춘다. 1분봉으로도 순서를 알 수 없으면 실패. 몇 시간 단위 기간은 1분봉만(design 6.6). 사람과 AI의 해석이 끼지 않는다 | open-issues 1.1 |
| 가상 청산 | 과제, 같은 봉 입력 | 신호를 따랐을 때의 수익률: 목표·손절가, 그 가격을 넘겨 시작한 날은 시가, 만기는 종가(design 6.6) | design 6.6 |

- AI가 내는 입력 타입(`submit_estimate`)에는 과제 ID, p, 근거, 반대 논거, 출처만 있다. AI가 방향, 수량, 가격을 전달할 통로가 타입에 없다.
- T, S, H와 기준 확률은 AI 실행 전에 코드가 정하고 과제(`get_estimate_task`)로 보여 준다. AI는 `task_id`로만 과제를 받고 제출한다. 과제가 없는 종목은 추정할 수 없다.
- 몇 시간 단위 기간은 기본값에서 꺼 둔다(open-issues 1.2).
- 실제 돈을 넣기 전에 추정만 기록하는 기간을 둔다(S-06). 주문 기능 스위치는 처음에 꺼져 있다(ORD-5).

**규칙**

- 원시 타입을 그대로 쓰지 않는다. 종목 코드, 계좌, 금액, 확률뿐 아니라 개수, 시각, 문구까지 모든 값이 전용 타입이다. `bool`도 쓰지 않고 열거형으로 쓴다. 원시 타입은 경계 어댑터 안에서만 나타나고 받자마자 전용 타입으로 바뀐다. `xtask raw-types`가 검사한다 ([harness.md](harness.md) 3.4).
- 통화가 다른 `Money`는 더할 수 없다. 합산하려면 `FxRate`를 명시적으로 넘긴다.
- 십진수 연산은 `checked_*`만 쓴다. `rust_decimal`의 연산자는 넘치면 panic한다(R-09 Q5).
- 저장과 중간 계산에서는 표시 자릿수로 반올림하지 않는다. 표시할 때만 반올림한다(R-11).
- 더하기, 빼기, 곱하기는 정확해야 한다. 결과를 정확히 표현할 수 없으면 오류다(ADR 0025, 0029).
- 나눗셈은 유한 십진수로 표현되지 않을 수 있다(1/3). 그때는 표현할 수 있는 최대 자릿수에서 끊고, 근사라는 사실을 타입에 남긴다. 근사 값은 금액이 될 수 없다(ADR 0029).
- `Decimal`의 곱·나눗셈·반올림 메서드는 구조 검사의 허용 파일에서만 쓴다.
- 상태가 있는 것은 열거형이나 타입 상태로 표현하고, 불가능한 상태를 만들 수 없게 한다.
- 외부 열거값(토스 거래 유형 등)은 명시 변형 `Unknown(String)`을 갖는다. 와일드카드 match를 쓰지 않기 위해서다(R-01, R-09).

### 3.5 이벤트와 UI 전달

코어 안의 일은 `DomainEvent`로 알린다. 스케줄러 트리거와 UI 갱신이 모두 이것을 구독한다. 새 반응을 추가할 때 기존 코드를 고치지 않고 구독자를 추가한다.

다음은 닫힌 목록이다. `DomainEventKind`는 코드에 있는 각 변형과 같은 이름으로 일대일 대응하고 `DomainEvent::kind()`로 얻는다. 정의 대기 행은 코드와 종류 열거형에 넣지 않는다. 문서-코드 검사는 이름과 페이로드 타입, 전체 행의 중복, 고정된 정의 대기 이름 집합을 대조한다.

각 이벤트는 상태 변경이 저장·반영된 뒤 내는 변경 알림이며 승인이나 주문 전송 권한이 아니다. 페이로드는 기존 식별자 또는 닫힌 종류뿐이다. 종목·계좌 키를 받는 구독자는 그 키에 속한 현재 상태와 기록 목록을 다시 읽는다. 개별 신호·추정·스냅샷·외부 주문 ID가 없어도 이 조회 범위로 회복한다. 전역 단일 자원은 ID 없이 종류만 보내고 해당 전역 상태를 다시 읽는다.

| 변형 | 싣는 ID·종류 | 내는 곳·발생 조건 | 구독자 | 근거 | 정의 대기 여부 | 소유 작업 | 키 재조회 방식을 쓰지 않은 이유 |
|---|---|---|---|---|---|---|---|
| `PortfolioSyncRecorded` | `AccountId` | A-06이 성공·일부 실패·실패·접근 차단 어느 동기화 결과든 저장했을 때 | 스케줄러, UI 갱신 | A-06의 성공·일부 실패·접근 차단 경로, design 5.2 | 아니오 | - | - |
| `TransactionsSyncRecorded` | `AccountId` | A-07이 성공·일부 실패·실패·접근 차단 어느 대조 결과든 저장했을 때 | UI 갱신 | A-07, design 5.2의 14일 창 대조와 사라진 행 표시 | 아니오 | - | - |
| `PriceUpdated` | `InstrumentId` | 시세 수집기가 새 시세를 저장했을 때 | UI 갱신 | design 5.2, U-13 | 아니오 | - | - |
| `PriceMoved` | `InstrumentId` | N-12가 보유 종목 하루 변동 기준 충족을 기록했을 때 | 스케줄러, UI 갱신 | design 5.3의 하루 ±5% | 아니오 | - | - |
| `DisclosureArrived` | `DisclosureKey` | 공시 수집기가 신규·정정 공시 원시 도착을 저장했을 때 | UI 갱신 | design 5.4의 공시 저장; 등급 판정은 N-12만 수행 | 아니오 | - | - |
| `NewsActivityIncreased` | `InstrumentId` | N-12가 같은 종목 뉴스 급증을 기록했을 때 | 스케줄러, UI 갱신 | design 5.3의 1시간 5건 | 아니오 | - | - |
| `ProposalCreated` | `ProposalId` | A-14가 새 주문 제안을 저장했을 때 | UI 갱신 | A-14, design 6.4 | 아니오 | - | - |
| `ProposalExpired` | `ProposalId` | 예약 점검이 시간·가격 범위 만료를 기록할 때 | UI 갱신 | design 6.5의 10분·변동성 범위 만료; 주체는 보수적 초안, O-04 인계 | 아니오 | - | - |
| `ProposalApproved` | `ProposalId` | 승인 서비스가 본인 확인 성공을 감사 기록했을 때 | UI 갱신 | architecture 4.4, SEC-8 | 아니오 | - | - |
| `ProposalRejected` | `ProposalId` | 승인 서비스가 사용자 거절·확인 취소를 기록했을 때 | UI 갱신 | SEC-8, O-04 | 아니오 | - | - |
| `ProposalSkipped` | `ProposalId` | 넘기기 유스케이스가 사용자 선택을 저장했을 때 | UI 갱신 | design 6.5, E-10 | 아니오 | - | - |
| `OrderSubmitted` | `ClientOrderId` | 주문 실행이 전송 사실을 기록했을 때 | 스케줄러, UI 갱신 | design 5.2의 주문 직후 동기화, ORD-2 | 아니오 | - | - |
| `OrderOutcomeUnknown` | `ClientOrderId` | 주문 실행이 전송 결과 미확정을 저장했을 때 | 스케줄러, UI 갱신 | ORD-2, O-19 | 아니오 | - | - |
| `BrokerConnectionChanged` | 없음 | 토스 연결 관리가 차단·만료·사용자 조치 필요·차단 해제·재발급 성공 등 연결 상태 변경을 저장했을 때 | 스케줄러, UI 갱신 | design 2.6의 1~3번, API-3, REL-1 | 아니오 | - | - |
| `SettingsChanged` | 없음 | 설정 유스케이스가 일반·성향·예약·알림 설정 변경을 저장했을 때 | 스케줄러, UI 갱신(설정을 전부 다시 읽음) | design 8장, A-17, J-10, N-14 | 아니오 | - | - |
| `PolicyChanged` | 없음 | 원칙 설정이 변경을 저장했을 때 | 스케줄러, UI 갱신 | design 6.2, 8장 | 아니오 | - | - |
| `OrderSwitchChanged` | 없음 | 설정 유스케이스가 주문 켜기·끄기를 저장했을 때 | UI 갱신 | ORD-5, O-06 | 아니오 | - | - |
| `WatchlistChanged` | 없음 | J-09가 관심 종목 목록 변경을 저장했을 때 | 스케줄러, UI 갱신 | J-09의 수집·감시·추정 대상 반영 | 아니오 | - | - |
| `ThesisUpdated` | `InstrumentId` | J-02가 해당 종목 논거 노트 차분·이력을 저장했을 때 | UI 갱신 | J-02, J-08 | 아니오 | - | - |
| `SelfCheckChanged` | 없음 | A-19가 토스 연결 밖의 자체 점검 항목 변경을 저장했을 때 | UI 갱신 | design 2.6의 4~8번, U-21, U-27; 토스 연결 사건은 제외 | 아니오 | - | - |
| `MarketSessionChanged` | `KnownMarket` | 장 운영 조회가 개장·마감 전이를 저장했을 때 | 스케줄러, UI 갱신 | A-14, design 6.4; KnownMarket은 닫힌 시장 종류 | 아니오 | - | - |
| `SignalRaised` | `InstrumentId` | A-13이 해당 종목의 새 신호를 저장했을 때 | UI 갱신, 알림 | A-13의 종목별 재계산, UX-3 | 아니오 | - | - |
| `SignalCleared` | `InstrumentId` | A-13이 해당 종목의 신호 해제를 저장했을 때 | UI 갱신 | A-13; 종목의 현재 신호를 다시 읽음 | 아니오 | - | - |
| `EstimateSubmitted` | `InstrumentId` | A-12가 해당 종목의 추정 제출을 저장했을 때 | 스케줄러, UI 갱신 | A-12; 종목의 추정들을 다시 읽음 | 아니오 | - | - |
| `EstimateResolved` | `InstrumentId` | A-15가 해당 종목의 최종 판정과 보정표를 저장했을 때 | UI 갱신 | A-15, ACC-9; 종목의 판정들을 다시 읽음 | 아니오 | - | - |
| `ValuationSnapshotTaken` | `AccountId` | A-16이 해당 계좌의 일별 조회·재계산 기록을 저장했을 때 | UI 갱신 | A-16, J-11; 계좌의 평가 기록을 다시 읽음 | 아니오 | - | - |
| `OrderRejected` | `ClientOrderId` | 주문 실행이 이 앱이 낸 주문의 거절·사전 요건 실패를 저장했을 때 | UI 갱신 | design 2.6의 8번, ORD-2; 이 앱의 주문 기록을 다시 읽음 | 아니오 | - | - |
| `OrderOutcomeResolved` | `ClientOrderId` | O-19가 미확정 결과를 대조로 확정하거나 확인 불가로 굳혔을 때 | 스케줄러, UI 갱신 | ORD-2, O-19의 하나로 확정되지 않으면 확인 불가와 같은 종목 새 주문 차단 | 아니오 | - | - |
| `OrderFilled` | `AccountId` | 주문 조회·웹소켓 대조가 해당 계좌의 체결 변화를 저장했을 때 | 스케줄러, UI 갱신 | T-17, design 5.2; 이 앱 밖에서 낸 주문도 포함, 계좌 주문을 다시 읽음 | 아니오 | - | - |
| `OrderCancelled` | `AccountId` | 주문 조회·웹소켓 대조가 해당 계좌의 외부·DAY 취소 변화를 저장했을 때 | 스케줄러, UI 갱신 | design 6.5; 이 앱 밖에서 낸 주문도 포함, 계좌 주문을 다시 읽음 | 아니오 | - | - |
| `OrderModified` | `AccountId` | 주문 조회·웹소켓 대조가 해당 계좌의 외부 정정 변화를 저장했을 때 | 스케줄러, UI 갱신 | design 6.5; 이 앱 밖에서 낸 주문도 포함, 계좌 주문을 다시 읽음 | 아니오 | - | - |
| `CollectionFinished` | `InformationSource` | 수집 실행기가 소스 종류별 수집 결과를 저장했을 때 | 스케줄러, UI 갱신 | design 4.4, AI-14; 기존 닫힌 InformationSource로 해당 종류의 수집 결과를 다시 읽음 | 아니오 | - | - |
| `NewsClusterGrew` | `InstrumentId` | N-09가 해당 종목의 사건 묶음 증가를 저장했을 때 | UI 갱신 | design 5.4의 같은 종목·48시간 묶음; 종목의 사건 묶음들을 다시 읽음 | 아니오 | - | - |
| `AiBudgetChanged` | 없음 | AI 작업 큐가 예약 비율 도달·구독 한도 초과·재개 등 예산 상태 변경을 저장했을 때 | 스케줄러, UI 갱신(고급 상태) | design 4.3의 한도에 걸리면 대기하고 재개 시각에 이어 감, AI-2, REL-4; 전역 예산을 다시 읽음 | 아니오 | - | - |
| `AccountSelectionChanged` | 없음 | U-22가 선택 계좌 변경을 저장했을 때 | 스케줄러, UI 갱신(계좌 키를 쓰는 모든 조회 무효화) | design 2.5의 계좌가 여럿이면 고급 설정에서 바꿀 수 있음, U-22 | 아니오 | - | - |
| `StoredDataChanged` | 없음 | U-15 전체 삭제 또는 S-11 백업 복구 절차가 저장소 교체를 완료했을 때 | 스케줄러, UI 갱신(저장 데이터 조회 전체 무효화) | design 8장 시스템의 전체 삭제, U-15, S-11의 복구 테스트; 새 복원 화면을 추가하는 계약이 아님 | 아니오 | - | - |
| `FxRateUpdated` | `FxPair` | 환율 수집·캐시가 해당 통화쌍의 환율을 저장했을 때 | UI 갱신(해당 통화쌍의 모든 환율 종류 재조회) | ACC-3의 쓴 환율 종류·시각 표시, T-08, S-04, A-08 | 아니오 | - | - |
| `DisclosureTriggerRecorded` | `DisclosureKey` | N-12가 보유 종목의 긴급·높음 공시 기준 충족을 판정해 기록했을 때 | 스케줄러, UI 갱신, 알림(기록된 긴급만) | design 5.3의 보유 종목 긴급·높음 공시 즉시, 정정 한 단계 낮춤; UX-3 긴급 공시 알림 | 아니오 | - | - |
| `AiJobFinished` | 미정(AI 작업 ID) | AI 실행기가 작업 종료 결과를 저장했을 때 | 스케줄러, UI 갱신 | architecture 2.3의 실행 ID, AI-7 동시 채팅·예약 실행 | 정의 대기 | AI-17 | 동시 실행이 둘이므로 단일 현재 작업 키가 없고, 어느 실행의 후속 단계를 이어갈지 구별할 실행 ID·조회 계약이 필요함 |
| `BriefingFinished` | 미정(브리핑 ID) | J-03이 브리핑 결과를 저장했을 때 | UI 갱신, 알림 | UX-3, J-07의 현재·지난 브리핑 카드 | 정의 대기 | J-03 | 현재·지난 브리핑이 함께 남아 최신 카드 조회만으로 알릴 완료 기록을 특정할 수 없음; 브리핑 ID와 완료 조회 계약 필요 |
| `SourceStatusChanged` | 미정(통합 소스 종류) | 수집기가 소스별 성공·실패·마지막 성공을 저장했을 때 | UI 갱신(고급 상태) | REL-5, N-01 소스 레지스트리 | 정의 대기 | N-01 | InformationSource는 웹·뉴스·공시만 구분해 재무·미 재무부·FRED 등 전체 소스를 식별하지 못함; 통합 소스 종류와 상태 조회 계약 필요 |

토스 연결의 한 상태 변경에는 `BrokerConnectionChanged` 하나만 낸다. 차단·만료·사용자 조치 필요뿐 아니라 차단 해제·재발급 성공도 같은 이벤트다. 토스 연결 상태가 바뀌었다는 이유로 `SelfCheckChanged`를 함께 내지 않는다. `SelfCheckChanged`는 토스 연결 밖의 자체 점검 항목에만 쓴다. 동기화 결과 저장과 연결 상태 변경이 각각 실제 발생하면 각각의 사건이며, 오류를 관측한 모든 호출마다 연결 이벤트를 중복 발행하지 않는다.

`PortfolioSyncRecorded`·`TransactionsSyncRecorded`는 성공 여부를 이름으로 판정하지 않는다. 성공·일부 실패·실패·접근 차단 어느 결과든 저장하면 하나를 내고 구독자가 결과를 다시 읽는다. 계좌 선택 변경은 `AccountSelectionChanged`로 계좌 키를 쓰는 모든 조회를 무효화하며, 전체 삭제·백업 복구의 저장소 교체는 `StoredDataChanged`로 저장 데이터 조회 전체를 무효화한다. 단순 백업 파일 생성은 저장소 교체가 아니므로 이 이벤트를 내지 않는다. `SettingsChanged` 구독자는 설정을 전부 다시 읽는다. `AiBudgetChanged`는 예약 작업 비율 도달·구독 한도 초과·재개를 전역 예산 상태 변경 하나로 알리며 주 화면 알림을 내지 않는다.

공시 등급과 트리거 판정은 N-12 한 곳에 둔다. `DisclosureArrived`는 원시 도착의 UI 갱신만 알린다. N-12가 보유 여부·긴급/높음·정정 한 단계 하향을 판정해 충족 사실을 저장한 뒤 `DisclosureTriggerRecorded`를 낸다. 구독자는 그 판정 기록을 읽으며 등급을 다시 계산하지 않는다. 스케줄러는 기록된 긴급·높음에 반응하고 알림은 UX-3의 기록된 긴급에만 반응한다. 가격·뉴스도 N-12가 기록한 기준 충족 사실로 깨운다. 알림은 UX-3의 새 신호·긴급 공시·브리핑 완료만 수행한다.

`ProposalApproved`는 감사 사실일 뿐 승인 상태나 전송 권한이 아니다. 승인과 전송은 기존 경로에서 한 번에 끝낸다. `ProposalRejected`는 사람의 확인 거절, `OrderRejected`는 이 앱이 낸 주문의 거절이다. `OrderOutcomeResolved`는 미확정 결과의 대조 확정 또는 확인 불가 확정을 알린다. `OrderFilled`·`OrderCancelled`·`OrderModified`는 이 앱 밖에서 낸 주문까지 포함한 해당 계좌의 관측 변화이며 구독자가 계좌 주문 목록을 다시 읽는다. 이 앱의 정정·취소 전송 기능을 추가하지 않는다. 킬 스위치는 ORD-5의 `OrderSwitchChanged`로 표현한다.

UI 이벤트 DTO에는 종류와 **계좌 키가 아닌 ID의 문자열 표현**만 싣는다. domain 이벤트의 `AccountId`는 UI DTO에서 제외한다. 계좌는 하나만 선택되므로 UI는 선택 계좌 조회를 무효화하고 다시 읽는다. accountSeq를 문자열로 바꾸어 보내지도 않는다(ADR 0022의 숫자 키를 화면이나 AI에 보내야 한다는 뜻은 아니다, ADR 0016의 i64 내보내기 거부). 문자열 표현은 경계 변환이며 domain의 ID 타입을 바꾸지 않는다. 닫힌 소스·시장·통화쌍 종류의 DTO 표현과 DTO 타입 자체는 U-02가 정한다. 종류·식별자 외의 금액, 수량, 가격, 계좌번호, 종목 이름, AI 글은 싣지 않는다. 실패 원인·재개 시각도 재조회한다. 직렬화·IPC 변환은 A-10/U-02에 인계하며 domain에는 직렬화 파생을 추가하지 않는다.

| 전달 대상 | 방식 | 근거 |
|---|---|---|
| 도메인 알림 → UI | tauri-specta 타입 이벤트. 페이로드는 ID와 종류만. UI는 `shared/platform`의 구독 하나에서 쿼리를 무효화하고 다시 읽는다. 이벤트를 놓쳐도 다음 조회로 회복된다 | R-05 Q4, R-08 |
| 채팅 스트림 → UI | 요청마다 `tauri::ipc::Channel<ChatChunk>`. `TextDelta`, `ToolCallStarted`, `ToolCallFinished`, `Card{kind, id}`, `Sources`, `Done`, `Failed` | R-05, R-14 |
| UI → 코어 | IPC 명령만. 코어는 WebView가 쏜 이벤트를 받지 않는다(`core:event:allow-emit` 권한 없음) | R-05 |

- `Channel`의 `send` 성공은 UI가 받았다는 뜻이 아니다(창 교체 시). 대화 원본은 코어가 SQLite에 저장하고, UI가 새로 고쳐지면 저장분을 읽은 뒤 새 Channel로 다시 구독한다.
- 채팅 카드(신호, 주문 제안, 보유, 시세)는 마크다운에서 파싱하지 않는다. 코어 이벤트의 ID로만 그리고 내용은 그 ID로 다시 읽는다. AI 글로 가짜 카드를 만들 수 없다(R-14 2.6).

### 3.6 IPC와 타입 생성

| 항목 | 결정 | 근거 |
|---|---|---|
| 생성 도구 | `tauri-specta` `=2.0.0-rc.28`, `specta-typescript` `=0.0.15`. `ErrorHandlingMode::Result`를 명시한다 | R-05 Q3 |
| 생성 진입점 | `src-tauri`의 `gen-bindings` 바이너리. 앱 시작 시 생성하지 않는다 | R-05 |
| 생성물 | `shared/ipc/bindings.ts`, 테스트용 `MockCommandHandlers`, 명령 목록 파일, capability의 명령 허용 항목. 생성 경로 목록에는 있지만 보호 목록에는 넣지 않는다. 에이전트가 `gen-bindings`로 다시 만들고, `just check`와 CI가 다시 생성해서 `git diff --exit-code`가 0이 아니면 실패한다 | R-05, R-15 |
| DTO 위치 | `src-tauri/ipc`. domain → DTO 변환. 금액은 십진수 문자열. 큰 정수(`u64` 등)는 생성이 거부한다 | R-05 |
| 입력 DTO | `deny_unknown_fields`. 승인 명령은 `proposal_id`만 받는다 | R-15, R-16 |
| 명령 권한 | `build.rs`의 `AppManifest::commands`로 앱 ACL을 켠다. `build.rs`는 보호 파일이지만 명령 목록을 손으로 적지 않고 생성된 명령 목록 파일에서 읽는다. 그래서 명령을 하나 추가해도 보호 파일을 바꿀 필요가 없다. capability는 `main` 하나를 `tauri.conf.json`에 명시한다. capability의 명령 허용 항목(`allow-*`)은 같은 목록에서 생성하고, 그 밖의 권한은 손으로 쓴 보호 파일에만 둔다. `core:default`, `core:event:allow-emit`, `shell:*`, `dialog:*`, `opener:*`, `fs:*`, `http:*`, `biometric:*`, `notification:*`, `autostart:*`, `remote`는 WebView에 주지 않는다 | R-05 Q1, R-16 Q6 |
| 일치 검사 | `xtask capability-check`: 생성된 명령 목록과 capability의 명령 허용 항목이 같아야 하고, 위험한 권한(위 금지 목록)이 있으면 실패한다 | R-05 |
| TS 경계 | `shared/ipc`가 Tauri 호출의 거절을 한 곳에서 `Result`로 바꾸고, 응답과 오류를 Zod `safeParse`로 다시 검증한다. ACL 거부도 명령 오류처럼 보일 수 있기 때문이다 | R-05, R-08 |

### 3.7 스케줄러와 시계

| 규칙 | 내용 | 근거 |
|---|---|---|
| 시계 | 모든 시각은 `Clock` 포트의 벽시계 값이다 | R-05 Q9 |
| 깨어나는 방식 | 짧은 주기(예: 30초)로 깨어 다음 작업 시각과 벽시계를 비교한다. `Instant` 기반 장시간 sleep은 쓰지 않는다. macOS의 `Instant`는 잠자는 동안 멈추는 시계라서, 노트북이 잠들면 그만큼 늦게 깬다 | R-05 Q9 |
| 놓친 작업 | 작업마다 정책을 둔다: 한 번만 따라잡기, 또는 건너뛰기. 지난 브리핑을 여러 개 만들지 않는다(REL-3) | design REL-3 |
| 장 시각 | 개장·마감은 토스 장 운영 정보에서 읽는다. 서머타임과 휴장일을 직접 계산하지 않는다 | design 4.3 |
| 레지스트리 | 작업은 레지스트리에 등록한다. 경로 고정 테스트가 모든 작업을 고정 시계로 한 바퀴 돌린다 | R-15 |
| 쓰지 않는 것 | `tokio-cron-scheduler`. 시스템 시계를 직접 써서 `Clock`을 주입하기 어렵다 | R-05 |
| 테스트 | `SteppedClock`. tokio `start_paused`는 실제 자식 프로세스를 기다리는 테스트에서 쓰지 않는다(거짓 타임아웃) | R-15 |

새로 생기는 예약 작업: 일별 평가 스냅샷(마감 후), 추정 판정(마감 후), 장중 스냅샷 다운샘플과 `incremental_vacuum`(6.3), 토스 스펙 변경 감시(I-08: 공식 `versions.json`, 파일 `Last-Modified`, 정규화 해시).

### 3.8 오류

- Rust: 함수는 `Result<T, E>`를 반환한다. `E`는 crate별 열거형이다. `panic`, `unwrap`, `expect`는 금지다.
- 오류는 세 종류다: 사용자가 고칠 수 있음, 기다리면 됨, 버그. 토스 오류는 [R-01](research/R-01-toss-open-api.md) Q7의 오류표 분류를 그대로 쓴다. `token-revoked`가 반복되면 "사용자가 고칠 수 있음(같은 키를 다른 도구에서 씀)"이다. 오류 파서는 `requestId`와 `referenceId`를 둘 다 받는다.
- 주문의 "결과 모름"은 오류가 아니라 `OrderOutcome::Unknown` 상태다. 10분 안에는 같은 키로 다시 요청하고, 그 뒤에는 (종목, 방향, 가격, 수량, 시각)으로 대조한다(I-07).
- AI 한도 초과는 오류가 아니라 대기 상태다(REL-4).
- 오류 값에는 비밀이나 요청·응답 본문을 담지 않는다. 엔드포인트 이름과 상태 코드만 넣는다(R-16 Q4).
- IPC를 건너는 오류도 타입이 있다. TS는 문자열 오류를 받지 않는다(3.6).

## 4. 주문 안전 구조

목표는 그대로다: **사람 승인 없이 주문이 나가는 경로가 구조적으로 없다.** 한 겹이 뚫려도 주문이 나가지 않도록 겹을 쌓는다.

### 4.1 막는 겹

| # | 겹 | 막는 것 | 강제 수단 |
|---|---|---|---|
| 1 | AI는 확률만 낸다 | AI가 주문 내용을 정하는 것 | MCP에 제안·주문 도구가 없다. `submit_estimate` 입력 타입에 방향·수량·가격이 없다 |
| 2 | 컴파일 단위 분리 | AI·MCP·스케줄러 쪽 코드가 전송 코드를 부르는 것 | 4.2의 그래프 규칙, `xtask arch-check` |
| 3 | 봉인된 타입 | 승인 없이 `ApprovedOrder`를 만드는 것 | 4.3, trybuild compile-fail 테스트 |
| 4 | 코어가 띄우는 본인 확인 | WebView 스크립트의 승인 위조 | 4.5. WebView는 `proposal_id`로 요청만 한다 |
| 5 | 재검증과 소비 기록 | 오래된 값으로의 전송, 같은 제안의 중복 전송 | 4.4. 고유 제약과 결정적 `clientOrderId` |
| 6 | 전송 전 기록 | 기록 없는 전송 | 감사 로그 쓰기가 실패하면 전송하지 않는다 |
| 7 | 주문 기능 스위치 | 준비되지 않은 상태의 승인 | 기본 꺼짐. 켜는 것은 코어가 띄우는 네이티브 확인 창을 거친다(SEC-12) |
| 8 | 경로 고정 테스트 | 위 겹이 조립 단계에서 무너지는 것 | 4.6 |

### 4.2 crate 분리

주문을 실제로 보내는 데 필요한 것은 `order-execution`에만 있다.

- `BrokerTrade` 포트
- `UserApproval`, `ApprovedOrder`
- `approve_and_submit` (승인과 전송을 한 호출로)

| crate | `order-execution`과의 관계 |
|---|---|
| `approval-command` | 유일한 호출자. WebView 요청을 받아 4.4 순서를 실행한다. 공개 API는 승인 요청 함수와 생성자뿐이다 |
| `broker-toss-trade` | `BrokerTrade` 구현. `ApprovedOrder`를 받아야만 주문 요청을 만들 수 있다. 토큰은 `broker-toss`의 토큰 관리자에서 받는다 |
| `trade-fakes` | 테스트와 `e2e-app`용 `RecordingBroker`. `ApprovedOrder`를 만들 수 없고 받기만 한다 |
| `src-tauri` | 직접 의존하지 않는다. `broker-toss-trade`의 구현체를 만들어 `approval-command`에 넘기고, 승인 서비스를 IPC 명령에 연결만 한다 |
| 그 밖 전부 | 전이 그래프에도 나타나지 않는다 |

`broker-toss`(읽기)는 공개 API에 GET과 토큰 발급만 둔다. 주문 경로 요청은 `broker-toss-trade`에만 있다. 그래서 `toss-smoke`나 읽기 어댑터를 손에 넣어도 주문을 만들 수 없다(R-01).

### 4.3 타입

```
Proposal<Draft>  (application이 저장하고 복원한다)
   │ domain::validate(facts, policy)
   ▼
Proposal<Validated>  (메모리에만. Clone 없음. 저장·복원하지 않음)
   │ order_execution::approve_and_submit(...)
   ├ UserPresence::confirm(OrderSummary) → Confirmed
   ├ UserApproval  (Proposal<Validated>를 소유한다)
   ├ 재확인, 소비 기록, 전송 전 기록
   ├ UserApproval::into_order(self) → ApprovedOrder
   └ BrokerTrade::submit(self, ApprovedOrder)
```

| 규칙 | 근거 |
|---|---|
| `BrokerTrade::submit`은 `ApprovedOrder`만 받는다 | v0.1 |
| `UserApproval`, `ApprovedOrder`의 필드와 생성자는 `order-execution` 밖에서 보이지 않는다. 비공개 필드 하나로 crate 밖 생성이 막힌다 | R-09 Q4 |
| 두 타입은 직렬화, 역직렬화, 복제, `Default`를 구현하지 않는다. `&mut` 접근자를 내지 않는다 | R-09 Q4 |
| 토큰은 `self`를 소비하는 메서드로만 넘어간다 | R-09 Q1 |
| `UserApproval`이 `Proposal<Validated>`를 소유한다. 승인한 제안과 보내는 제안이 다를 수 있는 상태가 타입에 없으므로, 방어 분기 없이 커버리지 100%가 된다 | R-09 Q7, I-13 |
| `Proposal<Validated>`는 `Clone`이 없고 DB에서 복원하지 않는다. DB에는 `Proposal<Draft>`와 `ValidationReport`만 있고, 저장된 "검증됨" 표시는 화면 힌트일 뿐이다. 승인할 때마다 `validate`를 다시 거친다 | R-09 Q12 |
| `Proposal<Draft>`는 코드(`DraftProposal`)만 만든다. AI가 만들 수 있는 경로가 없다 | open-issues 1장 |
| 테스트용 생성자를 feature나 `cfg`로 열지 않는다. `cfg(test)` 외의 `cfg`와 `features` 표는 `xtask`가 금지한다 | R-09 Q4 |
| `UserApproval`에 Secure Enclave 승인 서명을 담을 자리를 둔다. 서명은 감사·변조 탐지용이고 2단계에서 켠다(U-02 선행) | R-16 Q2, 3.2 |

다음 우회는 이 규칙으로 막힌다(R-09 Q4): `unsafe` 위조(`unsafe_code` forbid와 린트 상속 검사. 예외가 생겨도 `platform-macos` 하나뿐이고 `order-execution`은 해당하지 않는다, 4.5), 구조체 리터럴 위조(E0451), 복제(E0308), `Default`(E0277), 비공개 메서드 호출(E0624), `build.rs`·proc-macro 코드 생성, `include!`·`#[path]`, feature·`cfg`로 생성자 노출.

### 4.4 승인 순서

`approval-command`가 한 호출 안에서 다음을 한다.

| 순서 | 하는 일 | 실패하면 |
|---|---|---|
| 1 | 요청 빈도 확인: 한 번에 확인 창 하나, 거절한 제안은 일정 시간 다시 요청할 수 없음. 요청 횟수를 감사 로그에 남긴다 | 거부, 주문 0건 |
| 2 | 주문 기능 스위치와 주문 횟수 상한(ORD-6) 확인 | 거부 |
| 3 | `proposal_id`로 `Proposal<Draft>`를 읽는다. 만료(10분, 변동성 기준 가격 범위 이탈)됐으면 신호에서 최신가로 제안을 다시 만든다(`DraftProposal`). 추정 기준가에서 변동성 기준 범위 밖이면 거부 | 거부 |
| 4 | 최신 시세와 잔고로 `validate`를 다시 돌려 `Proposal<Validated>`를 만든다 | 거부 |
| 5 | 코어가 `OrderSummary`(종목명과 코드, 시장, 방향, 수량, 지정가, 예상 금액과 수수료, 계좌 뒤 4자리, `clientOrderId` 앞 8자)를 계산한다 | - |
| 6 | `UserPresence::confirm` → 네이티브 요약 창(1억 원의 80% 이상이면 고액 확인을 함께 받고 `confirmHighValueOrder`를 붙인다), 그다음 macOS 본인 확인 | 거부 |
| 7 | 본인 확인 뒤 재확인: 확인 창이 떠 있는 동안 가격이나 잔고가 허용 범위를 넘게 바뀌었으면 보내지 않는다. 다시 계산해서 보내지 않는다. 사용자가 본 요약과 보내는 주문이 같아야 하기 때문이다 | 거부 |
| 8 | 한 트랜잭션으로 "소비됨"(제안 ID 고유 제약)과 "전송 중(`clientOrderId`)"을 감사 로그에 쓴다. `clientOrderId`는 제안 ID에서 결정적으로 만든다 | 전송하지 않음 |
| 9 | `ApprovedOrder`를 만들어 `BrokerTrade::submit` | 결과 모름이면 `OrderOutcome::Unknown`, I-07. 고액 확인을 요구하는 거절이면 그 주문은 실패로 두고 다시 보내지 않는다 |
| 10 | 응답을 감사 로그에 쓰고 `OrderSubmitted` | - |

이 구조로는 "지금 승인하고 장 시작에 전송", "여러 건 한 번에 승인"을 할 수 없다. 승인을 저장해야 하기 때문이다(R-09 Q12). 필요해지면 토스의 예약·조건 주문으로 보내거나 매번 다시 확인한다.

### 4.5 WebView 밖 본인 확인

WebView에는 AI가 쓴 글이 그려지므로 신뢰하지 않는다. WebView 안의 스크립트는 승인 버튼과 같은 IPC 명령을 부를 수 있다. 그래서 WebView의 호출은 승인 **요청**일 뿐이다.

`UserPresence`의 실제 구현은 `platform-macos`에 있다. 플러그인을 쓰지 않고 코어가 직접 부른다(I-02). tauri에 의존하지 않는다.

본인 확인 호출(Objective-C 블록과 콜백)에는 `unsafe`가 필요할 수 있다. 작업 공간은 `unsafe_code`를 `forbid`로 두지만, `platform-macos` crate 하나에만 보호된 예외를 둘 수 있다. 안전한 래퍼 crate로 `unsafe` 없이 할지, 예외를 둘지는 O-07의 ADR에서 정한다. 다른 crate에는 예외를 두지 않는다.

| 창 | 구현 | 보여 주는 것 | 한계 |
|---|---|---|---|
| 네이티브 요약 창 | `rfd` 0.18.0 | 4.4의 `OrderSummary` 전체, 확인·취소 버튼 | 인증된 화면이 아니다. 가치는 "WebView가 내용을 바꿀 수 없다"는 데 있다 |
| macOS 본인 확인 창 | `objc2-local-authentication` 0.3.2로 `LAContext.evaluatePolicy(.deviceOwnerAuthentication)`. 블로킹 호출이라 `spawn_blocking`에서 부른다 | 짧은 이유 문자열(예: 종목, 방향, 수량, 대략 금액) | 길이 제한과 잘림은 미확인이다(S-02). 표와 강조를 쓸 수 없다 |

- `.deviceOwnerAuthentication`은 Touch ID를 쓸 수 없으면 로그인 암호로 넘어간다. 실제로 Touch ID를 쓸 수 없는 순간이 관찰됐다(R-16 Q2).
- 모든 주문에 요구한다(P-9: 본인 확인은 주문을 보낼 때만).
- 위험 설정 변경(SEC-12의 목록: 주문 기능 켜기, 시장가 허용, 몇 시간 단위 기간 허용, 비중 상한 완화, 위험 성향을 더 공격적으로 바꾸기)도 같은 포트를 거치되 네이티브 확인 창만 띄우고 macOS 본인 확인은 부르지 않는다(SEC-12, P-9). 이 경로는 `approval-command`가 아니라 `application`의 설정 유스케이스이고 `order-execution`과 무관하다.
- 서명한 Tauri 앱에서 두 창과 데이터 보호 키체인이 실제로 동작하는지는 S-02에서 확인한다. Tauri 공식 biometric 플러그인은 쓰지 않으므로 그 플러그인의 macOS 지원 여부와 무관하다.

### 4.6 검증

| 검사 | 내용 | 언제 | 근거 |
|---|---|---|---|
| `xtask arch-check` | 3.2의 허용 간선, 전이 금지, 재수출 금지, 외부 crate 위치, 쓰기 메서드와 주문 경로 조합의 위치, `impl UserPresence` 위치와 `UserPresence`의 별칭 가져오기·재수출 금지, 제품 bin에 `testkit`·`trade-fakes`·자동화 플러그인 없음 | 매번 | R-09, R-15 |
| `xtask capability-check` | 생성된 명령 목록과 capability 명령 허용 항목 일치, 금지 권한 없음 | 매번 | R-05 |
| trybuild compile-fail | 리터럴 위조, 복제, `Default`, 비공개 메서드, `Proposal<Validated>` 위조가 컴파일되지 않음. `tests/`에 둔다 | 매번 | R-09 Q4 |
| 커버리지 | `order-execution`과 주문 게이트 줄 커버리지 100%. 남는 분기는 보호된 예외 목록 | 매번 | I-13 |
| 변이 테스트 | 승인 생략 변이체는 컴파일 불가(unviable)이거나 잡혀야 한다. 생존 0. `--cap-lints=true` | 푸시(변경분), 밤(전체) | R-09 Q6 |
| L4 경로 고정 테스트 | 아래 표. `tauri::test` 목 런타임, 제품과 같은 `build_app`, macOS 출처 `tauri://localhost` | 매번(로컬), macOS CI는 `main`과 밤 | R-15 |
| L7 E2E 연기 | `e2e-app`을 실제로 띄워 승인 버튼만 누르면 0건 | 밤 macOS | R-15 |
| L9 사람 확인 | 실제 Touch ID와 토스 소액 실주문 | 출시 전과 변경 시 (O-15) | R-15 |

L4에서 단언할 것. 이 표가 주문 경로 고정 테스트 단언의 유일한 목록이다. [harness.md](harness.md) 6.5는 이 표를 가리킨다. 명령 목록은 생성된 바인딩에서, 도구와 작업 목록은 레지스트리에서 가져온다. 손으로 쓴 목록을 쓰지 않는다. 가짜는 `testkit`의 `ScriptedPresence`와 `trade-fakes`의 `RecordingBroker`다(3.2).

| # | 상황 | 기대 |
|---|---|---|
| a | 본인 확인이 거부됨 | 주문 0건 |
| b | WebView가 승인 요청에 `proposal_id` 밖의 필드(수량, 가격, `approved`)를 덧붙임 | 역직렬화 거부(`deny_unknown_fields`), 본인 확인 0회, 주문 0건 |
| c | 승인 명령을 뺀 모든 등록 명령을 부름 | 본인 확인 0회, 주문 0건 |
| d | MCP 도구 레지스트리의 모든 도구를 무작위 인자(proptest)로 부름 | 주문 0건 |
| e | 스케줄러 레지스트리의 모든 작업을 고정 시계로 한 바퀴 돌림 | 주문 0건 |
| f | 본인 확인 통과 | 정확히 1건. 보낸 주문 = 확인 창에 보인 요약 = 코어 계산값 |
| g | 같은 제안을 다시 승인 요청 (소비 기록 뒤) | 본인 확인 0회, 주문 0건 |
| h | 원격 출처에서 승인 요청 | ACL 거부, 주문 0건 |
| i | 위험 설정 변경 명령을 네이티브 확인 없이(또는 확인 거부로) 부름 | 설정이 바뀌지 않음. macOS 본인 확인 0회 (SEC-12) |
| j | 감사 로그 쓰기가 실패하도록 만든 뒤 승인 | 주문 0건 |
| k | 주문 기능 스위치가 꺼진 상태에서 승인 | 본인 확인 0회, 주문 0건 |
| l | 주문 횟수 상한을 넘긴 뒤 승인 (ORD-6) | 본인 확인 0회, 주문 0건 |
| m | 확인 창이 떠 있는 동안 가격이 허용 범위를 넘게 바뀜 (4.4의 7) | 주문 0건 |
| n | 확인 창이 떠 있는 동안 두 번째 승인 요청 (SEC-8, ORD-7) | 두 번째 요청 거부, 확인 창 1개 |

- (d), (e)의 0건은 이미 그래프 규칙이 컴파일 단계에서 보장한다. 테스트는 그 보장이 조립 루트에서 무너지지 않았는지 보는 두 번째 겹이다.
- 본인 확인을 무시하게 바꾼 변이를 넣으면 (a)가 실패해야 한다. cargo-mutants로 확인한다(harness.md 6.4).
- 실제 앱 설정의 앱 ACL을 L4에 적용하려면 `mock_context` 대신 `generate_context!()`를 써야 한다. 이 조합이 되는지는 미확인이다(R-15, S-13). O-13에서 확인하고, 안 되면 capability 검증은 `xtask capability-check`와 L7이 맡는다.
- 경로 고정 테스트 파일과 가짜(`ScriptedPresence`, `RecordingBroker`)는 보호 파일이다.

## 5. 프런트엔드 구조

```
apps/desktop/src/
├ app/        라우트, 전역 제공자
├ features/   화면 단위 (home, chat, holding, scorecard, settings ...)
│  └ <name>/  ui/ · model/ · index.ts
├ shared/
│  ├ ds/      디자인 시스템 (컴포넌트, 생성된 tokens.css·tokens.ts)
│  ├ ipc/     생성된 bindings.ts, 명령 클라이언트, testing/
│  ├ platform/  허용 목록에 있는 파일만: 이벤트 구독, 테마·글자 크기 구독, Canvas 차트 래퍼, detach
│  └ lib/     Result, 금액 타입, 표시 형식
```

주 화면은 하나다: 내 현황과 채팅. 신호가 생기면 그 화면에 카드로 나타난다(P-10).

**폴더 의존** (R-08)

| 폴더 | 가져올 수 있는 것 |
|---|---|
| `app` | `features/*`(`index.ts`만), `shared/*` |
| `features/x` | `features/x` 내부, `shared/ds`, `shared/ipc`, `shared/platform`, `shared/lib` |
| `shared/ds` | `shared/lib` (연결 방식은 아래 "디자인 시스템 연결") |
| `shared/ipc` | `shared/lib` |
| `shared/platform` | `shared/ipc`, `shared/lib` |
| `shared/lib` | 없음 |

다른 요소의 파일은 `index.ts`로만 가져온다. 이 분류에 속하지 않는 파일은 만들 수 없다.

| 규칙 | 내용 |
|---|---|
| 계산 금지 | TS는 금액·비중·확률을 계산하지 않는다. 코어가 계산한 값을 받아 형식만 바꾼다 |
| IPC 한 곳 | Tauri 호출은 `shared/ipc`에서만 한다. 타입은 Rust에서 생성한다. 응답은 Zod `safeParse`로 검증한다 |
| 기능 격리 | `features/a`는 `features/b`를 가져올 수 없다. 공유할 것은 `shared`로 내린다 |
| 차트 숫자 | 차트 좌표는 코어가 표시 전용 십진수 문자열로 따로 보낸다. 숫자로 바꾸는 것은 디자인 시스템 차트 컴포넌트 안에서만 한다. 금액 타입을 숫자로 바꾸지 않는다. 방향(상승·하락·보합)도 코어가 보낸다 |
| 채팅 카드 | 카드는 마크다운에서 파싱하지 않는다. 코어 이벤트의 ID로만 그린다 |
| 디자인 시스템 | `features`와 `app`은 `shared/ds`의 컴포넌트를 조립만 한다. HTML 태그, `className`, `style`을 쓰지 않는다 ([harness.md](harness.md) 4.4) |
| 레이아웃 | 배치는 5.1의 레이아웃 컴포넌트로만 한다 |
| 효과 금지 | `useEffect` 계열과 `useSyncExternalStore`는 `shared/platform`의 허용 목록 파일에서만 쓴다. 데이터는 쿼리 계층과 이벤트 구독 훅으로 받는다 |
| 상태 | 서버 상태는 TanStack Query(`features/*/model`의 `queryOptions`). `queryFn`은 `Promise<Result>`를 돌려주고 거절하지 않는다. 화면 상태는 TanStack Router(메모리 또는 해시 기록)와 컴포넌트 상태. 전역 저장소는 ADR이 있어야 추가한다 |
| 외부 링크 | WebView 안에서 이동하지 않는다. 코어 명령 `open_external`이 `https`만 열고, 수집 목록에 있는 주소나 앱이 정한 고정 주소만 연다 |
| AI 마크다운 | 원시 HTML을 끈다. 이미지는 링크 글자로 바꾼다. 수집 목록에 있는 주소만 `open_external`을 부르는 DS 링크 컴포넌트로 바꾸고, 나머지 주소는 글자로만 보인다 |
| 디자인 시스템 연결 | 디자인 시스템 컴포넌트는 콜백(예: 링크를 눌렀을 때)과 테마 값을 속성으로 받는다. `shared/ipc`나 `shared/platform`에 연결하는 것은 화면 코드가 한다 |

### 5.1 레이아웃 시스템

데스크톱 앱 기준이다. `features`는 아래 컴포넌트와 토큰 이름만으로 화면을 배치한다. 값은 전부 디자인 시스템 토큰에서 온다.

**창과 구역**

| 항목 | 값 |
|---|---|
| 창 최소 크기 | 960 × 640. 이보다 작게 줄일 수 없다 |
| 구역 | 사이드바 · 본문 · 채팅 패널 (`AppShell`). 사이드바는 화면 전환 메뉴가 아니라 design.md 2.4의 보유·관심 종목 목록이다. 본문에는 주 화면의 현황과 카드가 들어가고, 보조 화면도 본문 자리에 열린다. 그래서 화면은 여전히 하나다 (P-10) |
| 창 폭 1280 이상 | 세 구역 모두 표시. 채팅 패널은 320~560 사이에서 조절 |
| 창 폭 1080~1279 | 채팅 패널이 본문 위에 뜨는 형태로 바뀌고 기본은 닫힘 |
| 창 폭 1080 미만 | 사이드바가 56 폭의 띠로 접힘. 띠에는 종목 이름 앞 두 글자와 ▲▼만 보이고, 펼치기 버튼을 누르면 목록 전체가 본문 위에 뜬다. 종목을 고르거나 Esc를 누르면 닫힌다 |
| 스크롤 | 창 전체는 스크롤하지 않는다. 본문과 채팅 패널이 각자 스크롤한다 |
| 창 모양 (사용자 결정 2026-11-12) | 주 창은 macOS의 제목 표시줄, 신호등 버튼, 창 테두리와 그림자, 전체 화면, 창 끌기를 그대로 쓴다. 창 장식을 웹으로 그리지 않는다 |
| 네이티브 창 | 확인 창(주문 요약, 고액 확인, 주문 켜기와 위험 설정 확인)은 `platform-macos`가 AppKit 기본 컨트롤로 그린다. 시스템 글꼴, 기본 버튼, 기본 체크박스를 쓰고 라이트·다크는 시스템을 따른다. 디자인 시스템 토큰을 쓰지 않는다 |
| 설정 창 | 설정은 주 화면 위의 대화 상자가 아니라 별도의 macOS 창으로 연다(⌘, 메뉴 막대 아이콘, 머리줄 설정 버튼). 1단계에서는 창 안 내용을 디자인 시스템의 폼 컴포넌트로 그린다. 내용까지 네이티브 컨트롤(SwiftUI 폼)로 바꿀지는 실측 S-24 뒤 ADR로 정한다 |

**본문 격자**

- 12열 격자, 열 사이 간격은 `space-6`.
- 구간은 창 폭이 아니라 본문 폭으로 정한다. 채팅 패널을 조절하면 본문 폭이 변하기 때문이다 (컨테이너 쿼리).

| 본문 폭 구간 | 이름 | 카드 배치 예 |
|---|---|---|
| 640~899 | `narrow` | 한 줄에 1~2개 |
| 900~1199 | `medium` | 한 줄에 2~3개 |
| 1200 이상 | `wide` | 한 줄에 3~4개 |

- 본문 최대 폭을 둔다 (`content-max`, 1200). 가장자리 여백 안쪽 폭에 적용하므로 격자가 `wide` 구간에 닿는다. 긴 글은 `prose-max`(720)에서 멈춘다.
- 구간 판정은 CSS 컨테이너 쿼리로 구현한다. 디자인 시스템의 미리보기는 같은 값을 스크립트로 판정하지만 결과는 같다.
- 격자 항목이 차지하는 열 수는 3, 4, 6, 8, 9, 12 중 하나만 고를 수 있고, 구간별로 지정한다.

**간격**

| 위치 | 토큰 |
|---|---|
| 화면 가장자리, 섹션 사이 | `space-8` |
| 카드 사이 | `space-6` |
| 카드 안쪽 | `space-4` |
| 표 셀 좌우 | `space-3` |

기준 단위는 4px이고 간격 토큰 밖의 값은 쓸 수 없다.

**스크롤과 넘침** (사용자 결정 2026-11-12)

목표는 두 가지다. 스크롤 막대는 항상 내용 위에 겹쳐 뜨고 자리를 차지하지 않는다. 내용이 잘리거나 영역 밖으로 삐져나오는 화면은 만들 수 없다.

| 원칙 | 내용 |
|---|---|
| 스크롤은 `ScrollArea` 하나로만 | `shared/ds`의 `ScrollArea`(Base UI ScrollArea 래퍼)만 스크롤을 만든다. 막대는 내용 위에 겹쳐 뜨고 폭을 차지하지 않아, 막대가 나타나고 사라져도 배치가 움직이지 않는다. 운영체제의 "스크롤 막대 항상 표시" 설정과 마우스 연결 여부에 상관없이 같은 모양이다 |
| 기본 스크롤 막대는 없다 | 스크롤 영역의 기본 막대를 숨긴다(`scrollbar-width: none`). 문서(`html`, `body`)는 스크롤하지 않는다 |
| 스크롤 영역의 자리는 정해져 있다 | 세로 스크롤은 `AppShell`의 본문, 채팅 패널, 종목 목록과 `Dialog`·`Popover`·`Menu`의 본문에만 있다. 가로 스크롤은 `Table`과 차트 컴포넌트가 스스로 갖는다. 화면 코드는 `ScrollArea`를 직접 쓰지 않는다 |
| 같은 방향 스크롤을 겹치지 않는다 | 세로 스크롤 영역 안에 세로 스크롤 영역을 두지 않는다. 예외는 `ChatInput`처럼 높이 상한이 토큰으로 정해진 입력뿐이다 |
| 내용을 자르지 않는다 | `overflow: hidden`·`clip`은 `ScrollArea`와 아래의 말줄임, 그리고 둥근 모서리를 맞추는 허용 목록 파일에만 있다. 그 밖의 컴포넌트는 내용을 숨길 수 없다 |
| 말줄임은 `Text`의 선택지 하나로만 | 글자는 기본이 줄바꿈이다(`word-break: keep-all`, `overflow-wrap: anywhere`로 한글은 어절 단위, 긴 영문·숫자는 강제로 꺾는다). 한 줄 말줄임은 `Text`·`Heading`의 `overflow: 'truncate'`로만 하고, 이때 전체 글자가 툴팁으로 반드시 뜬다. 숫자(`Money`, `Percent`, `Quantity`, `Probability`, `PriceChange`)는 줄바꿈도 말줄임도 하지 않는다. 자리가 모자라면 그 숫자를 담은 표나 카드가 가로 스크롤 영역이 된다 |
| 줄어들 수 있는 배치 | `Stack`, `Inline`, `Split`, `GridItem`의 자식은 `min-width: 0`·`min-height: 0`이 기본이다. 자식이 부모보다 넓어져 삐져나오는 flex·grid의 기본 동작을 없앤다 |
| 고정 높이를 주지 않는다 | 글자를 담는 컨테이너는 높이를 받지 않고 내용만큼 늘어난다. 높이 토큰은 컨트롤, 표 행, 차트, 머리줄처럼 허용 목록에 있는 컴포넌트에서만 쓴다. 폭은 격자 열 수와 폭 토큰으로만 준다 |
| 떠 있는 요소는 겹침 컴포넌트로만 | `position: absolute`·`fixed`·`sticky`는 `Dialog`, `Popover`, `Menu`, `Tooltip`, `Toast`, `AppShell`, `Table`의 머리줄 등 허용 목록 파일에만 있다. 겹침 컴포넌트는 포털로 그리고 화면 가장자리에서 방향을 바꾸므로 조상 요소에 잘리지 않는다 |
| 화면 단위를 쓰지 않는다 | `vh`, `vw`, `dvh` 등은 `AppShell` 한 곳에서만 쓴다. 나머지는 부모 크기를 따른다 |

이 원칙은 린트와 자동 넘침 검사로 강제한다 ([harness.md](harness.md) 4.4 "스크롤과 넘침").

**레이아웃 컴포넌트**

| 컴포넌트 | 역할 | 받는 값 |
|---|---|---|
| `AppShell` | 세 구역과 창 폭별 동작. `app`에서 한 번만 쓴다 | 없음 |
| `Page` | 화면 하나. 가장자리 여백과 최대 폭 | 제목, 동작 버튼 |
| `Section` | 화면 안의 묶음. 제목과 세로 간격 | 제목 |
| `Grid`, `GridItem` | 12열 격자 | 구간별 열 수 (허용 값만) |
| `Stack` | 세로로 쌓기 | 간격 토큰 이름 |
| `Inline` | 가로로 늘어놓기 | 간격 토큰 이름, 정렬, 줄바꿈 |
| `Split` | 고정 폭 쪽과 늘어나는 쪽 | 고정 쪽 폭 토큰 이름 |
| `Card` | 내용 묶음의 면 | 제목, 동작 버튼 |
| `Toolbar` | 필터와 동작 버튼 줄 | 없음 |
| `ScrollArea` | 겹쳐 뜨는 막대를 가진 스크롤 영역. `shared/ds` 안에서만 쓴다 | 방향 |
| `Divider` | 구분선 | 방향 |

- 모든 화면은 `Page`로 시작한다.
- 밀도는 두 단계다: 표 행 `row-height`/`row-height-dense`, 컨트롤 `control-height`/`control-height-sm`.
- 겹침 순서는 `z-*` 토큰만 쓴다.

레이아웃 토큰은 디자인 시스템(`design/tokens.json`)에 모두 있다: `window-min-width`, `window-min-height`, `bp-window-md`(1080), `bp-window-lg`(1280), `bp-content-medium`(900), `bp-content-wide`(1200), `content-max`, `prose-max`, `grid-columns`, `chat-width-min`, `chat-width-max`, `sidebar-width`, `sidebar-collapsed`, `dialog-width`, `dialog-width-wide`, `toolbar-height`, `chart-height-*`. 간격은 `space-1`~`space-12`(4px 기준)이고 예외는 `gap-hairline`과 테두리 굵기 토큰뿐이다.

### 5.2 확정한 라이브러리

버전은 리서치 파일에 적힌 2026-11-11 기준 값이다. 모두 정확한 버전으로 고정한다.

| 용도 | 선택 | 근거 |
|---|---|---|
| IPC 클라이언트 | `@tauri-apps/api` 2.12.4 + 생성된 바인딩. `@tauri-apps/api/mocks`는 `shared/ipc/testing`에서만 | R-15 |
| 서버 상태 | TanStack Query 5.107.1 | R-08 |
| 라우팅 | TanStack Router 1.170.46 (메모리 또는 해시 기록) | R-08 |
| 경계 스키마 | Zod 4.6.7, `safeParse`만 | R-08 |
| 오류 경계 | `react-error-boundary` (클래스를 직접 쓰지 않기 위해) | R-08 |
| 컴포넌트 기반 | Base UI 1.9.0 | R-14 |
| 차트 | Recharts 3.10.1 (선, 영역, 도넛, 트리맵), Lightweight Charts 5.3.0 (캔들만) | R-14 |
| 토큰 | Tailwind CSS 4.3.5 `@theme`에 `--*: initial`, `design/`의 `tokens.json`에서 `xtask gen-tokens`로 생성 | R-14 |
| 채팅 마크다운 | react-markdown 10.1.0, 원시 HTML 끔 | R-14, R-16 |
| 아이콘 | lucide-react 1.72.0 | R-14 |
| 컴포넌트 목록과 시각 회귀 | Storybook 10.6.7 + Vitest 5.1.0 브라우저 모드. 기준 환경은 고정 태그 Playwright 컨테이너(linux/arm64). 비교 엔진은 harness.md 6.6 | R-14, R-15 |
| 컴파일러 | 타입 검사는 TypeScript 7.0.2, 린트는 TypeScript 6 API와 함께 설치 | I-16, R-08 |

- 제3자 컴포넌트는 반드시 `shared/ds`의 래퍼로 감싼다. `--*: initial`이 Tailwind 기본 클래스를 없애기 때문이다(R-14).
- Canvas 차트(Lightweight Charts)는 CSS 변수를 해석하지 못하므로 `shared/platform`의 래퍼가 테마를 읽어 `shared/ds`의 차트 컴포넌트에 속성으로 넘기고, 테마가 바뀌면 다시 그린다(R-14).
- 디자인 산출물의 "컴포넌트가 비중·손익을 계산", "주문 버튼 없음"은 이 문서와 충돌한다. 디자인 쪽을 고친다(R-14 5장). 승인 버튼은 신호·주문 제안 카드에만 있고, 누르면 WebView 밖 확인 창이 뜬다.

### 5.3 금액과 숫자 표시

| 규칙 | 내용 | 근거 |
|---|---|---|
| 금액 타입 | `Money`, `Quantity`, `Price`, `Ratio`, `Probability`는 비공개 symbol 키를 가진 **불투명 객체**다. 문자열 브랜드로 만들지 않는다. 문자열 브랜드는 `parseFloat`와 `+`가 타입 검사를 통과한다 | I-15, R-08 |
| 만드는 곳 | `shared/lib/money.ts` 하나. IPC DTO의 십진수 문자열을 파싱해서만 만든다. 타입 술어 허용 목록의 유일한 파일이고 보호 파일이다 | R-08 |
| 할 수 있는 것 | 형식 바꾸기뿐이다. 산술, 비교, 숫자 변환은 로컬 린트 규칙이 막는다 | R-08 |
| 형식 | 십진수 문자열을 `Intl.NumberFormat`에 문자열 그대로 넘긴다. 부동소수점을 거치지 않는다. 빼기 기호는 U+2212로 바꾼다. 만·억 혼합 표기와 `원` 접미는 직접 만든다 | R-14 2.5 |
| 방향과 색 | 상승·하락·보합은 코어가 보낸 값으로 정한다. 표시 단위로 반올림하면 0이 되는 값의 방향 규칙은 아직 정하지 않았다(R-14). D-01·DS-05에서 정한다 | R-14 |
| ID와 종목 코드 | 산술과 무관하므로 Zod 브랜드 문자열로 충분하다 | R-08 |
| 그 밖의 모든 값 | 타입 자리에 `string`·`number`·`bigint`를 쓰지 않는다. 생성된 `bindings.ts`의 원시 타입은 `shared/ipc`의 Zod 스키마가 브랜드로 바꾸고, `features`는 브랜드 타입만 받는다. `boolean`도 쓰지 않고 리터럴 유니온으로 쓴다(`disabled: boolean`이 아니라 `state: 'enabled' \| 'disabled'`). 화면 문구는 `UiText` 브랜드이고 `messages.ts`(문구 키마다 언어별 문장)에만 있어 다국어를 바로 붙일 수 있다. `shared/ds`의 props도 브랜드와 리터럴 유니온만 받는다. `design/`의 타입 선언에 있는 불린 props(`judged`, `over`, `dense` 등)는 앱의 `shared/ds`에서 리터럴 유니온으로 옮긴다 | harness 4.2 |

WKWebView에서의 한글 입력과 `Intl` 결과는 S-05에서 실제 창으로 확인한다.

## 6. 데이터

### 6.1 드라이버와 암호화

| 항목 | 결정 | 근거 |
|---|---|---|
| 드라이버 | `rusqlite` `=0.41.1`, 기능 `bundled-sqlcipher`, `backup`, `hooks`. `store-sqlite`에만 | R-06 |
| 연결 | 전용 DB 스레드 하나가 쓰기 연결을 쥐고 채널로 요청을 받는다 | R-06 |
| 암호화 | SQLCipher(macOS CommonCrypto). WAL과 저널 포함, `TEMP_STORE=2` | R-06 |
| 키 | 키체인의 32바이트 난수 원시 키(KDF 없음). 조립 루트가 `SecretStore`에서 꺼내 `store-sqlite`에 넘긴다. 두 어댑터는 서로 의존하지 않는다 | R-06 |
| 키체인 | 데이터 보호 키체인(`security-framework` 3.7.0), 앱 팀 접근 그룹, `ThisDeviceOnly`. App ID 서명과 프로비저닝 프로파일이 필요하다. 개발 빌드는 별도 서비스 이름과 가짜 키만 쓴다 | R-16 Q1 |
| 빌드 검사 | 빌드 산출물이 `cipher_provider = commoncrypto`이고 시스템 프레임워크에만 링크되는지 확인한다 | R-06 |

- Apple 개발자 멤버십이 없으면 데이터 보호 키체인을 쓸 수 없고 로그인 키체인으로 낮춰야 한다(U-02). 서명한 앱에서 실제로 동작하는지는 S-02에서 확인한다.
- 키를 잃으면 DB와 백업을 모두 잃는다. 복구 경로는 아직 정하지 않았다(R-06, R-16). 키체인 구현 작업 앞의 서명 구성 작업(tasks.md의 작업 S-14)에서 ADR로 정한다.

### 6.2 테이블 묶음

| 묶음 | 내용 | 특성 |
|---|---|---|
| 계좌·보유 스냅샷 | 시점별 보유와 평가 | 장중 30일 보관 후 일별 하나로 줄인다 |
| 일별 평가 | `ValuationSnapshot`(환율 포함). 빠진 날은 일봉과 거래내역으로 다시 계산 | 추가만 (I-09) |
| 거래내역 | 토스 원본과 정규화 결과. 결제 완료 행은 최근 14일을 다시 받아 같은 행의 개수까지 맞추는 멀티셋, 사라진 행은 지우지 않고 표시 | 원본 보존 (I-06, R-01) |
| 원가·현금 | 로트 원장, 현금 원장(대조용), 기업 행위 기록, 섹터 대응표 | 다시 만들 수 있음 (R-11) |
| 시세 | 일봉, 최근 현재가, 환율(`kind`, 출처, 시각) | 캐시 |
| 뉴스·공시 | 제목, 요약, 링크, 종목 매칭, 묶음. 공시 키는 `(source, receipt_no)` | 앱 측 바이그램 FTS5 색인 |
| 수집 결과 | 수집 실행이 찾은 항목 | 비신뢰 표시, 출처 필수 |
| 논거 노트 | 종목별 현재 노트와 변경 이력 | 이력 보존 |
| 추정 | 추정 과제, 추정, 판정 결과, 보정표 | 추가만. 거래하지 않은 추정도 판정 |
| 신호·제안 | 신호, `Proposal<Draft>`, 검증 보고서, 소비 기록(제안 ID 고유 제약) | 상태 전이 기록. `Validated`는 저장하지 않음 |
| 주문 | `clientOrderId`, 전송 결과, 체결 상태 | R-01 |
| 감사 로그 | 승인 요청, 본인 확인 결과, 전송 전 기록, 응답, 위험 설정 변경 | 6.3 |
| AI 작업 | 실행 종류, 프롬프트 해시, CLI 버전, 모델, 추론 강도, 파이프라인 변형, 증거 묶음 ID, 사용량, 결과 | 재현용 (R-12) |
| MCP 호출 원장 | 실행 ID, 도구, 인자, 결과 해시, 시각 | 추가만 (R-04) |
| 대화 | 메시지 | 앱을 다시 켜면 새 CLI 프로세스에 다시 넣는다 (R-03, I-04) |
| 설정 | 투자 성향, 원칙, 주기, 모델 선택, 토스 스펙 해시 | 버전 있는 스키마 |

### 6.3 규칙

| 규칙 | 내용 | 근거 |
|---|---|---|
| 테이블 | 전부 STRICT | R-06 |
| 금액 열 | TEXT 정규 십진수(`^-?\d+(\.\d+)?$`를 CHECK로 강제) + 통화 열. NUMERIC 열은 정밀도를 잃는다 | R-06 |
| SQL 산술 | 금액·수량·비율 열에 합산, 평균, 산술, `ORDER BY`를 하지 않는다. 계산은 Rust에서 한다 | R-06 |
| SQL 위치 | 모든 SQL은 `store-sqlite` 안의 상수다. 테스트가 마이그레이션한 빈 DB에 상수마다 `prepare`한다 | R-06 |
| 원본 보존 | 외부 원본을 보존하고 정규화 결과는 다시 만들 수 있게 한다 | v0.1 |
| 마이그레이션 | 번호가 붙은 마이그레이션만. 직접 만든 실행기와 SHA-256 원장(해시 불일치·누락·번호 공백이면 열기 실패), `xtask migration-check`(git에서 적용된 파일 수정 금지), 스키마 스냅샷 테스트 | R-06 |
| 감사 로그 | "지울 수 없는"이 아니라 **"변조를 탐지할 수 있는"** 로그다. 겹: 수정·삭제 금지 트리거, 연속 순번 트리거, 연결 단위 authorizer, DEFENSIVE, 해시 체인, HMAC(키는 키체인), 키체인에 고정한 머리값(마지막 순번과 해시), 토스 주문 내역과 대조 | I-10, R-06, R-16 Q7 |
| 감사 로그 쓰기 실패 | 주문을 보내지 않는다 | R-16 Q7 |
| 백업 | 하루 한 번, 쓰기 연결의 백업 API로 암호화 사본 → `cipher_integrity_check`, `quick_check`, 해시 체인 검증 → 원자적 이름 변경. 7일 보관 | R-06 |
| 보존 | 장중 스냅샷은 30일 뒤 일별 하나로 줄인다. `auto_vacuum=INCREMENTAL`과 예약 작업의 `incremental_vacuum` | R-06 |

감사 로그 겹마다 잡지 못하는 것이 남는다. 같은 앱 권한을 얻은 공격자가 DB와 머리값을 함께 되돌리는 경우다. 주문 사실에 대해서는 토스 서버 기록이 가장 강한 기준이다(R-16 Q7).

## 7. 확장 시나리오

| 하고 싶은 일 | 바뀌는 곳 |
|---|---|
| 증권사 추가 | 읽기 어댑터와 주문 어댑터를 따로 추가(`broker-xxx`, `broker-xxx-trade`), 조립 루트 등록, `arch-check` 표에 간선 추가 |
| AI 제공자 추가 | `adapters/ai-xxx` 추가, 설정 선택지 등록. 두 실행 종류의 격리 조건을 모두 충족해야 켠다 |
| 작업별 제공자 바꾸기 | 설정만 바꾼다. 어댑터 구조를 유지하는 이유다(open-issues 2.1) |
| 뉴스 소스 추가 | `adapters/sources`에 구현 추가, 레지스트리 등록 |
| 예약 작업 추가 | `scheduler`에 작업 구현 추가, 프롬프트 파일 추가. 경로 고정 테스트 대상에 자동으로 들어간다 |
| MCP 도구 추가 | 유스케이스 추가, `mcp-server`에 도구 등록, 실행 종류별 도구 묶음에 배정 |
| 새 화면 | `features/<name>` 추가, 필요한 IPC 명령 추가. `gen-bindings`로 다시 생성하면 명령 목록과 capability 명령 허용 항목이 함께 바뀐다. 보호 파일은 바꾸지 않는다 |
| 창 없이 별도 백그라운드 프로세스 | 조립 루트를 하나 더 만든다. 코어는 그대로다. 같은 프로세스 안의 메뉴 막대 상주는 이미 있다 |
| Windows 지원 | `SecretStore`, `UserPresence`, `Notifier` 구현 추가 |
| 조건주문 | `BrokerTrade` 확장, `OrderIntent` 종류 추가, 게이트 검증 추가 |
| 주문 정정·취소 | `BrokerTrade` 확장, 정정·취소용 제안 타입과 본인 확인 경로, 4.6에 경로 고정 단언 추가 |
| 승인 서명 (2단계) | `platform-macos`에 Secure Enclave 서명 추가, `UserApproval`의 서명 자리 채우기, 감사 로그에 서명 저장 |

## 8. 결정 기록

구조에 영향을 주는 선택은 `docs/adr/NNNN-제목.md`로 남긴다. 이번 판에서 확정한 결정 전체는 [ADR 0002](adr/0002-architecture-decisions.md)에 있다. 요약:

| 항목 | 확정 | 근거 |
|---|---|---|
| MCP 전송과 라이브러리 | Unix 소켓 + stdio 브리지(`mcp-bridge`), `rmcp` `=3.9.0` | R-04, R-16, I-01 |
| AI 실행 | 수집 실행과 분석 실행으로 분리. AI는 확률만 제출 | open-issues 1·3장 |
| AI CLI 격리 | 기존 로그인 사용, 환경 변수 허용 목록, 격리 옵션, 빈 작업 폴더, 시작 점검 | I-03, R-02, R-03 |
| 프로세스 | 메뉴 막대 상주, 단일 인스턴스, 자동 시작(기본 켬). 앱 샌드박스·업데이터·공증 없음 | I-09, R-05 |
| 툴체인 | Rust 1.99.1 (`rust-toolchain.toml`) | I-17, harness.md 1.3 |
| Rust→TS 타입 생성 | `tauri-specta` `=2.0.0-rc.28`, `specta-typescript` `=0.0.15` | R-05 |
| 이벤트 전달 | ID만 담은 타입 이벤트 + 재조회, 채팅은 요청별 Channel | R-05 |
| 주문 차단 crate | `approval-command`, `broker-toss-trade`, `trade-fakes` 분리. `src-tauri`는 `order-execution`에 직접 의존하지 않음 | R-01, R-09, R-15, I-14 |
| 의존 검사 | `xtask arch-check`(`cargo metadata`) 단독 | R-09 |
| 본인 확인 | 네이티브 요약 창(`rfd`) + `LAContext.evaluatePolicy` 직접 호출, `platform-macos` | I-02, R-16 |
| 키체인 | 데이터 보호 키체인, `security-framework` 3.7.0 | R-16 |
| SQLite 드라이버와 암호화 | `rusqlite` `=0.41.1` + SQLCipher(CommonCrypto), 32바이트 원시 키 | R-06 |
| 감사 로그 | 변조 탐지 로그(트리거, authorizer, 해시 체인, HMAC, 키체인 머리값, 토스 대조) | I-10, R-06, R-16 |
| 스케줄러 시계 | 벽시계 짧은 주기 비교, `Clock` 포트 | R-05 |
| 토스 타입 | `typify` 0.8.0으로 스키마만 생성(`broker-toss-schema`), HTTP는 손으로 | R-09 |
| 십진수 | `rust_decimal` 1.43.0, `checked_*`만 | R-09, R-11 |
| TS 린트·Result·경계 | ESLint + typescript-eslint, 직접 정의한 Result, `eslint-plugin-boundaries`, Zod 4 | R-08 |
| 프런트 라이브러리 | 5.2의 표 | R-08, R-14 |
| 테스트 계층 | L0~L9. L4는 `tauri::test` 목 런타임, E2E는 `e2e-app` + WebdriverIO | R-15 |

**아직 정하지 않은 것** (리서치가 답을 주지 않았거나 실측이 필요한 것)

| 항목 | 정하는 곳 |
|---|---|
| `domain`의 시간 타입 crate | D-01 작업의 ADR |
| DB 키 분실 시 복구 경로, 머리값 고정을 사건마다 할지 백업 시점에만 할지 | 작업 S-14 (tasks.md)의 ADR |
| 서명한 앱에서 키체인과 두 확인 창의 동작, 이유 문자열 길이 | 실측 S-02 |
| 수집 실행에서 웹 읽기를 살리는 Claude 옵션 조합 | 실측 S-07 |
| 샌드박스 안 구독 인증 | 실측 S-04 |
| 반올림 뒤 0이 되는 등락의 방향 규칙 | D-01, DS-05 |
| `generate_context!()`와 목 런타임의 조합 | 실측 S-13 (작업 O-13) |
