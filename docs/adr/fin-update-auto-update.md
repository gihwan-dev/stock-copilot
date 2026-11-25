# ADR (번호 없음): 자동 업데이트

- 상태: 확정 (2026-11-25, 사용자 지시 "자동 업데이트 기능을 넣어 달라. 로컬의 t3code처럼")

## 배경

배포는 메인이 로컬에서 Developer ID로 서명한 `.app`을 만들어 나눠 주는 방식이다(공증 없음). 새 버전이 나와도 사용자가 직접 받아 바꿔야 했다. t3code 데스크톱은 시작 때와 주기적으로 새 버전을 확인하고, 뒤에서 받아 둔 뒤 "재시작하면 업데이트"를 안내하고 릴리스 노트를 보여 준다. 같은 흐름을 Tauri로 옮긴다.

## 결정

1. 공식 `tauri-plugin-updater =2.14.1`만 들인다(사용자 승인). 기능은 `zip`만 켜고 `rustls-tls`는 끈다. `rustls-tls`는 `ring`을 직접 켜는데, 제품은 이미 `reqwest`의 `rustls`(aws-lc) 기능을 쓰므로 작업 공간 기능 통합으로 같은 TLS 공급자를 쓴다. 재시작은 Tauri의 `AppHandle::request_restart`로 하므로 `tauri-plugin-process`와 두 플러그인의 JS 짝은 들이지 않는다.
2. 엔드포인트는 GitHub Releases의 `https://github.com/gihwan-dev/stock-copilot/releases/latest/download/latest.json` 하나다. 업데이트 묶음(`Stock.Copilot.app.tar.gz`)은 minisign 서명(`.sig`)을 `tauri.conf.json`의 공개키로 설치 직전에 검증한다. 서명이 맞지 않으면 설치하지 않는다.
3. 확인·다운로드·설치는 모두 Rust 코어가 한다. 시작 15초 뒤 한 번, 그 뒤 6시간마다 확인한다. 새 버전이 있으면 뒤에서 받아 메모리에 두고, 사용자가 "재시작"을 고르면 그때 설치 → 수명 주기 종료(데이터베이스 닫기, 자식 프로세스 정리) → 재시작한다. 받아 둔 채 앱을 끄면 다음 실행에서 다시 받는다. 실행 중에 앱 묶음을 바꾸면 이미 떠 있는 앱과 새 `mcp-bridge`가 섞일 수 있어서 설치를 재시작 직전으로 미룬다.
4. 개발 빌드(`tauri::is_dev()`, 즉 `custom-protocol` 없이 만든 빌드)와 공개키가 자리표시(`STOCK_COPILOT_UPDATER_PUBKEY_PLACEHOLDER`)이거나 비어 있는 빌드에서는 업데이트를 끈다. 상태는 "off"로 보이고 확인 버튼이 꺼진다. 소스 검사가 `cfg!`를 막으므로 `cfg!(debug_assertions)` 대신 Tauri의 `is_dev()`를 쓴다.
5. WebView에는 코어 명령 두 개만 연다: `get_update_status`(`current` | `check-now`)와 `restart_to_update`(`restart`). 두 요청은 모르는 필드와 값을 거부하며 주소·버전·파일을 받지 않는다. WebView는 업데이트 주소나 묶음을 고를 수 없다.
6. 화면: 새 버전을 받아 두면 앱 상태 띠(DS `StatusBar`, info)에 "새 버전이 준비됐어요. 재시작하면 바로 바뀌어요."와 "재시작" 버튼이 뜬다. 누르면 바텀시트(DS `Dialog` `placement="sheet"`)가 새 버전과 릴리스 노트(DS `Markdown`, 링크 없음)를 보여 주고 "지금 재시작"/"나중에"를 고르게 한다. 설정의 시스템 영역에는 "앱 업데이트" 절(현재 버전, 상태 문장, "업데이트 확인" 버튼, 준비됐으면 "재시작")을 둔다. 화면은 상태를 10분마다, 확인·다운로드 중에는 2초마다 다시 묻는다(새 UI 이벤트를 만들지 않는다).
7. 릴리스 스크립트 `scripts/release-mac.mjs`가 버전 올리기 → `tauri build --bundles app`(Developer ID) → `gen-bindings` 빼기·`mcp-bridge` 넣기 → 브리지·앱 재서명 → 재서명한 `.app`으로 `Stock.Copilot.app.tar.gz` → `tauri signer sign` → `latest.json` → `gh release create`를 한다. 업데이터 비밀키는 저장소 밖 macOS 키체인(서비스 `dev.stockcopilot.updater-key`, 계정 `private-key`, 암호는 계정 `password`, 없으면 빈 암호)에만 두고, 서명할 때 자식 프로세스 환경 변수로만 넘긴다. `init-key`는 키를 만들어 키체인에 넣고(`security -i` 표준 입력, 프로세스 인자에 비밀키 없음) 공개키를 `tauri.conf.json` 자리표시에 써 넣는다. 이미 키가 있으면 덮어쓰지 않는다.

