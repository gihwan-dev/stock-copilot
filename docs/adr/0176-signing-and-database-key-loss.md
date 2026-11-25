# ADR 0176: 서명 구성과 DB 키 분실 (S-14)

- 상태: 확정 (2026-11-25, 묶음 fin-c. 사용자가 `tauri.conf.json` 변경을 승인했다)
- 관련: SEC-1, SEC-11, architecture 6.1, ADR 0002 #45, ADR 0016, S-12, S-16, S-17, U-02, U-15

## 배경

architecture 6.1은 DB 키 분실 시 복구 경로를 S-14에서 정한다고 미뤘다. Apple 개발자 멤버십(U-02)과 팀 ID, 프로비저닝 프로파일은 아직 없다. 실측 S-17(서명한 앱에서 키체인 왕복과 확인 창)은 사용자가 아직 하지 않았다. 사용자는 실측 없이 문서 기준으로 S-12와 S-14를 끝내라고 지시했다.

## 결정

### 1. 빌드 두 갈래

| 빌드 | 만드는 법 | 키체인 | 서비스 이름 | 비밀 |
|---|---|---|---|---|
| 개발 | `cargo build`, `tauri dev` (Tauri `custom-protocol` 기능 없음, `tauri::is_dev()`가 참) | 로그인 키체인 | `dev.stockcopilot.desktop.development-fake-keys` | 가짜 키만 |
| 배포 | `tauri build` (`custom-protocol` 기능 켜짐) | 데이터 보호 키체인, `AfterFirstUnlockThisDeviceOnly`, 동기화 끔 | `dev.stockcopilot.desktop` | 실제 키 |

- 갈래는 조립 루트(`src-tauri/src/main.rs`)가 `tauri::is_dev()`로 `secrets_keychain::BuildKind`를 골라 정한다. 코드에 `cfg!`를 쓰지 않는다(source-check).
- 개발 빌드는 묶음을 만들지 않으므로 entitlements가 붙지 않고, 실제 서비스 이름에 닿지 않는다.

### 2. 서명 구성

- `tauri.conf.json`의 `bundle`: `active: true`, 대상 `app`, `macOS.hardenedRuntime: true`, `macOS.entitlements: "Entitlements.plist"`, `macOS.files`에 `embedded.provisionprofile` ← `signing/embedded.provisionprofile`, 최소 macOS 13.0.
- `apps/desktop/src-tauri/Entitlements.plist`: `com.apple.application-identifier`, `com.apple.developer.team-identifier`, `keychain-access-groups` 한 항목(`<팀 ID>.dev.stockcopilot.desktop`). 팀 ID 자리는 `TEAMID_PLACEHOLDER`다.
- 서명 신원은 설정 파일에 적지 않고 빌드할 때 환경 변수 `APPLE_SIGNING_IDENTITY`로 준다. 프로비저닝 프로파일은 사용자가 `apps/desktop/src-tauri/signing/embedded.provisionprofile`에 두고 커밋하지 않는다. 파일이 없으면 `tauri build`가 실패한다(서명 없는 배포 빌드를 만들지 않는다).
- 코드는 접근 그룹을 적지 않는다. 데이터 보호 키체인은 entitlements의 첫 `keychain-access-groups` 항목을 기본 그룹으로 쓴다(`AccessGroup::EntitlementDefault`).
- 사용자가 멤버십을 얻으면 할 일: `TEAMID_PLACEHOLDER` 세 곳을 팀 ID로 바꾸고, App ID `dev.stockcopilot.desktop`과 Developer ID 프로비저닝 프로파일을 만들어 위 경로에 두고, `APPLE_SIGNING_IDENTITY`를 주고 `tauri build`.

### 3. 멤버십이 없을 때 (U-02가 "없음"으로 끝날 때)

데이터 보호 키체인은 프로비저닝이 필요하므로 로그인 키체인으로 낮춘다.

- `secrets_keychain::KeychainProfile::product_login()`(로그인 키체인 + 실제 서비스 이름)을 쓴다. 조립 루트의 배포 갈래 한 줄을 바꾸는 사용자 서명 커밋으로만 바꾼다. 실행 중 자동으로 낮추지 않는다(조용한 보안 저하를 막는다).
- `Entitlements.plist`에서 `keychain-access-groups`와 두 식별자 키를 빼고 `macOS.files`의 프로파일 줄을 뺀다. 서명은 ad-hoc(`APPLE_SIGNING_IDENTITY=-`)이다.
- 잃는 것: 다른 프로세스가 사용자 확인 창을 거쳐 항목을 읽을 수 있다(로그인 키체인 ACL). 앱을 다시 빌드하면 서명이 바뀌어 처음 읽을 때 확인 창이 한 번 뜬다.

