# ADR 0002: 아키텍처 v0.2 구조 결정

- 날짜: 2026-11-11
- 상태: 확정
- 관련 문서: [architecture.md](../architecture.md) v0.2, [open-issues.md](../open-issues.md), [research/](../research/)

리서치 16건과 사용자 결정(open-issues.md 1장, 3장, 4.1)을 반영해 확정한 구조 결정이다. 우선순위는 open-issues.md의 결정 > 리서치 결과 > architecture.md v0.1이다. 버전은 리서치 파일에 적힌 2026-11-11 기준 값이다.

## 1. AI와 실행 구조

| # | 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|---|
| 1 | AI는 채점할 수 있는 사건의 확률 p와 근거만 낸다(`submit_estimate`). 방향, 수량, 가격, 임계값은 `domain`이 정한다. `propose_order`와 `propose_idea`를 없앤다 | 결과를 일봉으로 판정할 수 있고, AI가 주문 내용을 정하는 통로가 타입에서 사라진다 | AI가 종목·방향·수량·가격을 제안 | open-issues 1장, R-12 |
| 2 | 추정 → 신호 → 주문 제안 흐름. `Proposal<Draft>`는 코드만 만든다. 장이 닫혀 있으면 신호만 남긴다 | 아이디어와 주문 제안의 이중 구조가 필요 없다 | 아이디어(만료 없음) + 주문 제안 | open-issues 1.5 |
| 3 | 수집 실행(종목명·코드만, 웹 검색(웹 읽기는 S-07), 앱 MCP 없음, 구조화 출력)과 분석 실행(보유 내역, 앱 MCP만, 웹 없음)으로 나눈다 | 보유 내역을 아는 실행에 웹으로 직접 나가는 통로가 없다. 남는 것은 종목·주제 선택의 낮은 대역 통로다. `read_article`은 언론사 RSS 도메인의 주소만 받는다. 수집 실행은 새어 나갈 비밀이 없다 | 한 실행에 웹 검색을 주고 검색어 게이트(`--permission-prompt-tool`)로 거르기 | open-issues 3장, R-12 |
| 4 | 분석 실행이 웹 조사를 원하면 `request_web_research(종목 코드 목록, 주제 항목)`만 부른다. 종목은 코어가 아는 종목만, 주제는 코드가 정한 주제 목록의 항목만 받는다. 분석 실행이 쓴 자유 문장은 수집 실행으로 넘어가지 않는다. 사용자가 채팅에 직접 쓴 문장으로 조사할 때는 코어가 저장된 사용자 메시지 원문을 넘긴다 (2026-11-11 리뷰로 고침) | 분석 실행의 글이 검색어가 되는 통로를 좁힌다. 숫자를 글자로 쓴 말은 문자열 검사로 막지 못한다. 사용자가 정한 주제는 사용자 문장 원문으로 찾는다 | 분석 실행이 검색어를 직접 써서 넘기기. SEC-9 검사를 통과한 짧은 주제어를 받기(v0.2 초안) | open-issues 3장, design SEC-9 |
| 5 | AI CLI는 사용자의 기존 로그인을 쓴다. 환경 변수 허용 목록, `--restricted --disable-slash-commands`와 권한 옵션, 빈 작업 폴더, 시작 점검으로 격리한다 | 사용자가 따로 로그인하지 않는다(P-2). 실측에서 훅, 플러그인, 설정, CLAUDE.md가 차단되고 구독 인증이 유지됐다 | 앱 전용 `CLAUDE_CONFIG_DIR`·`CODEX_HOME` | I-03, R-02, R-03 |
| 6 | 시작 점검에서 도구 목록과 `mcp_servers[].status == "connected"`를 함께 본다 | Claude Code는 MCP 접속에 실패해도 종료 코드 0으로 끝난다 | 도구 목록만 검사 | R-02, R-04 |
| 7 | 도구 결과는 응답 하나 3만 자 이하, 목록은 커서로 나눈다. 예약 작업과 채팅은 기록을 남기지 않는 옵션으로 돌린다. 채팅은 대화 동안 프로세스 하나를 유지하고 앱을 다시 켜면 앱이 저장한 대화를 다시 넣는다 | 5만 자를 넘는 결과는 CLI 폴더에 평문으로 저장된다 | `maxResultSizeChars`로 한도 올리기 | I-04, R-04 |
| 8 | Claude 구독 하나로 전부 돌리되 `AiProvider` 어댑터 구조와 Codex 어댑터는 유지한다 | 약관 위험은 사용자가 감수했다. 필요하면 작업별 제공자를 설정으로 바꾼다 | 확률 추정만 Codex나 API 키로 | open-issues 2.1 |

