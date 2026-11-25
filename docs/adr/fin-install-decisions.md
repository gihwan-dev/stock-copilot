# fin-install 결정 기록

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| 첫 설치 빌드 | Developer ID Application(팀 SLVS4WF9U2)으로 서명. 프로비저닝 프로파일이 없어 ADR 0176 §3대로 배포 갈래를 `KeychainProfile::product_login()`(로그인 키체인)으로 낮추고, Entitlements.plist의 식별자·`keychain-access-groups`와 `macOS.files`의 프로파일 줄을 뺌. Hardened Runtime 유지 | 프로파일을 만들어 데이터 보호 키체인 유지(App ID `dev.stockcopilot.desktop`과 Developer ID 프로파일이 생기면 되돌린다) |
| 묶음 구성 | `tauri build --bundles app` 뒤 `Contents/MacOS/gen-bindings`를 지우고 `target/release/mcp-bridge`를 같은 폴더에 넣은 다음 브리지와 앱을 `--options runtime --timestamp`로 다시 서명 | `bundle.externalBin`(대상 삼중 이름이 붙어 `current_exe` 옆 `mcp-bridge` 규칙과 어긋남) |
| 공증 | 하지 않음. 로컬에서 만든 앱이라 격리 속성이 없어 그대로 실행됨 | App Store Connect API 키로 공증 |
| 하네스 해시 | `hashes.mjs --write`, `fragment.mjs --write`로 다시 만들어 일반 커밋으로 넣음(에이전트에 SSH 서명 키 없음) | 사람 서명 커밋 |
