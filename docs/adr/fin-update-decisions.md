# fin-update 묶음 설계 결정 기록

구조 결정은 [fin-update-auto-update.md](fin-update-auto-update.md)에 있다.

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| 의존성 | `tauri-plugin-updater =2.14.1`(zip만, rustls-tls 끔)만 추가했다. `tauri-plugin-process`와 JS 짝은 쓰지 않는다. 재시작은 `AppHandle::request_restart` | 승인된 네 패키지 모두 추가(WebView 권한이 열리고 패키지가 늘어남) |
| 확인 주기 | 시작 15초 뒤 한 번, 그 뒤 6시간마다. 확인·받기 중이거나 이미 준비됐으면 새 확인을 건너뛴다. 실패하면 다음 주기나 "업데이트 확인"에서 다시 한다 | 시작 즉시 확인(첫 화면 준비와 겹침) |
| 설치 시점 | 받은 묶음은 메모리에 두고 "지금 재시작"을 누를 때 설치 → 수명 주기 종료 → 재시작. 앱을 그냥 끄면 다음 실행에서 다시 받는다 | 받자마자 설치(실행 중 앱 묶음 교체) |
| 개발 빌드 | `tauri::is_dev()`이거나 공개키가 자리표시·빈 값이면 끈다(상태 "off") | `cfg!(debug_assertions)`(소스 검사가 막음) |
| IPC | 명령 두 개 `get_update_status{query: current|check-now}`, `restart_to_update{confirm: restart}`. 상태는 외부 태그 열거(`"off"`, `{"ready":{version,notes}}` 등). `check-now`는 확인을 뒤에서 시작하고 바로 현재 상태를 돌려준다 | 내부 태그(`kind`) 열거(specta 파생이 `unreachable!`을 만들어 Clippy가 막음) |
| 화면 갱신 | 새 UI 이벤트 없이 상태 질의를 10분마다, 확인·받기 중에는 2초마다 다시 한다 | `UiEvent`에 새 변형 추가(이벤트 스키마·고정 목록 변경이 커짐) |
| 상태 띠 | 앱 셸 상태 자리에 업데이트 띠를 점검 띠 위에 둔다. 준비됐을 때만 보인다 | 토스트(DS에 없음) |
| 설정 위치 | 실제 조립(`CoreApp`)은 설정 클라이언트가 없어 시스템 절이 숨겨지므로, "앱 업데이트" 절을 항상 보이는 시스템 묶음(위험 편집 아래, 앱 정보 위)에 DS `Section`으로 둔다. 기존 고정 테스트가 설정 화면의 `h3`를 세 개로 묶어 `Card`(h3 제목)를 쓰지 않았다 | `Card`로 둠(기존 테스트 변경) |
| 릴리스 노트 | `latest.json`의 `notes`를 바텀시트에서 DS `Markdown`으로 보이고 링크는 모두 글자로만 둔다. 노트는 서명되지 않은 메타데이터라 신뢰하지 않는 글로 다룬다 | 링크 허용 |
| 자산 이름 | GitHub가 업로드 이름의 공백을 `.`으로 바꾸므로 묶음 파일을 처음부터 `Stock.Copilot.app.tar.gz`로 만든다. 묶음 안의 앱 이름은 `Stock Copilot.app` 그대로다 | `Stock Copilot.app.tar.gz`로 만들고 업로드 때 이름 바꾸기 |
| 키 보관 | 비밀키는 키체인 `dev.stockcopilot.updater-key`/`private-key`, 암호는 같은 서비스 `password`(없으면 빈 암호). `init-key`는 `security -i` 표준 입력으로 저장해 프로세스 인자에 비밀키가 보이지 않게 한다 | `security add-generic-password -w <값>`(인자에 노출) |
| 스크립트 위치 | 저장소 루트 `scripts/release-mac.mjs`(지시 위치), 시험은 `check-lite`가 도는 `apps/desktop/tests/fin-update-release.test.mjs` | `apps/desktop/scripts/` |
| 버전 | 루트 `Cargo.toml`의 `[workspace.package] version`만 올리고 `cargo update --workspace --offline`으로 잠금 파일을 맞춘다. `tauri.conf.json`에는 버전을 두지 않아 Cargo 버전을 따른다 | `tauri.conf.json`에 버전을 따로 둠(두 곳을 맞춰야 함) |
| 하네스 | `arch-check` 예외 4건, capability WebView API 목록의 `plugin`을 `src/main.rs`에만 허용(고정 사례 1건 추가), `cargo-deny` 허용 11건·빌드 스크립트 3건. 모두 추가형 | - |