## 2. 프로세스와 MCP

| # | 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|---|
| 9 | MCP 전송은 실행별 Unix 소켓(0700 폴더, 0600 소켓, 104바이트 이내) + CLI가 띄우는 stdio 브리지. 실행별 256비트 토큰은 환경 변수로만 전달 | 포트가 없어 브라우저·DNS 재바인딩 경로가 없다. 두 CLI 모두 stdio로 실제 호출에 성공했다 | 127.0.0.1 HTTP + bearer + Origin/Host 검사 | I-01, R-04, R-16 |
| 10 | MCP 라이브러리 `rmcp` `=3.9.0` | 두 CLI의 프로토콜 세대를 모두 받는 것을 확인한 유일한 선택지 | rust-mcp-sdk 2.0.0, pmcp 2.23.0 | R-04 |
| 11 | `mcp-bridge`는 작업 공간 crate에 의존하지 않는 별도 실행 파일. 앱 번들에 넣는다 | 브리지가 앱 코드에 닿지 않는다 | 앱 실행 파일에 브리지 모드를 두기 | R-04 |
| 12 | MCP 서버가 토큰의 실행 종류로 도구 묶음을 정한다 | CLI 플래그에만 기대지 않는다 | CLI의 허용 도구 목록만으로 제한 | open-issues 3장 |
| 13 | 앱은 한 프로세스로 메뉴 막대에 상주한다(`prevent_close`, 트레이, `Accessory`). 자동 시작 `tauri-plugin-autostart` 2.7.0(기본 켬, design 8장. 재부팅 뒤에도 예약 작업이 이어지게 2026-11-11 리뷰로 고침), 중복 실행 방지 `tauri-plugin-single-instance` 2.5.5 | 일별 평가와 예약 작업에 필요하고 구현 비용이 작다 | 창을 닫으면 종료, 별도 백그라운드 프로세스 | I-09, R-05 |
| 14 | AI CLI는 `tokio::process`로 Rust에서만 띄운다. 새 프로세스 그룹, `kill_on_drop`, stdin 유지, 종료 시 SIGTERM 후 SIGKILL, 시작 시 남은 그룹 정리 | 부모가 SIGKILL로 죽으면 자식이 살아남는 것을 실험으로 확인했다 | 셸 플러그인, sidecar, drop에만 의존 | R-05 |
| 15 | 앱 샌드박스, 업데이터, 공증을 쓰지 않는다. `sandbox-exec`는 추가 겹으로만 둔다 | 앱 샌드박스는 자식 CLI에 상속되어 CLI 인증을 막는다. 개인용 직접 빌드다. `sandbox-exec`는 폐기 예정 표시가 있다 | 앱 샌드박스, Tauri 업데이터 | R-05, R-16 |

## 3. 코드 구조와 의존

