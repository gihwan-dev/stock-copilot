# R-15 Tauri 앱 테스트 방법

- 조사일: 2026-11-11 (출처의 "확인" 날짜는 모두 이 날이다)
- 조사 환경: macOS 26.5.2 (Apple Silicon), Rust 1.95.0 (`cargo +1.95.0`), Node 24.22.0, Docker 29.2.1 (colima, aarch64)
- 실험 코드: `<세션 임시 폴더>/research/R-15/` (저장소 밖)
  - `exp/`: Tauri 목 런타임 경로 고정 테스트, 가짜 AI CLI 재생, 네트워크 차단 (Rust 테스트 11개)
  - `visual/`: 스크린샷 재현성 실험 (호스트 Chromium과 Docker Chromium 비교)
- 실제 토스 주소, 실제 계좌, API 키, 실제 `claude` CLI는 호출하지 않았다. 네트워크 차단 실험의 대상은 `example.com`이다.

## 1. 요약

1. **경로 고정 테스트(O-13)는 WebDriver가 아니라 Rust에서 `tauri::test` 목 런타임으로 만든다.** 실제 IPC 경로(`get_ipc_response`)로 승인 명령을 부르고 가짜 `UserPresence`와 기록하는 가짜 `BrokerTrade`로 0건과 1건을 단언하는 테스트를 실제로 돌렸다. 일부러 본인 확인을 건너뛰게 고친 코드(변이)를 이 테스트가 잡는 것도 확인했다. Touch ID 창은 어떤 E2E 도구로도 자동화할 수 없으므로 이 계층이 유일한 자동 고정 지점이다.
2. **macOS E2E는 이제 가능하다.** 다만 앱 안에 자동화 서버를 넣는 방식뿐이다. `tauri-driver`를 직접 쓰는 길은 macOS를 지원하지 않는다. 공식 권장 경로는 WebdriverIO `@wdio/tauri-service` 1.5.1과 앱에 넣는 `tauri-plugin-wdio-webdriver` 1.5.1이다. 이 플러그인은 HTTP로 앱을 조종할 수 있으므로 배포 바이너리에 들어가면 안 된다. 그래서 E2E는 밤에 도는 소수의 연기 테스트로만 둔다.
3. **UI 테스트는 tauri-specta 2.0.0-rc.28의 `Builder::mocks()`로 만든다.** 생성되는 `MockCommandHandlers` 타입과 `mockCommands`(내부에서 `mockIPC`를 쓴다)를 쓰면 가짜 IPC가 생성된 명령 타입과 컴파일 단계에서 어긋나지 않는다.
4. **AI CLI는 실행 파일 경로를 주입받아 가짜 실행 파일로 바꾼다.** 녹화한 stream-json을 재생하는 방식은 바로 동작했다. 함정도 하나 확인했다. tokio 시계를 멈춘 테스트(`start_paused`)에서 실제 자식 프로세스를 기다리면 시계가 자동으로 앞당겨져 타임아웃이 거짓으로 터진다.
5. **시각 회귀는 Linux arm64 Playwright 컨테이너 하나를 기준 환경으로 정한다.** 실험에서 같은 컨테이너는 두 번 모두 바이트까지 같았다. 호스트 macOS와는 글꼴부터 달랐고, amd64 에뮬레이션에서는 Chromium이 뜨지 않았다. 테스트가 토스로 나가는 것은 세 겹으로 막는다: 기본 URL 주입, 코드 검사, 테스트 실행 파일의 OS 수준 네트워크 차단. 마지막 겹은 cargo `runner` + `sandbox-exec`로 실제로 막히는 것을 확인했다. macOS 러너는 Linux보다 분당 약 10배 비싸다(분당 $0.062). 그래서 푸시마다 도는 일은 Linux에 두고 macOS는 밤과 `main`에만 쓴다.

## 2. 질문별 답

### 2.0 추가 질문: "WebView 승인 호출만으로는 0건, 본인 확인을 통과했을 때만 1건"을 자동 테스트로 고정하는 방법

**결론: `src-tauri`의 조립 함수를 런타임과 포트에 대해 제네릭으로 만든다. 그다음 통합 테스트에서 `tauri::test::mock_builder()`로 앱을 만들고 `get_ipc_response`로 WebView가 보내는 것과 같은 IPC 요청을 넣는다.**

이 방식으로 고정되는 범위는 다음과 같다.

- WebView → IPC 디스패처 → ACL 검사 → 명령 인자 역직렬화 → `order-execution::ApproveAndSubmit` → `UserPresence` → `BrokerTrade` 경로 전체가 실제 Tauri 코드로 실행된다. 목 런타임은 네이티브 WebView만 빼고 Tauri의 IPC 처리 코드를 그대로 쓴다. ["Under the mock runtime, native webview libraries are not executed."] [tauri-tests]
- `tauri::test`에는 `mock_builder`, `mock_context`, `noop_assets`, `mock_app`, `get_ipc_response`, `assert_ipc_response`가 있다. `test` 기능 플래그가 있어야 쓸 수 있고 문서에 "unstable"이라고 적혀 있다(tauri 2.12.5). [tauri-test-rs]

**실행한 테스트** (`exp/tests/order_path.rs`, tauri `=2.12.5`, `default-features = false, features = ["test"]`):

| 테스트 | 가짜 본인 확인 | 입력 | 기대 | 결과 |
|---|---|---|---|---|
| `webview_approve_call_with_denied_presence_sends_zero_orders` | 거부 | `approve_proposal {proposalId: 7}` | 오류 `PresenceDenied`, 본인 확인 호출 1회, 주문 0건 | 통과 |
| `webview_cannot_forge_approval_fields` | 거부 | `{proposalId: 7, approved: true, quantity: 1000}` | 역직렬화 거부(`deny_unknown_fields`), 본인 확인 0회, 주문 0건 | 통과 |
| `confirmed_presence_sends_exactly_one_order_once` | 승인 | 같은 승인 두 번 | 첫 번째 성공, 두 번째 `AlreadyUsed`, 주문 정확히 1건, 확인 창에 보인 요약 = 보낸 주문 | 통과 |
| `every_other_registered_command_sends_zero_orders_even_when_presence_would_confirm` | 승인 | 승인 외 명령과 없는 명령 | 본인 확인 0회, 주문 0건 | 통과 |
| `remote_origin_is_rejected_by_acl_before_reaching_the_command` | 승인 | 원격 출처(`http://tauri.localhost`)에서 승인 | ACL 거부, 본인 확인 0회, 주문 0건 | 통과 |

```
test webview_cannot_forge_approval_fields ... ok
test confirmed_presence_sends_exactly_one_order_once ... ok
test every_other_registered_command_sends_zero_orders_even_when_presence_would_confirm ... ok
test webview_approve_call_with_denied_presence_sends_zero_orders ... ok
test remote_origin_is_rejected_by_acl_before_reaching_the_command ... ok
test result: ok. 5 passed; 0 failed
```

**변이 확인**: `approve_and_submit`이 본인 확인 결과를 무시하고 항상 `Confirmed`로 처리하도록 손으로 바꿨다. 그러자 `webview_approve_call_with_denied_presence_sends_zero_orders`가 실패했다(4 통과, 1 실패). 원래 코드로 되돌려 두었다. cargo-mutants 27.1.0은 이 PC에 없어서 돌리지 못했다. [crates-versions]