## SEC 항목과의 관계

- SEC-10(WebView에 `http` 권한 없음, CSP의 외부 연결 금지): 그대로다. 네트워크는 Rust 플러그인 안에서만 열고 CSP `connect-src ipc:`를 바꾸지 않는다. capability에 `updater:*` 권한을 주지 않는다. 플러그인이 등록하는 JS 명령은 권한이 없어 WebView에서 부를 수 없다. capability 검사의 WebView API 목록에서 `plugin`을 `src/main.rs` 한 곳에만 열었다(다른 파일은 계속 실패, 고정 사례 `plugin-outside-assembly`).
- SEC-5·8(주문 경로): 업데이트 코드는 주문 포트에 닿지 않는다. 재시작은 사용자가 바텀시트에서 고를 때만 하고, 수명 주기 종료를 거친다.
- SEC-1·11(비밀): 업데이터 비밀키는 저장소·로그·CLI 인자에 두지 않는다. 앱에는 공개키만 들어간다.
- 공급망: 업데이트 묶음은 공개키 서명 검증을 통과해야 설치된다. 서명 키를 잃으면 새 키로 바꾼 버전을 사용자가 한 번 직접 설치해야 한다(ADR 0176과 같은 성격의 키 분실 위험).
- 구조 검사: `arch-check`에 `tauri-plugin-updater` 경유 HTTP·TLS 전이 예외(reqwest를 src-tauri가 직접 쓰지 않음)와, 플러그인의 `zip`이 켜는 `zeroize` derive 기능의 전체 대상 통합으로 보이는 절차 매크로 경로 예외를 추가했다. `cargo-deny`에는 새 전이 crate(`tauri-plugin`, `minisign-verify`, `osakit`, `objc2-osa-kit`, `tar`, `filetime`, `xattr`, `tempfile`, `rustix`, `errno`)와 빌드 스크립트(`tauri-plugin-updater`, `filetime`, `rustix`)를 추가했다.

## 버린 대안

- WebView에서 `@tauri-apps/plugin-updater`로 확인·설치: `updater:default` 권한이 WebView에 열려 SEC-10의 "WebView에 네트워크 경로 없음"을 깨고, JS가 설치 시점을 정하게 된다.
- 받자마자 설치: 실행 중 앱 묶음이 바뀌어 브리지와 앱 버전이 섞일 수 있다.
- 공증한 `.dmg`로 배포: 지금 배포 방식(공증 없음)과 다르고 사용자 절차가 늘어난다.
- `tauri-plugin-process`의 `relaunch`: 코어의 `request_restart`로 충분하다.

## 메인이 할 일

1. `node scripts/release-mac.mjs init-key` → 바뀐 `tauri.conf.json`(공개키) 커밋·푸시.
2. `node scripts/release-mac.mjs release <버전> [--notes <파일>]` → 끝에 안내하는 `git push origin HEAD v<버전>`.
3. 이 버전부터 설치한 앱이 다음 버전을 스스로 받는다. 공개키가 들어가기 전 빌드(자리표시)는 업데이트가 꺼져 있으므로 첫 업데이트 지원 버전은 한 번 직접 설치한다.