| # | 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|---|
| 16 | 루트 가상 작업 공간, `[workspace.dependencies]`와 `[workspace.lints]`, `rust-toolchain.toml`에 1.99.1 고정(Tauri 2.12는 최소 1.90, ADR 0003) | 핵심 crate 테스트가 WebView 스택 없이 돈다. Tauri 2.12와 rusqlite 0.41의 MSRV | `src-tauri`를 작업 공간 루트로 | R-05, I-17 |
| 17 | Tauri `2.12.5`. `tauri`, `tauri-plugin-*`, `specta`는 `src-tauri`와 `e2e-app`에만 | 같음 | 여러 crate가 tauri에 의존 | R-05 |
| 18 | `src-tauri`를 lib(`build_app`)와 bin(`main`, `gen-bindings`)으로 나눈다 | 경로 고정 테스트가 제품과 같은 조립 함수를 불러야 한다 | 테스트 전용 조립 경로 | R-15 |
| 19 | 의존 방향은 `xtask arch-check`(`cargo_metadata` 0.23.1)가 단독으로 검사한다. 허용 간선 표, 전이 금지 표, 재수출 금지, 외부 crate 위치 | cargo-deny는 작업 공간 간선과 전이 경로를 막지 못한다(실측) | cargo-deny `bans.deny` + `wrappers` | R-09 |
| 20 | 승인 명령을 `approval-command` crate로 꺼낸다. `src-tauri`는 `order-execution`에 직접 의존하지 않는다 | 모듈 단위 제한은 이름 검색이라 `use … as`나 재수출로 피할 수 있다. 직접 의존이 없으면 이름으로 부를 수 없다 | `src-tauri` 안 승인 모듈 하나로 제한하고 이름 검색 | R-09 |
| 21 | 토스 어댑터를 `broker-toss`(GET과 토큰 발급만), `broker-toss-trade`(쓰기 메서드의 주문 경로, 1차는 주문 생성만, `order-execution` 의존), `broker-toss-schema`(생성물)로 나눈다. 위치 검사는 경로 문자열이 아니라 HTTP 메서드와 경로의 조합으로 한다 | 주문 경로 요청을 만드는 코드가 승인 증거 없이는 존재하지 않는다. 주문 조회와 주문 생성은 경로가 같고 메서드만 다르다 | `broker-toss` 하나가 읽기와 주문을 함께 구현. 경로 문자열 검사(v0.2 초안) | R-01, R-09, reviews/DESIGN-v0.2 #19 |
| 22 | 토스 타입은 `typify` 0.8.0으로 `components.schemas`만 생성한다. HTTP 호출은 손으로 쓴다 | OpenAPI 3.1 원본을 그대로 받고 생성물이 컴파일된다 | progenitor 0.16.0, oas3-gen 0.29.0, openapi-generator 7.27.0 | R-09 |
| 23 | `testkit`은 `order-execution`에 의존하지 않는다. 가짜 본인 확인은 `application`의 `UserPresence` 포트 가짜 `ScriptedPresence` 하나로 `testkit`에 두고 `order-execution` 단위 테스트와 L4가 함께 쓴다. `RecordingBroker`는 `trade-fakes`에 둔다. `trade-fakes`는 `src-tauri`의 개발 의존과 `e2e-app`만 쓸 수 있다. `impl UserPresence`는 `platform-macos`와 `testkit`에만 둔다. `UserPresence`의 별칭 가져오기와 재수출은 금지한다 | `testkit → order-execution`은 주문 차단 구조와 충돌한다. crate 내부 `cfg(test)` 모듈은 다른 crate의 테스트가 쓸 수 없고, harness가 `cfg(test)` 밖의 `cfg`와 `[features]`를 막는다. 테스트용 생성자를 열지 않는다 | `testkit`이 `order-execution`에 의존. `order-execution` 내부 테스트 모듈에 가짜 본인 확인 | I-14, R-09, R-15 |
| 24 | `UserPresence` 구현은 `platform-macos`(`objc2-local-authentication` 0.3.2, `rfd` 0.18.0). `Notifier`는 `src-tauri/platform`(`tauri-plugin-notification` 2.8.1, Rust에서만 호출) | 본인 확인 어댑터가 tauri에 의존하지 않는다 | `src-tauri/platform`에 본인 확인 | R-05, R-16 |
| 25 | `domain`은 `rust_decimal`(`maths`)과 시간 타입 crate에만 의존하고 직렬화 derive를 붙이지 않는다. 각 경계가 DTO로 변환한다 | specta·serde가 domain에 들어오지 않는다 | domain 타입에 직접 derive | R-05, R-11 |
| 26 | 십진수는 `rust_decimal` 1.43.0, `checked_*`만. 통계 연산도 `MathematicalOps` | 28자리, `Copy`, 할당 없음. 연산자는 넘치면 panic한다 | bigdecimal 0.4.11, fastnum 0.7.5 | R-09, R-11 |