**실험 중 발견한 사실 두 가지**

1. **macOS에서는 `tauri::test` 문서 예제의 URL을 그대로 쓰면 안 된다.** 예제에는 `url: "http://tauri.localhost"`라고 적혀 있다(주석에 "alternatively use tauri://localhost"). macOS에서 이 URL은 로컬 출처로 판정되지 않아 `"approve_proposal not allowed. Plugin not found"`로 거부된다. 소스를 보면 `is_local_url`이 Windows·Android에서만 `http://<scheme>.localhost` 형태를 로컬로 본다. 다른 플랫폼에서는 `tauri://localhost`여야 한다(`tauri-2.12.5/src/webview/mod.rs`의 `is_local_url`, `on_message`). 그리고 출처가 로컬이 아니면 앱 ACL 매니페스트가 없어도 ACL 검사를 강제한다("remote content can never reach custom commands unless an explicit `remote` capability has been configured"). 테스트 헬퍼는 macOS에서 `tauri://localhost`를 써야 한다. 반대로 원격 출처 거부는 그 자체로 고정할 가치가 있는 보안 성질이다(위 표의 마지막 행).
2. **`mock_context`는 빈 ACL(`Resolved::default()`)을 쓴다.** 그래서 이 테스트로는 실제 capability 파일(R-05가 권하는 `AppManifest::commands` 기반 앱 ACL)이 검증되지 않는다. 실제 설정까지 묶으려면 테스트에서 `mock_context` 대신 `tauri::generate_context!()`로 앱의 설정을 읽어야 한다. 문서 예제 주석이 그렇게 하라고 하지만("remove the string argument to use your app's config file") 이번에는 실행해 보지 않았다(미확인). [tauri-test-rs]

**O-13 테스트 구성 제안**

```
apps/desktop/src-tauri/
  src/lib.rs        pub fn build_app<R: Runtime>(builder: Builder<R>, ports: Ports, ctx: Context<R>) -> Result<App<R>>
  src/main.rs       실제 어댑터(키체인, Touch ID, 토스)로 Ports를 만들어 build_app(tauri::Builder::default(), ...) 호출
  tests/order_path.rs   mock_builder() + testkit 가짜 Ports로 build_app 호출
```

- 경로 고정 테스트는 제품 코드와 **같은 `build_app`**을 부른다. 테스트 전용 조립 경로가 따로 있으면 제품 경로를 고정하지 못한다.
- `testkit`은 `RecordingBroker`(보낸 주문 기록)와 `ScriptedPresence`(정해 둔 응답을 돌려주고, 호출 횟수와 확인 창에 보인 요약을 기록)를 제공한다. architecture.md 4.2의 "테스트용 생성자를 feature로 열지 않는다"는 원칙은 그대로 지켜진다. `ApprovedOrder`는 가짜가 만들지 않고 `order-execution` 안에서만 만들어진다. 가짜 증권사는 그것을 받기만 한다.
- 단언할 것:
  - (a) 본인 확인이 거부되면 0건
  - (b) WebView가 승인 필드를 위조하면 0건(DTO에 `deny_unknown_fields`)
  - (c) 승인 명령을 제외한 **모든 등록 명령**을 불러도 0건. 명령 목록은 tauri-specta가 생성한 목록에서 가져온다. 손으로 쓴 목록을 쓰면 새 명령이 빠진다.
  - (d) MCP 도구 레지스트리의 **모든 도구**를 무작위 인자(proptest 1.12.0)로 불러도 0건
  - (e) 스케줄러 레지스트리의 **모든 작업**을 고정 시계로 한 바퀴 돌려도 0건
  - (f) 본인 확인을 통과하면 정확히 1건이고, 보낸 주문 = 확인 창에 보인 요약 = 코어가 계산한 값
  - (g) 같은 토큰을 다시 쓰면 0건
  - (h) 원격 출처에서 승인하면 0건
- (d)와 (e)에서 "주문 0건"은 이미 `xtask arch-check`(의존 그래프)로 컴파일 단계에서 보장된다. 테스트는 그 보장이 조립 루트에서 무너지지 않았는지 확인하는 두 번째 겹이다.
- 이 테스트 파일과 `testkit`의 가짜 두 개는 보호 파일로 둔다. 에이전트가 단언을 약하게 고치지 못하게 하기 위해서다.
- 실제 Touch ID 창(`UserPresence` 실구현)은 자동화할 수 없다. GitHub의 macOS 러너는 Apple Virtualization Framework 위의 VM이라 생체 인식 장치가 없을 것으로 보이지만 확인하지 못했다(미확인). 실구현은 O-15(사용자 수동 소액 실주문)에서 사람이 확인한다. 자동 테스트는 "코어가 `UserPresence` 포트를 반드시 부른다"까지만 고정한다.

### 2.1 Tauri 앱 E2E 방법의 현재 지원 상태와 macOS 제약

| 방법 | macOS | 근거 |
|---|---|---|
| `tauri-driver` 직접 (2.1.0) | **불가**. "only Windows and Linux are supported on desktop", "macOS has no WKWebView driver tool available" | [tauri-webdriver] |
| WebdriverIO `@wdio/tauri-service` 1.5.1 + `embedded` 제공자(`tauri-plugin-wdio-webdriver` 1.5.1) | **가능**. Tauri 문서가 "this is how macOS is supported"라고 쓴다. 앱 안에서 W3C WebDriver 서버가 127.0.0.1:4445에 뜬다. 엔드포인트 47개 | [tauri-webdriver], [wdio-platform], [wdio-plugin-readme] |
| WebdriverIO + `crabnebula` 제공자 | 가능하지만 macOS에서는 유료 API 키(`CN_API_KEY`)가 필요하다 | [tauri-webdriver], [wdio-platform] |
| `tauri-plugin-playwright` 0.4.1 (커뮤니티) | 가능. 앱 안 제어 서버(기본 유닉스 소켓 `/tmp/tauri-playwright.sock`)로 Playwright 비슷한 API를 제공한다. macOS 네이티브 스크린샷은 CoreGraphics를 쓰며 화면 기록 권한이 필요하다. 0.x 버전 | [tauri-playwright] |
| Playwright로 WebView에 직접 붙기(CDP) | **불가**. CDP는 WebView2(Windows)만 지원한다 | [tauri-playwright] |
| 웹 빌드만 Chromium·WebKit에서 테스트(Vite + IPC 가짜) | 가능. 단 엔진이 WKWebView와 다르고 Rust 코어는 실행되지 않는다. WebdriverIO의 "browser mode"도 같은 방식이다 | [tauri-webdriver] |

macOS 제약 정리:

- WKWebView용 WebDriver를 Apple이 제공하지 않는다. 그래서 macOS에서는 **앱 바이너리 안에 자동화 서버를 넣는 방식만** 가능하다. 플러그인 README는 "This plugin exposes automation capabilities via HTTP. Never include it in production builds."라고 경고하고 `#[cfg(debug_assertions)]`로만 등록하라고 한다. [wdio-plugin-readme]
  - 이 저장소에서 `debug_assertions` 분기는 위험하다. 개발 중 쓰는 디버그 앱에도 자동화 서버가 열리고, 로컬의 다른 프로세스(AI CLI 포함)가 4445 포트로 승인 버튼을 누를 수 있다.
  - 그래서 별도 바이너리 target `e2e-app`에서만 플러그인을 등록하고, `arch-check`가 제품 바이너리의 의존 그래프에 `tauri-plugin-wdio-webdriver`·`tauri-plugin-playwright`가 없는지 검사하게 한다.
  - 이렇게 해도 WebDriver가 할 수 있는 일은 WebView 조작까지다. 승인 버튼을 눌러도 코어가 띄우는 본인 확인을 통과하지 못하므로 주문은 나가지 않는다. 이것 자체가 SEC-5의 시험이 된다. 단, 이를 위해 E2E 빌드는 가짜 `UserPresence`(항상 거부) + 가짜 증권사로 조립한다.
- 시스템 확인 창, 알림, 메뉴 막대, Touch ID 같은 네이티브 UI를 WebdriverIO 문서는 다루지 않는다(미확인). WebDriver 명세 범위 밖이므로 자동화할 수 없다고 보는 것이 안전하다.
- macOS 버전 요구사항은 문서에 없다(미확인).
- WebdriverIO 플러그인의 `2.0.0-next.0`(사전 배포판)이 2026-11-08에 올라왔다. 안정판은 1.5.1이다. [crates-versions]

### 2.2 IPC를 가짜로 바꿔 UI만 테스트하는 구성과 타입 일치

- **기반 API**: `@tauri-apps/api/mocks`(2.12.4)가 제공한다.
  - `mockIPC((cmd, args) => …)`는 `invoke`를 가로챈다.
  - `mockIPC(…, { shouldMockEvents: true })`는 `emit`/`listen`을 흉내 낸다. Tauri 2.7.0에서 추가됐고 `emitTo`, `emit_filter`는 흉내 내지 않는다.
  - `mockWindows`는 창이 있는 것처럼만 보이게 한다.
  - `clearMocks`는 테스트가 끝날 때마다 불러야 한다.
  - [tauri-mocking]
- **타입 일치**: tauri-specta 2.0.0-rc.28의 `Builder::mocks()`가 바인딩 옆에 `MockCommandHandlers` 타입과 `mockCommands` 도우미를 생성한다. [specta-docs], [specta-builder]
  - 핸들러는 인자를 객체 하나로 받고 성공 값을 돌려준다. 예외를 던지면 명령 실패가 된다.
  - **핸들러가 없는 명령은 `undefined`가 아니라 거부(reject)된다.** 가짜를 빠뜨렸다는 사실이 테스트 실패로 드러난다.
  - `@tauri-apps/api` 2.7 이상이 필요하다. `semantic_types`와 같이 쓰면 내보내기가 실패한다.
- **어긋남 방지 방법**:
  1. UI 테스트는 `mockIPC`를 직접 부르지 못하게 하고 생성된 `mockCommands`만 쓰게 한다. ESLint `no-restricted-imports`로 `@tauri-apps/api/mocks`를 `shared/ipc/testing` 밖에서 금지한다. 그러면 명령 이름, 인자, 반환 타입이 바뀌는 즉시 `tsc`가 테스트 파일에서 실패한다.
  2. 생성물(바인딩과 가짜 타입)은 harness.md 5장대로 CI에서 다시 만들어 차이가 있으면 실패한다.
  3. 가짜가 돌려주는 **값**이 Rust 직렬화 결과와 같은 모양인지는 타입만으로 보장되지 않는다. 예: 십진수 문자열 형식, 열거형 태그. 그래서 Rust 쪽 테스트가 명령별 대표 응답을 JSON 고정 자료로 내보내고(insta 1.49.0 스냅샷), TS 가짜는 그 JSON을 불러 돌려준다. 고정 자료는 Rust가 만들므로 어긋나면 Rust 스냅샷 차이로 드러난다.
  4. 오류 경로: 문서에 따르면 알 수 없는 명령, 잘못된 인자, ACL 거부 같은 **프레임워크 오류는 자유 형식 문자열로 거부된다**. 선언된 오류 타입 `E`는 명령이 직접 돌려준 오류만 설명한다. [specta-builder] 실험에서도 ACL 거부는 `"approve_proposal not allowed. Plugin not found"` 문자열이었다. UI는 오류 값을 스키마로 다시 파싱해야 하고, 가짜에는 "프레임워크 문자열 오류" 경우도 넣어야 한다(R-05와 같은 결론).
- **실행 환경**: Vitest 5.1.0 + Testing Library. jsdom에서 돌리거나 Vitest browser mode(Playwright 제공자)에서 돌린다. Tauri 문서 예제는 jsdom에서 `mockIPC`를 쓴다. [tauri-mocking]

### 2.3 Rust 코어를 UI 없이 통합 테스트하는 구성 (하루치 시나리오)

구성:

| 바깥 세계 | 테스트에서의 대체 | 비고 |
|---|---|---|
| 증권사 읽기 `BrokerRead` | `testkit::FakeBroker`(메모리 상태: 잔고, 보유, 시세 시계열) | 어댑터 계약 테스트는 따로 (아래) |
| 증권사 쓰기 `BrokerTrade` | `RecordingBroker` | 보낸 `ApprovedOrder` 기록 |
| AI `AiProvider` | (1) 순수 가짜: 스크립트대로 `propose_order` 등을 호출 (2) 가짜 CLI 실행 파일: 2.4 참고 | 시나리오 테스트는 (1), 어댑터 테스트는 (2) |
| 시계 `Clock` | `FixedClock`/`SteppedClock` | 벽시계 시각은 반드시 포트로 받는다 |
| 타이머·주기 | 스케줄러를 "지금 실행할 작업 = f(now, 상태)" 순수 함수 + 얇은 루프로 나누고, 테스트는 루프 대신 `tick(now)`를 직접 부른다 | 아래 함정 참고 |
| 본인 확인 `UserPresence` | `ScriptedPresence` | |
| 저장소 | 실제 SQLite를 임시 파일이나 메모리로 | 마이그레이션까지 함께 검증 |
| 뉴스·공시 | `FakeNewsSource`(녹화 고정 자료) | |

하루치 시나리오 예 (`tests/day_scenario.rs`):

1. 08:30 수집 → 공시 1건 도착 (`DisclosureArrived`)
2. 09:00 장 시작 → `tick` → 트리거 규칙이 AI 작업을 예약
3. 가짜 AI가 `propose_order` 호출 → 게이트 검증 → `ProposalCreated`
4. 사용자 승인 유스케이스 + `ScriptedPresence(승인)` → `RecordingBroker`에 1건, 감사 로그 해시 체인 검증
5. 15:30 장 마감 이후 `propose_order` → 거부 (O-16)
6. 하루가 끝났을 때의 이벤트 순서와 DB 상태를 insta 스냅샷으로 고정

**시계에 관해 확인한 사실**