### 4. DB 키 분실

- 키는 키체인에만 있고 `ThisDeviceOnly`라 백업·이전·동기화로 옮겨지지 않는다. 키를 내보내는 복구 문구나 별도 사본은 만들지 않는다(SEC-1, P-1).
- 시작할 때(조립 루트) 키가 없으면:
  - DB 파일이 없으면 첫 실행이다. `SecRandom` 32바이트 키를 만들어 키체인에 저장한 뒤 DB를 연다.
  - DB 파일이 있으면 키를 잃은 것이다. 새 키를 만들지 않고 DB 파일을 지우거나 덮어쓰지 않는다. 저장소 상태는 `Failed(SecretStore, SecretIntegrity)`다.
- 복구 경로: 사용자가 설정 > 고급 > 시스템의 전체 삭제(U-15)로 DB와 키체인 항목을 지우고 새로 시작한다. 토스에서 다시 받을 수 있는 것(계좌, 보유, 거래, 시세)은 다음 동기화로 돌아온다. 이 기기에만 있던 것(AI 추정, 신호, 제안, 감사 로그, 성적표, 채팅)은 잃는다. 백업도 같은 키로 암호화되므로 함께 잃는다.
- 키체인 읽기 자체가 실패하면(잠김, 권한) 키가 없는 것으로 보지 않는다. 저장소 상태는 그 오류로 `Failed`이고 키를 새로 만들지 않는다.

### 5. 감사 로그 머리값 고정 시점 (S-16에 넘김)

- 감사 기록을 SQLite에 커밋한 직후마다 마지막 순번과 해시를 키체인 항목 하나에 덮어쓴다.
- 키체인 쓰기가 실패하면 기록은 그대로 두고 다음 커밋 때 다시 쓴다. 시작할 때 DB 머리값이 키체인 머리값보다 앞이면 키체인을 따라잡게 쓰고, 뒤면 되돌리기로 탐지한다.
- HMAC 키와 머리값은 DB 키와 같은 서비스, 서로 다른 계정이다. DB 키 분실 복구(전체 삭제)는 이 둘도 지운다.

### 6. S-17 시험 빌드

- 따로 최소 앱을 두지 않는다. 위 2의 서명 구성으로 만든 제품 앱이 시험 빌드다. 첫 실행이 DB 키를 만들어 저장하고, 두 번째 실행이 그 키로 DB를 연다(키체인 왕복). 위험 설정 변경 확인 창과 주문 요약·본인 확인 창의 실측 절차는 S-17에서 사용자가 정한다.

### 7. `gen-bindings` 묶음 제외 (미룸)

- ADR 0016의 1순위(예시 대상으로 옮기기)는 보호 파일 `harness/generated-files/generated.mjs`, `harness/antigravity-hooks/adapter.mjs`와 기존 테스트 `crates/xtask/src/workspace_layout_tests.rs`(bin 목록 고정), `harness/antigravity-hooks/tests/adapter.test.mjs`를 추가형이 아닌 방식으로 바꿔야 한다. 그래서 이번에는 옮기지 않고 사용자 서명 커밋으로 넘긴다. 첫 서명 빌드 전에 해야 한다.

## 버린 대안

| 대안 | 버린 이유 |
|---|---|
| 서명용 Tauri 덮어쓰기 설정 파일(`tauri.signed.conf.json`) | capability-check가 `tauri.`로 시작하는 추가 설정 파일을 금지한다. 덮어쓰기 파일이 보안 설정까지 바꿀 수 있다 |
| 데이터 보호 키체인 실패 시 실행 중 로그인 키체인으로 자동 전환 | 사용자 모르게 보안이 낮아진다 |
| DB 키 복구 문구·내보내기 | 사용자 절차가 늘고(P-1) 키가 키체인 밖에 생긴다(SEC-1) |
| 키를 잃었을 때 새 키로 새 DB를 자동으로 만들기 | 기존 DB를 덮거나 옮겨야 하고, 키체인 일시 오류를 분실로 오판하면 자료를 잃는다 |
| 코드에 팀 ID·접근 그룹 고정 | 팀 ID가 없고, entitlements 한 곳에서 정하면 된다 |