## 4. 주문 안전

| # | 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|---|
| 27 | `UserApproval`이 `Proposal<Validated>`를 소유하고 `self`를 소비해 `ApprovedOrder`가 된다 | 승인한 제안과 보내는 제안의 불일치를 표현할 수 없어 방어 분기 없이 커버리지 100% | 제안 ID 비교 분기 + "95% + 변이 생존 0" | R-09, I-13 |
| 28 | `Proposal<Validated>`는 `Clone`이 없고 저장·복원하지 않는다. 승인할 때마다 `validate`를 다시 거친다 | 복원 생성자가 곧 검증 우회다 | 검증 상태 저장(필요하면 HMAC) | R-09 |
| 29 | 전송 직전 한 트랜잭션으로 소비 기록(제안 ID 고유 제약)과 전송 전 기록을 쓴다. `clientOrderId`는 제안 ID에서 결정적으로 만든다. 감사 로그 쓰기 실패 시 전송하지 않는다 | 중복 전송과 기록 없는 전송을 막는다 | 타입만으로 한 번 전송 보장 | R-09, R-16, I-07 |
| 30 | 본인 확인 뒤에는 값이 허용 범위 안에 있는지만 다시 보고, 다시 계산해서 보내지 않는다 | 사용자가 본 요약과 보내는 주문이 같아야 한다 | 본인 확인 뒤 최신 값으로 다시 계산해 전송 | R-09, R-16 |
| 31 | 본인 확인은 플러그인 없이 코어가 직접: 네이티브 요약 창 + `LAContext.evaluatePolicy(.deviceOwnerAuthentication)`. 승인 IPC는 `proposal_id`만 받는다. 확인 창은 한 번에 하나, 거절 후 재요청 제한 | 플러그인의 macOS 지원 여부와 무관해진다. Touch ID를 쓸 수 없을 때 암호로 넘어간다 | `tauri-plugin-biometric` 2.5.0 | I-02, R-05, R-16 |
| 32 | 위험 설정 변경(SEC-12의 목록)은 코어가 띄우는 네이티브 확인 창을 거친다. macOS 본인 확인(Touch ID)은 요구하지 않는다 | WebView가 설정을 바꿔 안전장치를 끄는 경로를 막는다. 본인 확인은 주문 때만 요구한다(P-9, ADR 0001 20번) | 일반 설정 명령으로 처리. 위험 설정 변경에도 Touch ID(R-16 제안) | R-16, P-9 |
| 33 | Secure Enclave 승인 서명은 2단계. `UserApproval`에 자리만 둔다 | 데이터 보호 키체인과 서명 체계가 먼저 필요하다 | 1단계부터 서명 | R-16 |
| 67 | `BrokerTrade`는 주문 생성만 한다. 정정·취소는 1차 범위 밖이고 토스증권 앱에서 한다. 1억 원의 80% 이상이면 처음부터 최종 확인에서 고액 확인을 받고, 그래도 거절되면 실패로 둔다(재전송 없음) (2026-11-11 리뷰) | 정정·취소 경로는 승인 구조를 따로 설계해야 한다. 거절 뒤 새 키 재전송은 결정적 `clientOrderId`와 소비 기록과 부딪힌다 | 앱 안 정정·취소, 고액 거절 뒤 새 키로 재전송(v0.2 초안) | reviews/DESIGN-v0.2 #1, #18 |
| 34 | 경로 고정 테스트(L4)는 `tauri::test` 목 런타임과 제품의 `build_app`으로, 명령·도구·작업 목록은 생성물과 레지스트리에서 가져온다. 단언은 architecture.md 4.6의 (a)~(n) 하나뿐이고 harness.md 6.5는 그것을 가리킨다 | WebDriver로는 macOS 본인 확인 창을 자동화할 수 없다. 손으로 쓴 목록은 새 명령을 놓친다 | WebDriver E2E로 고정 | R-15, R-16 |