- `#[tokio::test(start_paused = true)]`는 시계를 멈춘 채 시작한다. 할 일이 없으면 다음 타이머까지 시계를 **자동으로 앞당긴다**. current-thread 런타임에서만 동작하며 `test-util` 기능이 필요하다. [tokio-test-attr], [tokio-pause]
- **함정(실험으로 확인)**: 멈춘 시계에서 실제 자식 프로세스의 출력을 기다리면 런타임이 할 일이 없다고 판단하고 시계를 앞당긴다. 그 결과 `timeout(30s)`이 바로 터진다. 실제로 약 0.8초 걸리는 가짜 CLI를 `timeout(30s)`로 감쌌더니 가상 경과 시간 30초, `Err(Elapsed)`가 나왔다(`exp/tests/ai_replay.rs`의 `pitfall_paused_clock_auto_advances_and_fires_timeout_while_real_child_runs`).
  - 따라서 tokio 시계 멈춤은 **순수 가짜만 쓰는 시나리오 테스트**에만 쓴다. 실제 프로세스, 실제 소켓, 실제 SQLite 파일 I/O가 섞인 테스트에는 쓰지 않는다.
  - 문서는 블로킹 작업(`spawn_blocking`)이 도는 동안에는 자동 앞당김이 멈춘다고 쓴다. [tokio-pause]
- tokio 시계 멈춤은 `tokio::time::Instant`만 바꾼다. 벽시계 시각(장 운영 시간 판단 등)은 바꾸지 않는다. 그러니 architecture.md 3.3의 `Clock` 포트는 그대로 필요하다(위 문서 범위에서 추론).

**어댑터 계약 테스트**: `broker-toss`는 로컬 가짜 HTTP 서버(wiremock 0.6.5 또는 httpmock 0.9.0)에 녹화한 응답을 올리고, 기본 URL을 주입해 테스트한다. 녹화 자료는 `docs/toss-openapi-1.2.28.json` 스키마로 검증해서 스펙과 녹화가 어긋나면 실패하게 한다. [crates-versions]

### 2.4 자식 프로세스 AI CLI를 테스트에서 대체 (녹화 스트림 재생)

**방법**: `ai-claude-code` 어댑터는 실행 파일 경로를 설정(조립 루트)에서 받는다. 테스트는 그 경로에 가짜 실행 파일을 넣는다. `PATH`를 바꾸는 방식은 쓰지 않는다. 실제 `claude`가 잡힐 수 있기 때문이다.

**실행한 실험** (`exp/bin/fake-claude`, `exp/tests/ai_replay.rs`):

- 가짜 CLI: 고정 자료 JSONL을 한 줄씩 출력하고, 인자로 받은 종료 코드로 끝난다. 줄 사이 지연도 줄 수 있다.
- 고정 자료(`fixtures/propose-order.jsonl`): `system/init` → `assistant`(`tool_use: mcp__app__propose_order`) → `user`(`tool_result`) → `result`. **이번 자료는 공식 문서의 메시지 종류에 맞춰 손으로 만든 것이고, 실제 CLI에서 녹화한 것이 아니다.** 실제 CLI는 호출하지 않았다.
- 결과:

```
test replays_recorded_stream_and_surfaces_tool_use ... ok   (init → ToolUse(mcp__app__propose_order) → Result{is_error:false})
test broken_line_is_a_typed_error ... ok                    (JSON이 아닌 줄 → 타입 있는 오류)
test nonzero_exit_is_reported ... ok                        (종료 코드 3 전달)
test pitfall_paused_clock_auto_advances_and_fires_timeout_while_real_child_runs ... ok  (virtual elapsed = 30s)
```

- 이 가짜 CLI는 아래 2.7의 네트워크 차단 샌드박스 안에서도 그대로 동작했다. 자식 프로세스는 샌드박스를 물려받는다.

**권장 구성**

1. **녹화**: `xtask record-ai <시나리오>`가 실제 `claude -p --verbose --input-format stream-json --output-format stream-json`을 **가짜 MCP 서버**에 붙여 실행하고 stdout을 저장한다. 이 명령은 사람이 수동으로만 돌린다. CI와 테스트에서는 실행 금지(2.7의 샌드박스가 막는다).
   - 저장할 때 `session_id`, `uuid`, 비용, 경로를 고정 값으로 바꾼다(비식별화).
   - `system/init`에 들어 있는 CLI 버전과 `capabilities`를 고정 자료 머리에 남긴다.
   - stream-json의 이벤트 종류와 `system/init` 필드는 [cc-headless]에 있다.
2. **재생 모드 두 가지**
   - (a) **출력 재생**: 위 실험처럼 stdout만 재생한다. 스트림 파서, UI 스트림 전달, 오류 분류(`system/api_retry`, `result.is_error`, 0이 아닌 종료 코드, 깨진 줄)를 테스트한다.
   - (b) **대본 MCP 클라이언트**: 가짜 CLI가 고정 자료의 `tool_use`를 실제로 앱 MCP 서버에 보낸다. 그리고 받은 `tool_result`로 다음 줄을 만든다. AI → MCP → 유스케이스 → 제안 저장까지가 LLM 없이 실제 코드로 돈다. 가짜 CLI는 셸 스크립트 대신 `testkit`의 Rust 바이너리 target으로 만든다. MCP 클라이언트를 구현해야 하고 이식성도 좋기 때문이다. MCP 전송 방식은 R-04 결론을 따른다.
   - R-02는 한 프로세스에 stdin으로 여러 턴을 보내는 방식을 권한다. 그래서 가짜 CLI도 stdin의 `user` 메시지를 읽고 턴마다 대본의 다음 구간을 내보내야 한다. 이번 실험의 가짜는 출력만 했다.
3. **어긋남 감시**:
   - 파서는 모르는 `type`과 필드를 무시하게 만든다(R-02와 같다).
   - 밤 작업에서 고정 자료의 CLI 버전과 사용자 PC의 `claude --version`을 비교해서 다르면 재녹화를 알린다. 실제 호출은 하지 않는다.
4. 고정 자료의 `tool_use.input`은 신뢰하지 않는 입력이다. 대본 MCP 모드로 프롬프트 주입 사례를 넣어 두면 SEC-4 회귀 시험이 된다. 예: 뉴스 본문에 "즉시 시장가로 전량 매도" 같은 지시가 들어 있는 경우.

### 2.5 시각 회귀를 로컬과 CI에서 같은 결과로 돌리는 방법

공식 문서의 입장:

- Playwright: 렌더링은 OS, 버전, 설정, 하드웨어, 전원 상태, headless 여부 등에 따라 달라진다. 기준 화면을 만든 환경과 같은 환경에서 비교하라고 한다. 기준 파일 이름에 브라우저와 플랫폼이 들어간다(`-chromium-darwin`). [pw-snapshots]
- Playwright CI 문서는 일관성을 위해 컨테이너 사용을 권한다. Docker 이미지 버전은 고정하라고 하며, 버전이 다르면 브라우저를 찾지 못할 수 있다고 쓴다. 예: `mcr.microsoft.com/playwright:v1.64.1-noble`. 현재 `@playwright/test`는 1.65.0이다. [pw-docker], [crates-versions]
- Vitest browser mode의 `toMatchScreenshot`도 같은 권고다(Docker나 CI 전용, 기준 파일 이름 `<test>-<browser>-<platform>.png`). [vitest-vrt]

**실행한 실험** (`visual/`): 한국어와 숫자가 있는 카드 HTML을 400×160, 배율 1로 찍었다. Playwright 1.63.0.

