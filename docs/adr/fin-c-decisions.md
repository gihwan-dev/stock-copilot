# fin-c 묶음 결정 기록

- 상태: 확정 (2026-11-25, 묶음 fin-c, ADR 0173 경량 절차)
- 사용자 지시: S-12는 실측 S-17 없이 문서 기준으로 구현한다. S-14의 "사용자 승인: tauri.conf.json"은 승인된 것으로 본다.

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| S-12 | `SecretStore` 구현은 `KeychainSecretStore<B: KeychainBackend>`이고 실제 키체인은 `SystemKeychain`(`security-framework` 3.7.0 `passwords` 안전 래퍼, 기능 `OSX_10_15`만)이다. 이 crate에 `unsafe`는 없다 | `security-framework-sys` 직접 호출(`unsafe` 필요) |
| S-12 | 키체인 호출은 호출 스레드에서 바로 끝내고 즉시 완료된 `Future`로 돌려준다. 데이터 보호 키체인은 사용자 상호작용 없이 끝난다 | `spawn_blocking`(이 crate에 tokio 의존 추가) |
| S-12 | 항목 하나 = 서비스 이름 하나 + `SecretKey`별 계정 이름(`toss.client-secret`, `database.encryption-key` 등) | 키마다 서비스 이름 |
| S-12 | 접근성은 `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`, 동기화 끔. 화면이 잠긴 동안에도 상주 작업이 DB를 열어야 하기 때문이다 | `WhenUnlockedThisDeviceOnly`(잠금 중 상주 작업 실패) |
| S-12 | 접근 그룹은 코드에 적지 않고 서명 entitlements의 첫 `keychain-access-groups` 항목(앱 팀 그룹)을 기본으로 쓴다. 이름을 줘야 할 때만 `AccessGroup::Named` | 코드에 팀 ID 고정(팀 ID가 아직 없음) |
| S-12 | 저장은 기존 항목 삭제 후 추가. 접근 제어 속성이 든 질의로 `SecItemUpdate`를 부르지 않는다 | `SecItemUpdate` |
| S-12 | 오류 매핑: 권한 부족(`-34018`)·인증 실패·취소 → `PermissionDenied`, 잠김(`-25308`)·기타 → `Unavailable(Unknown)`, 형식 오류 → `SecretIntegrity`. 항목 없음은 읽기 `None`, 삭제 성공. 오류에 코드·값을 싣지 않는다 | 원시 OSStatus 노출 |
| S-12 | 개발·배포 구분은 조립 루트가 `tauri::is_dev()`로 `BuildKind`를 정해 넘긴다. 개발은 로그인 키체인 + 서비스 `dev.stockcopilot.desktop.development-fake-keys`, 배포는 데이터 보호 키체인 + 서비스 `dev.stockcopilot.desktop` | `cfg!(debug_assertions)`(source-check가 `cfg!` 금지) |
| S-12 | 실제 키체인을 건드리는 테스트는 두지 않는다. source-check가 `#[ignore]`를 금지하고 에이전트는 사용자 키체인을 건드리지 않는다. 실제 왕복은 S-17(사용자) | `#[ignore]` 테스트 |
| S-12 | DB 키 생성 `generate_database_key()`는 `SecRandom`으로 32바이트를 만든다(새 패키지 없음) | `getrandom` 등 새 의존 |
| S-12 | `harness/arch-check/rules.toml`에 `secrets-keychain`의 `syn`·`proc-macro2` 경유 `security-framework` 예외 추가(작업 공간 기능 통합 경로, ADR 0173 6항의 추가형 변경) | - |