## 5. IPC와 이벤트

| # | 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|---|
| 35 | `tauri-specta` `=2.0.0-rc.28` + `specta-typescript` `=0.0.15`, `ErrorHandlingMode::Result`, `gen-bindings`에서 생성, 재생성 후 diff 검사 | 명령·이벤트·Channel을 한 생성물로 묶고, 큰 정수와 `serde(with)` 불일치를 거부한다 | ts-rs 12.0.1, taurpc 0.8.2, typeshare 1.0.5 | R-05 |
| 36 | 도메인 알림은 ID만 담은 타입 이벤트 + 쿼리 재조회. 채팅은 요청별 `Channel<ChatChunk>`. 코어는 WebView가 쏜 이벤트를 받지 않는다 | 이벤트는 대량 전송용이 아니다. 놓쳐도 재조회로 회복된다 | 이벤트에 전체 페이로드 | R-05 |
| 37 | 앱 ACL(`AppManifest::commands`), capability 명시, `core:default`·`core:event:allow-emit`·플러그인 권한을 WebView에 주지 않음. `xtask capability-check`. 보호된 `build.rs`는 명령 목록을 생성 파일에서 읽고, capability의 명령 허용 항목도 같은 목록에서 생성한다. IPC 생성물은 보호하지 않고 다시 생성한 차이로만 지킨다 (2026-11-11 리뷰로 고침) | 권한을 받은 명령만 WebView가 부를 수 있다(소스 확인). 명령 하나를 추가할 때마다 보호 파일 세 곳을 바꾸면 화면·채팅·주문 작업이 모두 사용자 승인 뒤에 멈춘다 | 기본값(모든 창이 모든 명령). 명령 목록을 보호 파일 세 곳에 손으로 적기(v0.2 초안) | R-05, R-16, reviews/DESIGN-v0.2 #2 |
| 38 | Tauri isolation 패턴은 쓰지 않는다. CSP, `freezePrototype`, 원격 콘텐츠 금지는 쓴다 | 코어가 모든 입력을 다시 검증하고 주문은 WebView 밖에서 확인받으므로 얻는 것이 적다 | isolation 패턴 | R-16 |

## 6. 저장소와 비밀