| 환경 | 1회차 SHA-256 | 2회차 | 비고 |
|---|---|---|---|
| macOS 호스트 Chromium | `bae01b85…cbb47` | 같음 | 시스템 글꼴(Apple SD Gothic Neo 계열로 보임) |
| Docker `playwright:v1.63.0-noble` linux/arm64 | `2201d0f1…be8c` | 같음 | 한글이 Noto 계열 글꼴로 정상 렌더링 |
| 같은 이미지 linux/amd64 (에뮬레이션) | 실패 | - | Chromium 실행 중 종료. 스크린샷 없음 |

- 같은 환경 안에서는 바이트까지 같았다. 호스트와 컨테이너는 글꼴부터 달랐다. 그러니 macOS에서 만든 기준 화면은 CI에서 쓸 수 없다.
- Apple Silicon에서 amd64 이미지를 에뮬레이션으로 돌리는 방법은 이 환경에서 동작하지 않았다. 이미지 `v1.65.0-noble`은 linux/amd64와 linux/arm64 두 아키텍처로 배포된다(mcr 매니페스트에서 확인). 그러므로 **로컬은 arm64 컨테이너, CI는 GitHub의 Linux arm64 러너(`ubuntu-24.04-arm`, 2코어 분당 $0.005)에서 같은 이미지 태그**를 쓰면 아키텍처까지 맞출 수 있다. [gh-pricing]
- arm64와 amd64 사이의 픽셀 차이에 관해서는 공식 자료가 없다. 커뮤니티 보고는 "거의 보이지 않는 차이가 있다"와 "Linux끼리는 일관적이다"로 엇갈린다(미확인, 보조 출처). [community-arch]

권장:

1. 시각 회귀 대상은 **디자인 시스템 컴포넌트 목록 화면과 주요 화면**이다. Vite 웹 빌드에 `mockCommands`로 데이터를 넣어 Chromium으로 찍는다. 이것을 기준으로 삼는다. WKWebView와 엔진이 다르다는 한계는 받아들인다.
2. 글꼴은 앱에 번들한 웹 글꼴만 쓴다(시스템 글꼴 금지). 그래야 컨테이너와 실제 앱의 차이가 줄어든다. 애니메이션 끄기, 캐럿 숨기기, 고정 시계, 고정 난수를 테스트 설정에 넣는다.
3. 기준 화면 갱신(`--update-snapshots`)은 컨테이너 안에서만 하게 한다. `just visual-update`가 Docker를 부르게 만든다. 호스트에서 갱신하면 `-darwin` 파일이 생기므로, 이를 `xtask`가 "darwin 기준 파일 존재 = 실패"로 막는다.
4. 실제 WKWebView 스크린샷(`tauri-plugin-playwright`의 네이티브 스크린샷이나 WebDriver 스크린샷)은 **기준 비교 없이 보관만** 한다. 사람이 볼 수 있게 밤 작업 산출물로 남긴다. macOS 러너의 화면 배율과 글꼴이 사용자 PC와 같은지는 미확인이라 기준으로 쓰지 않는다.
5. 도구는 R-14의 결론과 맞춘다. Storybook(10.6.7) + test-runner를 쓸지, Vitest browser `toMatchScreenshot`을 쓸지, Playwright `toHaveScreenshot`을 쓸지는 컴포넌트 목록 도구 선택에 따른다. 어느 쪽이든 위 1~3이 핵심이다.

### 2.6 macOS 러너의 CI 비용과 시간, 무엇을 언제 돌릴 것인가

가격 (GitHub 문서, 2026-11-11 확인): [gh-pricing], [gh-billing]

| 러너 | 분당 |
|---|---|
| Linux 2코어 x64 | $0.006 |
| Linux 2코어 arm64 | $0.005 |
| macOS 3·4코어 (M1, `macos-latest`/`macos-15`/`macos-26`, 3코어 7GB RAM) | $0.062 |
| macOS 5코어 M2 Pro (larger) | $0.102 |

- 공개 저장소의 표준 러너는 무료다. 비공개 저장소는 플랜별 포함 금액(Free $12, Pro·Team $18)만큼 무료다. 포함 금액은 분이 아니라 돈으로 정해져 있다. 작업마다 분 단위로 올림한다. [gh-billing], [gh-pricing]
- 포함 금액을 macOS 분으로 환산하면 Free 약 190분, Pro 약 290분이다(문서의 단가로 계산한 추정).
- macOS 러너는 중첩 가상화를 지원하지 않는다. [gh-runners]
- 자체 호스팅 러너는 현재 문서상 무료다. 2026-03-01부터 분당 $0.002를 받겠다던 계획은 연기됐고 새 일정은 찾지 못했다(미확인). [gh-billing], [gh-selfhosted-postpone]
- **시간**: 실제 앱(wry 포함)의 macOS 러너 빌드 시간은 측정하지 못했다(미확인). 이 PC에서 `tauri`(`test` 기능, wry 제외)를 포함한 실험 crate는 처음부터 약 26초(사용자 CPU 96초)에 빌드됐다. M1 3코어 러너는 이보다 느릴 것이고 실제 앱은 의존성이 훨씬 많다. 캐시 없는 첫 빌드는 10분 단위를 예상하고 첫 CI 구성 때 측정한다.

**비용 예시 (가정: 비공개 저장소, 한 달 푸시 60회)**

- 안 A "푸시마다 macOS 15분": 60 × 15 × $0.062 = **$55.8/월**
- 안 B "푸시마다 Linux 15분 + 밤마다 macOS 25분": 60 × 15 × $0.006 + 30 × 25 × $0.062 = $5.4 + $46.5 = **$51.9/월**. 밤 작업을 "그날 `main`에 변경이 있을 때만"으로 줄이면 크게 준다. 예: 20일 × 25분 = $31.0 → 합계 약 $36.
- 안 C "macOS 작업은 사용자 Mac의 자체 호스팅 러너": 러너 비용 $0. 대신 그 Mac이 켜져 있어야 하고, 실제 키체인과 실제 계좌 설정이 있는 PC에서 CI를 돌리게 된다. 토스 키가 있는 PC에서 남이 바꿀 수 있는 코드를 실행하는 셈이다. 1인 저장소라도 에이전트가 커밋한 코드가 그대로 돈다는 점에서 위험하다.

**권장 배치**

| 언제 | 러너 | 내용 |
|---|---|---|
| 에이전트 훅·커밋 훅 (`just check`) | 로컬 | 형식, 린트, `arch-check`, 단위·속성 테스트, Rust 경로 고정 테스트, Vitest |
| 푸시 훅 (`just check-full`) | 로컬 | 위에 더해 변이 테스트(변경 범위만), 컨테이너 시각 회귀 |
| CI 푸시마다 | **Linux arm64** | `just check`와 같은 명령, 컨테이너 시각 회귀, 플랫폼 중립 crate의 하루 시나리오 테스트, 생성물 차이 검사 |
| CI `main` 병합과 밤 | **macOS** | `src-tauri` 경로 고정 테스트(macOS 출처 규칙을 실제 macOS에서), 앱 빌드, WebdriverIO E2E 연기 테스트 3~5개, WKWebView 스크린샷 보관 |
| CI 밤 (Linux) | Linux | 전체 변이 테스트(cargo-mutants 27.1.0), 오래 걸리는 속성 테스트, AI 고정 자료 버전 점검 |

- 경로 고정 테스트는 목 런타임이라 WebView가 없다. 그래서 Linux에서도 돌 가능성이 높다. 다만 실험에서 본 것처럼 로컬 출처 판정이 플랫폼마다 다르다(`http://tauri.localhost`는 Windows·Android에서만 로컬). macOS 판정은 macOS 러너에서 한 번 더 확인해야 한다. Linux에서 `src-tauri`를 빌드하려면 webkit2gtk 개발 패키지가 필요할 수 있다(미확인. wry를 끈 목 런타임 전용 테스트 crate로 나누면 피할 수 있다).
- harness.md 1장은 "CI는 푸시 훅과 같은 명령, 로컬과 CI가 다르지 않다"고 한다. 위 표는 "macOS 전용 검사는 CI 푸시마다 돌리지 않는다"는 예외를 만든다. 이 예외는 설계 문서에 명시해야 한다(5장).

### 2.7 테스트가 실제 토스 주소로 요청하지 못하게 막는 방법

토스 API의 기본 주소는 `https://openapi.tossinvest.com`이다(`docs/toss-openapi-1.2.28.json`의 `servers`). 세 겹으로 막는다.

1. **구조**: `broker-toss`는 기본 URL을 생성자 인자(`TossEndpoint` 타입)로만 받는다. 실제 주소 문자열은 조립 루트(`src-tauri/src/main.rs`)와 `toss-smoke`에만 둔다. `xtask`가 그 두 파일 밖에서 `tossinvest.com` 문자열을 발견하면 실패한다(이름 검색이지만 보조 검사로 충분하다). `testkit`과 테스트는 `TossEndpoint::local(addr)`만 만들 수 있다. 실제 주소 생성자는 조립 루트에서만 보이게 한다.
2. **HTTP 클라이언트**: reqwest 0.13.5 `ClientBuilder`에는 `https_only`, `no_proxy`, `dns_resolver`, `resolve`가 있다. [reqwest-builder] 제품 클라이언트는 `https_only(true)`로 만들고, 테스트 클라이언트는 루프백만 허용하는 `dns_resolver`를 주입한다. 이것은 cfg 분기가 아니라 생성자 주입이므로 harness.md의 "조건부 제외 금지"와 충돌하지 않는다.
3. **OS 수준 (실험으로 확인)**: cargo의 `target.<triple>.runner`는 `cargo run`·`cargo test`·`cargo bench`가 실행 파일을 실행할 때 쓰는 래퍼다. [cargo-config] 이것을 `sandbox-exec` 네트워크 차단 프로필로 지정하면 **모든 Rust 테스트 실행 파일과 그 자식 프로세스(가짜 AI CLI 포함)**의 외부 연결이 막힌다. 루프백은 열어 둔다.

```toml
# .cargo/config.toml
[target.aarch64-apple-darwin]
runner = ["sandbox-exec", "-f", "deny-net.sb"]
```
```
(version 1)
(allow default)
(deny network-outbound (remote ip "*:*"))
(allow network-outbound (remote ip "localhost:*"))
```

실행 결과 (`exp/tests/net_guard.rs`):

```
connect example.com:443 -> Err("Operation not permitted (os error 1)")      # runner 경유
test outbound_internet_is_blocked_for_test_binaries ... ok
test loopback_fake_servers_still_work ... ok
connect example.com:443 -> Ok(())                                            # 같은 바이너리를 runner 없이 실행(대조군)
```

이 runner 아래에서 Tauri 목 런타임 테스트와 가짜 CLI 테스트를 포함한 11개가 모두 통과했다.

- 주의:
  - `sandbox-exec`는 man 페이지에 "DEPRECATED"라고 적혀 있다. 하지만 macOS 26.5.2에서 동작했다. Apple이 언제 제거할지는 미확인이다.
  - DNS 조회는 막히지 않았다(시스템 데몬이 처리). 연결만 막혔다.
  - runner가 doctest와 build script에도 적용되는지 cargo 문서는 말하지 않는다(미확인). 그래서 build script에서 네트워크를 쓰지 않는 규칙을 따로 둔다.
- Linux CI에서는 같은 역할을 컨테이너 `--network none`이나 `unshare -n`으로 한다. 이번에 실행해 보지는 않았다.
- TS 쪽: Vitest와 Playwright 설정에서 `fetch`와 라우팅을 막는다. 실제 UI는 토스와 직접 통신하지 않는다(IPC만). 그러니 `fetch`/`XMLHttpRequest`/`WebSocket` 사용 자체를 린트로 금지하는 것이 더 단순하다.
- 하네스 자기 시험(harness.md 6장): "테스트에서 토스로 연결을 시도하는 위반 예제"를 두고 그 예제가 실패하는지 확인한다.

## 3. 선택지 비교표와 추천

### 3.1 E2E 방식

| 기준 | A. WebdriverIO + embedded 플러그인 | B. tauri-plugin-playwright | C. CrabNebula 제공자 | D. 웹 빌드 + Chromium(가짜 IPC) |
|---|---|---|---|---|
| macOS 실제 WKWebView | 예 | 예 | 예 | 아니오 |
| 관리 주체·성숙도 | WebdriverIO 조직, Tauri 공식 문서가 권장. 1.5.1 | 커뮤니티, 0.4.1 | 상용 | Playwright 1.65.0 |
| 비용 | 무료 | 무료 | macOS 유료 키 | 무료 |
| 앱에 자동화 서버 포함 | HTTP 127.0.0.1:4445 | 유닉스 소켓 | 플러그인 | 없음 |
| 코어(Rust)까지 실행 | 예 | 예 | 예 | 아니오 |
| 결정적 스크린샷 | 어려움 | 어려움(화면 기록 권한) | 어려움 | 컨테이너로 가능 |
| 네이티브 확인 창 | 불가 | 불가 | 불가 | 해당 없음 |

**추천**: UI 흐름과 시각 회귀의 주력은 D로 한다. A는 밤 연기 테스트 몇 개로만 쓴다. A는 별도 `e2e-app` 바이너리에서만 플러그인을 등록하고, 가짜 본인 확인과 가짜 증권사로 조립한다. B는 Playwright API를 하나로 통일하고 싶을 때의 대안이지만 0.x라 보류한다. C는 유료라 제외한다.

### 3.2 테스트 계층 (최종 추천)

| 계층 | 대상 | 도구 | 언제 |
|---|---|---|---|
| L0 정적 | 타입, 린트, `arch-check`, 생성물 차이, 금지 문자열 | tsc, ESLint, clippy, `xtask` | 매번 |
| L1 단위·속성 | `domain`, 게이트, 파서 | cargo test, proptest 1.12.0, Vitest 5.1.0 | 매번 |
| L2 계약 | `broker-toss`(로컬 HTTP 가짜 + OpenAPI 검증), `ai-claude-code`(가짜 CLI 출력 재생), 생성 바인딩과 Rust 응답 스냅샷 | wiremock/httpmock, insta 1.49.0 | 매번 |
| L3 코어 시나리오 | 하루치 시나리오, 고정 시계, 대본 MCP 가짜 CLI | `testkit`, cargo test | 매번 |
| **L4 경로 고정** | 조립된 앱의 IPC → 승인 → 본인 확인 → 증권사, 모든 명령·MCP 도구·스케줄러 작업 | `tauri::test` 목 런타임 + `testkit` 가짜 | 매번(로컬), macOS CI는 `main`·밤 |
| L5 UI | 화면별 로딩·빈·오류·정상 상태 | Vitest + Testing Library + tauri-specta `mockCommands` | 매번 |
| L6 시각 회귀 | DS 목록 화면, 주요 화면 | Playwright(또는 R-14 도구) in `mcr.microsoft.com/playwright:<고정>-noble` arm64 | 푸시 훅, CI Linux arm64 |
| L7 E2E 연기 | 실제 앱 실행, 승인 버튼만 누르면 0건 | WebdriverIO + `tauri-plugin-wdio-webdriver` (`e2e-app`) | 밤 macOS |
| L8 변이 | `order-execution`, 게이트, `domain` | cargo-mutants 27.1.0 | 푸시 훅(변경분), 밤(전체) |
| L9 사람 | 실제 Touch ID, 실제 토스 소액 주문 | O-15 | 출시 전 1회와 변경 시 |