| # | 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|---|
| 39 | `rusqlite` `=0.41.1`(`bundled-sqlcipher`, `backup`, `hooks`), 전용 쓰기 스레드 하나 | 최신 SQLCipher·SQLite, authorizer와 백업 API. 주변 crate가 0.42를 아직 따라오지 않았다 | sqlx 0.9, diesel, turso, `tauri-plugin-sql`, `tokio-rusqlite` | R-06 |
| 40 | SQLCipher(CommonCrypto), 키체인의 32바이트 원시 키 | 추가 설치 없음, 페이지 HMAC, 암호문 KDF 비용 없음 | 암호문 + PBKDF2, SQLite3MultipleCiphers, FileVault만 | R-06 |
| 41 | 금액 열은 STRICT TEXT 정규 십진수 + CHECK + 통화 열. SQL에서 금액 산술·합산·정렬 금지 | NUMERIC은 REAL로 바뀌어 정밀도를 잃는다(실측). 토스 문자열과 손실 없이 왕복 | NUMERIC, 고정 배율 INTEGER | R-06 |
| 42 | 마이그레이션은 직접 만든 실행기 + SHA-256 원장 + git 불변 검사 + 스키마 스냅샷 테스트 | `rusqlite_migration`은 적용된 파일 수정을 잡지 못했다(실측) | rusqlite_migration, refinery | R-06 |
| 43 | 감사 로그는 "변조를 탐지할 수 있는 로그": 트리거, 연속 순번, authorizer, DEFENSIVE, 해시 체인, HMAC, 키체인 머리값, 토스 주문 내역 대조 | 단독 사용자 PC에서 지울 수 없는 로그는 만들 수 없다 | "지울 수 없는 로그", 트리거와 해시 체인만 | I-10, R-06, R-16 |
| 44 | 한국어 검색은 앱 측 바이그램 + `unicode61` contentless-delete FTS5 | 2글자 질의와 조사를 모두 잡았다. trigram은 2글자를 놓쳤다 | trigram, 형태소 분석기 | R-06 |
| 45 | 키체인은 데이터 보호 키체인(`security-framework` 3.7.0), 팀 접근 그룹, `ThisDeviceOnly`. 개발 빌드는 별도 서비스 이름과 가짜 키 | 다른 프로세스를 프롬프트 없이 거부하는 유일한 방식 | 파일 기반 키체인(`keyring`), 자체 금고 파일 | R-16, U-02 |
| 46 | 비밀은 `secrecy` 0.10.3 래퍼, `Debug`·`Display`·`Serialize`·`Clone` 없음, 헤더는 `set_sensitive` | 로그·오류로 새는 경로를 줄인다 | 일반 문자열 | R-16 |
| 47 | 거래내역은 최근 14일을 다시 받아 같은 행의 개수까지 맞추는 멀티셋. 사라진 행은 표시만 | 거래내역에 고유 ID가 없다 | 내용 해시로 중복 제거 | I-06, R-01 |
| 48 | 토스 스펙 변경은 파일 해시로 감지한다 | 같은 버전 번호로 내용이 바뀐다 | 버전 번호 비교 | I-08, R-01 |

## 7. 스케줄러와 데이터 출처

| # | 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|---|
| 49 | 스케줄러는 짧은 주기로 깨어 `Clock` 포트의 벽시계와 비교한다. 놓친 작업은 작업별 정책 | macOS `Instant`는 잠자는 동안 멈춘다 | `Instant` 기반 sleep, `tokio-cron-scheduler` 0.15.1 | R-05 |
| 50 | 미국 지수는 장중 토스 미국 ETF 시세, 금리·지수 일별 값은 미 재무부와 FRED(`MacroSeries` 포트) | 토스 API에 미국 지수가 없다 | 지수 없이 운영 | I-18, R-11 |
| 51 | 화면의 현금은 매수가능금액. 거래내역으로 재구성한 현금 원장은 대조용 | 예수금 API가 없다 | 현금 원장을 정본으로 | I-05, R-01, R-11 |

## 8. 프런트엔드