**이 추천이 틀렸다고 판명될 조건**

- `tauri::test`가 Tauri 2.x 안에서 크게 바뀌거나, Tauri 3(현재 3.0.0-alpha.6)에서 없어지는 경우. 그러면 L4를 `src-tauri`의 명령 함수를 직접 부르는 방식으로 낮춰야 한다. ACL·역직렬화 경로 검증은 L7로 옮겨야 한다.
- `generate_context!()`를 목 런타임과 함께 쓸 수 없어서 실제 capability가 L4에서 검증되지 않는 경우. 그러면 capability 검증을 L7(밤)이 혼자 맡게 되어 피드백이 늦어진다.
- 컨테이너 Chromium과 실제 WKWebView의 렌더링 차이가 커서 L6이 실제 회귀를 놓치는 경우. 예: 같은 CSS인데 WKWebView에서만 깨지는 일이 반복될 때. 그러면 macOS 러너 + WKWebView 기준 화면을 다시 검토한다.
- `sandbox-exec`가 macOS에서 제거되는 경우. L0의 문자열 검사와 HTTP 클라이언트 주입만 남는다.
- 실제 앱 빌드가 macOS 러너에서 너무 오래 걸리는 경우(예: 30분 초과). 그러면 밤 macOS 작업을 주 1회로 줄이거나 안 C(자체 호스팅)를 다시 검토한다.

## 4. 위험과 미확인 항목

**위험**

- E2E 자동화 플러그인이 실수로 제품 빌드에 들어가면, 로컬의 아무 프로세스(AI CLI 포함)나 WebView를 조종할 수 있다. 주문은 본인 확인이 막지만 설정 변경 같은 다른 UI 동작은 막지 못한다. 별도 바이너리 + `arch-check`로 막아야 한다.
- 경로 고정 테스트가 제품 조립(`main.rs`)과 다른 조립 함수를 쓰면 의미가 없어진다. `main.rs`는 `build_app` 호출 한 줄과 실제 어댑터 생성만 담게 하고, 리뷰 대상으로 고정한다.
- `tauri::test`는 "unstable"이다. Tauri 업그레이드 때 L4가 깨질 수 있다. Tauri 버전을 `=`로 고정하고 업그레이드는 ADR로 다룬다.
- tauri-specta는 아직 RC(2.0.0-rc.28)이고 문서도 `=` 고정을 요구한다. `mocks()`는 rc.28 문서에서 확인했다. 언제 추가됐는지와 앞으로 바뀔지는 미확인이다.
- tokio 시계 멈춤과 실제 I/O를 섞으면 거짓 타임아웃이 생긴다(실험으로 확인). 린트로 막기 어려우므로 테스트 작성 규칙과 리뷰 항목으로 둔다.
- 녹화한 AI 고정 자료가 CLI 업데이트를 따라가지 못해 오래될 수 있다.

**미확인**

- `tauri::generate_context!()` + `mock_builder()`로 실제 capability(앱 ACL)를 적용한 L4 테스트가 되는지(실행 안 함).
- GitHub macOS 러너에서 WebdriverIO embedded 제공자가 문제없이 도는지, WKWebView 화면 배율·글꼴(실행 안 함).
- 실제 앱의 macOS 러너 빌드·테스트 시간.
- Playwright 컨테이너의 linux/arm64와 linux/amd64 사이 픽셀 일치 여부(이 PC에서 amd64 에뮬레이션은 Chromium 실행 실패).
- cargo `runner`가 doctest와 build script에 적용되는지. `sandbox-exec`의 향후 지원.
- `@wdio/tauri-service`의 명령 가짜(command mocking)가 Rust 명령을 백엔드에서 대체하는지 WebView 쪽에서만 가로채는지(문서에 설명 없음).
- 가짜 CLI 대본 모드가 R-04에서 정할 MCP 전송 방식과 맞는지.
- 실제 `claude` CLI로 녹화한 stream-json과 이번에 손으로 만든 고정 자료가 필드 단위로 같은지(실제 CLI는 호출하지 않았다).
- 2026-03-01 예정이던 자체 호스팅 러너 과금의 현재 상태.

## 5. 이 결과로 바뀌어야 할 설계 문서 항목

| 문서·위치 | 현재 | 바꿀 내용 |
|---|---|---|
| architecture.md 3.1 저장소 배치 | `src-tauri/` 조립 루트, IPC 명령 | `src-tauri`를 lib(`build_app<R: Runtime>(builder, Ports, ctx)`)와 bin(`main.rs`: 실제 어댑터 생성 + `build_app` 호출만)으로 나눈다. E2E용 별도 bin `e2e-app`을 추가한다(자동화 플러그인은 여기만) |
| architecture.md 3.2 의존 규칙 | 표 | "`tauri-plugin-wdio-webdriver`, `tauri-plugin-playwright` 등 자동화 플러그인은 `e2e-app`만 의존 가능. 제품 bin의 의존 그래프에 나타나면 실패"를 추가한다 |
| architecture.md 3.3 포트 / `testkit` | 가짜 어댑터 | `RecordingBroker`, `ScriptedPresence`, `FakeBroker`, `SteppedClock`, 가짜 AI CLI 바이너리(출력 재생과 대본 MCP 클라이언트 두 모드)를 `testkit` 구성 요소로 명시한다. `AiProvider` 구현은 실행 파일 경로를 주입받는다 |
| architecture.md 4.4 검증 | 경로 고정 테스트 3문장 | 2.0의 (a)~(h) 단언 목록. 실행 방식(`tauri::test` 목 런타임, 제품과 같은 `build_app`, macOS 출처 `tauri://localhost`). 명령·도구·작업 목록은 생성물과 레지스트리에서 가져온다는 규칙. 실제 Touch ID는 O-15 수동 확인 |
| architecture.md 8장 | "Tauri 앱의 E2E 테스트 방식 R-15" | ADR로 확정: 3.2의 L0~L9 계층 |
| harness.md 1장 | "CI = 푸시 훅과 같은 명령" | 예외 명시: macOS 전용 검사(L4의 macOS 재확인, L7)는 `main` 병합과 밤에만 돈다. 푸시마다 도는 CI는 Linux arm64 |
| harness.md 1장 `just check-full` | 변이 + 시각 회귀 | 시각 회귀는 고정 태그 Playwright 컨테이너 안에서만 돈다. 기준 갱신 명령도 컨테이너 전용이고, `-darwin` 기준 파일이 있으면 실패 |
| harness.md 4.3 도구 | 테스트 후보 Vitest + Testing Library | 확정 후보에 tauri-specta `mockCommands`. `@tauri-apps/api/mocks` 직접 가져오기를 `shared/ipc/testing` 밖에서 금지(ESLint) |
| harness.md 5장 | 생성물 재생성 차이 검사 | 가짜 타입(`MockCommandHandlers`)도 생성물에 포함. 명령별 대표 응답 JSON은 Rust가 생성해 TS 가짜가 쓴다 |
| harness.md 6장 테스트 기준 | "테스트 빌드에서 토스 주소로 가는 요청은 실패한다" | 방법 명시: (1) `TossEndpoint` 주입 + 실제 주소 문자열 위치 제한 (2) 테스트 클라이언트 루프백 전용 resolver (3) `.cargo/config.toml` runner로 `sandbox-exec` 네트워크 차단(macOS), 컨테이너 `--network none`(Linux). 위반 예제 테스트 포함 |
| harness.md 6장 | (없음) | "tokio `start_paused`는 순수 가짜만 쓰는 테스트에서만 쓴다"는 규칙을 추가한다. 경로 고정 테스트 파일과 `testkit`의 주문·본인 확인 가짜를 7.1 보호 파일에 추가한다 |
| tasks.md O-13 | 선행 O-04, O-08, R-15 | 완료 기준에 "의도적 우회 변이(본인 확인 무시)를 넣으면 테스트가 실패함을 cargo-mutants로 확인"과 "원격 출처 승인 0건"을 추가한다 |
| tasks.md DS-07 | 시각 회귀 | 기준 환경 = Playwright 컨테이너(linux/arm64, 태그 고정), 웹 글꼴 번들 |
| research.md R-14 | 시각 회귀 도구 비교 | R-15의 "컨테이너 기준 환경" 제약을 입력 조건으로 넣는다 |