| # | 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|---|
| 52 | TanStack Query 5.107.1 + TanStack Router 1.170.46(메모리 또는 해시 기록). `queryFn`은 `Promise<Result>`를 돌려준다 | 던지기 금지 규칙 아래에서 예제가 통과했다 | SWR, React Router 8.5.1 | R-08 |
| 53 | 경계 스키마 Zod 4.6.7, `safeParse`만 | 실측 | Valibot 1.5.0, ArkType 2.2.9 | R-08 |
| 54 | Result는 직접 정의한 판별 유니온 | 무시된 Result가 일반 린트 조합으로 막힌다 | neverthrow 8.2.0, effect 4.1.4 | R-08 |
| 55 | TS 금액 타입은 비공개 symbol 키를 가진 불투명 객체. `shared/lib/money.ts`에서만 만든다 | 문자열 브랜드는 `parseFloat`와 `+`를 통과시킨다 | 문자열 브랜드 | I-15, R-08 |
| 56 | 타입 검사 TypeScript 7.0.2, 린트용 TypeScript 6 API 함께 설치 | typescript-eslint가 TS 7을 지원하지 않는다 | TS 6만, TS 7만 | I-16, R-08 |
| 57 | 컴포넌트 기반 Base UI 1.9.0 | Combobox와 IME 조합 처리가 있다 | Radix, React Aria Components 1.22.1 | R-14 |
| 58 | 차트 Recharts 3.10.1 + Lightweight Charts 5.3.0(캔들만) | SVG는 CSS 변수가 바로 먹는다. 캔들은 전용 라이브러리 | ECharts 6.1.0 하나 | R-14 |
| 59 | 토큰은 Tailwind 4.3.5 `@theme` + `--*: initial`, `xtask gen-tokens`로 생성 | 토큰 밖 클래스가 아예 생성되지 않는다 | CSS Modules + stylelint, Panda CSS, vanilla-extract, Style Dictionary 5.8.0 | R-14 |
| 61 | 화면은 Tauri + React를 유지하고, 창 장식은 macOS 기본, 확인 창은 AppKit 기본 컨트롤, 설정은 별도 macOS 창으로 한다 | 사용자 결정(2026-11-12). 화면 쪽 강제 도구(린트, 디자인 시스템, 넘침 검사)를 유지하면서 네이티브 느낌을 얻는다 | 전체를 SwiftUI로 다시 만들기(화면 하네스와 디자인 시스템을 버려야 한다) | 사용자 결정 |
| 60 | 채팅 마크다운 react-markdown 10.1.0, 원시 HTML 끔, 이미지는 링크 글자로, 링크는 `open_external` | 이미지 렌더링만으로 데이터가 나가는 공격이 있다 | Streamdown 2.7.1 | R-14, R-16 |
| 61 | 숫자는 십진수 문자열을 `Intl.NumberFormat`에 그대로 넘기고 빼기는 U+2212 | 부동소수점 없이 정확히 형식화된다(실험) | `Number`로 바꿔 형식화 | R-14 |

## 9. 테스트 구조

| # | 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|---|
| 62 | 테스트 계층 L0~L9(정적, 단위·속성, 계약, 코어 시나리오, 경로 고정, UI, 시각 회귀, E2E 연기, 변이, 사람) | 계층마다 고정하는 성질이 다르다 | - | R-15 |
| 63 | UI 테스트는 tauri-specta가 생성한 `mockCommands`·`MockCommandHandlers` | 가짜 IPC가 생성된 명령 타입과 어긋나지 않는다 | `mockIPC` 직접 사용 | R-15 |
| 64 | E2E는 별도 `e2e-app`에서만 WebdriverIO `@wdio/tauri-service` 1.5.1 + `tauri-plugin-wdio-webdriver` 1.5.1, 밤 macOS | `tauri-driver`는 macOS를 지원하지 않는다. 자동화 플러그인이 제품에 들어가면 안 된다 | `tauri-driver` 2.1.0, crabnebula, `tauri-plugin-playwright` 0.4.1 | R-15 |
| 65 | AI CLI는 실행 파일 경로를 주입받아 `testkit`의 가짜 CLI(녹화 재생, 대본 MCP 클라이언트)로 바꾼다 | 실제 CLI 없이 재현할 수 있다 | 실제 CLI 호출 | R-15 |
| 66 | 시각 회귀 기준 환경은 고정 태그 Playwright 컨테이너(linux/arm64) | 같은 컨테이너는 바이트까지 같았고, 호스트 macOS와는 글꼴부터 달랐다 | 호스트 macOS 기준 | R-15 |

## 10. 남은 미확인과 실측

이 결정들 중 실측으로 확인해야 하는 것은 [open-issues.md](../open-issues.md) 4.3의 ID로 따라간다: 서명한 앱의 키체인과 확인 창(S-02), 샌드박스 안 빌드(S-03), 격리 옵션과 구독 인증(S-04), 한글 입력과 숫자 형식(S-05), 추정 적중률(S-06), 수집 실행의 웹 읽기 옵션(S-07), App Nap(S-10), stdin EOF(S-11), Tauri CLI와 루트 `target/`(S-12), `generate_context!()`와 목 런타임(S-13). 결정 9의 전제(두 CLI가 stdio 서버에 환경 변수를 넘기고 Unix 소켓 접속을 막지 않음)가 바뀌면 결정 9를 다시 본다(R-04 3장).