## 6. 출처 목록

모두 2026-11-11에 확인했다.

- [tauri-webdriver] Tauri, "WebDriver" (마지막 수정 2026-06-29): https://v2.tauri.app/develop/tests/webdriver/
- [tauri-tests] Tauri, "Tests" (마지막 수정 2026-06-29): https://v2.tauri.app/develop/tests/
- [tauri-mocking] Tauri, "Mock Tauri APIs": https://v2.tauri.app/develop/tests/mocking/
- [tauri-test-rs] docs.rs, `tauri::test` (tauri 2.12.5): https://docs.rs/tauri/latest/tauri/test/index.html. 소스: crates.io 배포본 `tauri-2.12.5/src/test/mod.rs`, `src/webview/mod.rs`(`is_local_url`, `on_message`), `src/ipc/authority.rs`
- [wdio-platform] WebdriverIO, "Tauri Platform Support": https://webdriver.io/docs/desktop-testing/tauri/platform-support
- [wdio-tauri] WebdriverIO, "Tauri": https://webdriver.io/docs/desktop-testing/tauri
- [wdio-plugin-readme] docs.rs, tauri-plugin-wdio-webdriver 1.5.1 README: https://docs.rs/crate/tauri-plugin-wdio-webdriver/latest/source/README.md
- [tauri-playwright] docs.rs, tauri-plugin-playwright 0.4.1 README: https://docs.rs/crate/tauri-plugin-playwright/latest/source/README.md
- [specta-docs] docs.rs, tauri-specta 2.0.0-rc.28: https://docs.rs/tauri-specta/2.0.0-rc.28/tauri_specta/
- [specta-builder] docs.rs, `tauri_specta::Builder` (`mocks`, `error_handling`): https://docs.rs/tauri-specta/2.0.0-rc.28/tauri_specta/struct.Builder.html
- [specta-repo] GitHub, specta-rs/tauri-specta: https://github.com/specta-rs/tauri-specta
- [tokio-test-attr] docs.rs, `tokio::test`: https://docs.rs/tokio/latest/tokio/attr.test.html
- [tokio-pause] docs.rs, `tokio::time::pause` / `advance`: https://docs.rs/tokio/latest/tokio/time/fn.pause.html, https://docs.rs/tokio/latest/tokio/time/fn.advance.html
- [cc-headless] Claude Code 문서, "Run Claude Code programmatically" (stream-json, `system/init`, `system/api_retry`): https://code.claude.com/docs/en/headless
- [pw-snapshots] Playwright, "Visual comparisons": https://playwright.dev/docs/test-snapshots
- [pw-docker] Playwright, "Docker": https://playwright.dev/docs/docker , CI: https://playwright.dev/docs/ci . 이미지 아키텍처: `https://mcr.microsoft.com/v2/playwright/manifests/v1.65.0-noble` (linux/amd64, linux/arm64)
- [vitest-vrt] Vitest, "Visual Regression Testing": https://vitest.dev/guide/browser/visual-regression-testing
- [community-arch] 보조 출처(커뮤니티): https://lightrun.com/answers/microsoft-playwright-question-visual-testing-in-docker-on-different-cpu-architecture , https://awesome.ecosyste.ms/projects/github.com%2Fnickofthyme%2Fplaywright-docker-arch-matrix
- [gh-pricing] GitHub Docs, "Actions runner pricing": https://docs.github.com/en/billing/reference/actions-runner-pricing
- [gh-billing] GitHub Docs, "GitHub Actions billing": https://docs.github.com/en/billing/concepts/product-billing/github-actions
- [gh-runners] GitHub Docs, "GitHub-hosted runners": https://docs.github.com/en/actions/reference/runners/github-hosted-runners
- [gh-selfhosted-postpone] GitHub Changelog, "Update to GitHub Actions pricing" (2025-12-16, 연기 공지 포함): https://github.blog/changelog/2025-12-16-coming-soon-simpler-pricing-and-a-better-experience-for-github-actions/ , "Reduced pricing for GitHub-hosted runners usage" (2026-01-01): https://github.blog/changelog/2026-01-01-reduced-pricing-for-github-hosted-runners-usage/
- [cargo-config] Cargo Book, "Configuration" (`target.<triple>.runner`): https://doc.rust-lang.org/cargo/reference/config.html
- [reqwest-builder] docs.rs, reqwest 0.13.5 `ClientBuilder`: https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html
- [crates-versions] crates.io API와 npm registry에서 2026-11-11에 조회한 최신 버전:
  - crates: tauri 2.12.5(사전 배포 3.0.0-alpha.6), tauri-specta 2.0.0-rc.28, tauri-plugin-wdio-webdriver 1.5.1(사전 배포 2.0.0-next.0), tauri-driver 2.1.0, wiremock 0.6.5, httpmock 0.9.0, insta 1.49.0, proptest 1.12.0, cargo-mutants 27.1.0
  - npm: @tauri-apps/api 2.12.4, @wdio/tauri-service 1.5.1, @playwright/test 1.65.0, vitest 5.1.0, storybook 10.6.7
  - 조회 주소: `https://crates.io/api/v1/crates/<이름>`, `https://registry.npmjs.org/<이름>/latest`
- 이 저장소: `docs/toss-openapi-1.2.28.json`(servers: `https://openapi.tossinvest.com`), `docs/research/R-02-claude-code-runtime.md`, `docs/research/R-05-tauri-architecture.md`
- `sandbox-exec(1)` man 페이지(macOS 26.5.2, "DEPRECATED" 표기)
