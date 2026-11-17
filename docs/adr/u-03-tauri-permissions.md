# ADR 초안: U-03 Tauri 권한 최소화

- 상태: 초안
- 작업: U-03
- 구현: Codex `gpt-6.1-terra` (max)
- 구현 모델 결정: 2026-11-17 사용자 결정으로 이 작업은 Terra가 직접 구현한다. 구현 모델 문서 갱신은 H-50에서 진행한다. 적대적 리뷰는 Claude Opus 5.5, 독립 비판은 Codex `gpt-6.1-sol`이다.

## 맥락

U-02는 UI 이벤트와 명령을 등록했지만 WebView 이벤트 구독 권한은 남겨 두었다. WebView는 비신뢰 영역이므로 이벤트 발행, 외부 이동, 새 창과 원격 콘텐츠를 허용하지 않으면서 앱에서 만든 이벤트를 구독할 수 있어야 한다.

## 결정

| 항목 | 결정과 근거 |
|---|---|
| 명령 ACL | 기존 생성 명령 목록과 `AppManifest::commands`, `commands` 권한 묶음을 유지한다. `capability-check`가 등록 명령·목록·묶음과 빌드 스크립트를 대조한다. |
| capability | 설정에 `main` 하나를 명시한다. `commands`, `core:event:allow-listen`, `core:event:allow-unlisten`만 부여한다. 구독 해제를 지원하며 발행·다른 플러그인 권한은 주지 않는다. |
| 이벤트 ACL 증거 | 제품 설정에서 생성한 Tauri `RuntimeAuthority`로 주 창의 구독·해제 허용, 다른 창과 원격 출처 거부, 발행 거부를 확인한다. 빈 권한 ACL에서는 구독과 해제 모두 거부된다. 별도 실제 WKWebView 실행을 뜻하지는 않는다. |
| CSP | `default-src`, `script-src`, `font-src`는 앱 자체 출처, `style-src`는 앱 출처와 인라인 스타일, `img-src`는 앱 출처와 data, `connect-src`는 Tauri의 `ipc:`만 허용한다. `worker-src`, `object-src`, `frame-src`, `base-uri`, `form-action`은 none이다. 외부 네트워크 주소는 없다. |
| 프로토타입 | `freezePrototype`을 켠다. 별도 개발 CSP와 CSP 수정 비활성화를 허용하지 않는다. |
| 탐색 | macOS 앱 자체 주소 `tauri://localhost`만 허용하며 포트·사용자 정보와 다른 호스트·스킴은 거부한다. 새 창은 대상에 관계없이 거부한다. 개발용 원격 URL과 원격 프런트 배포 경로는 허용하지 않는다. |
| 창 조립 | Tauri 자동 생성을 끄고 U-02 이벤트를 등록한 뒤 설정의 창을 보안 콜백과 함께 생성한다. 초기 창은 기존처럼 숨긴 상태에서 자료 준비 뒤 표시한다. |
| 검사 | `capability-check`는 CSP·프로토타입·프런트 위치·단일 수동 창·정확한 이벤트 권한을 검사한다. 구문 검사는 제품 조립의 이벤트 등록과 보호된 창 생성 연결, 탐색·새 창 정책을 고정한다. 독립된 정상·위반 자료로 각각을 검증한다. |

보안 검사에서 창 생성 코드의 구체적인 구문을 함께 고정하므로 창 조립을 바꿀 때는 검사의 기대 구문과 회귀 사례를 함께 검토해야 한다. 이는 외부 주소를 허용하기 위한 설정이 아니다. 새 외부 패키지는 추가하지 않았다.

## 보호 파일과 기존 테스트

| 파일 | 이유 |
|---|---|
| `apps/desktop/src-tauri/tauri.conf.json`, `capabilities/main.json` | 명시적 CSP·프로토타입·수동 창·최소 이벤트 권한 |
| `harness/capability-check/allowed-permissions.toml`, `webview-apis.toml` | 지정한 구독 권한과 보안 창 생성 파일만 한정해 등록 |
| `crates/xtask/**`, `harness/capability-check/**` | 완료 기준을 강제하는 검사와 위반 자료 추가 |
| `apps/desktop/src-tauri/src/main.rs`, `src/lib.rs` | 실제 ACL 검증에 기존 생성 context 재사용, 조립 시 이벤트 등록과 보호된 창 생성 연결 |

기존 테스트의 단언을 삭제하거나 완화하지 않았다.

## 인계

| 작업 | 인계 |
|---|---|
| U-05 | UI의 실제 이벤트 구독과 제품 발행 시 수명 주기 및 구독 해제를 연결하고 검증한다. |
| O-13 | 실제 창과 제품 capability를 포함한 실행 경로 검증 및 S-13의 목 런타임 적용 가능성을 확인한다. |
| H-50 | 사용자가 결정한 구현 모델 허용 범위를 저장소의 모델 지침에 반영한다. |

## 손 변형

격리된 임시 복사본에서 기준 테스트와 각 변형의 재컴파일·실패를 확인했다. 첫 측정에서 창 개수 불일치 변형이 살아남아 위반 사례를 추가한 뒤 다시 검출했다.

| 번호 | 바꾼 것 | 검출 테스트 | 결과 |
|---|---|---|---|
| 1 | CSP 비교 반전 | `u03_security_fixture_baseline_is_accepted` | 검출 |
| 2 | devCsp 존재 반전 | `u03_security_fixture_baseline_is_accepted` | 검출 |
| 3 | CSP 수정 차단 반전 | `u03_security_fixture_baseline_is_accepted` | 검출 |
| 4 | freezePrototype 비교 반전 | `u03_security_fixture_baseline_is_accepted` | 검출 |
| 5 | 프런트 위치 비교 반전 | `u03_security_fixture_baseline_is_accepted` | 검출 |
| 6 | devUrl 존재 반전 | `u03_security_fixtures_detect_each_regression` | 검출 |
| 7 | 창 목록 판정 반전 | `u03_security_fixture_baseline_is_accepted` | 검출 |
| 8 | 창 이름 비교 반전 | `u03_requires_csp_prototype_freezing_and_event_subscription_permissions` | 검출 |
| 9 | 자동 창 생성 비교 반전 | `u03_security_fixture_baseline_is_accepted` | 검출 |
| 10 | 창 URL 부재 반전 | `u03_security_fixture_baseline_is_accepted` | 검출 |
| 11 | 창 개수 불일치 허용 | `u03_security_fixtures_detect_each_regression` | 검출 |
| 12 | 이벤트 권한 비교 반전 | `u03_security_fixture_baseline_is_accepted` | 검출 |
| 13 | listen 대신 emit 허용 | `u03_security_fixture_baseline_is_accepted` | 검출 |
| 14 | unlisten 대신 listen 중복 | `u03_security_fixture_baseline_is_accepted` | 검출 |
| 15 | worker CSP 기대값 변경 | `u03_security_fixture_baseline_is_accepted` | 검출 |
| 16 | CSP 위반 규칙 교체 | `u03_security_fixtures_detect_each_regression` | 검출 |
| 17 | setup 메서드 비교 반전 | `u03_navigation_wiring_fixtures_detect_each_regression` | 검출 |
| 18 | setup 인자 수 변경 | `u03_navigation_wiring_fixtures_detect_each_regression` | 검출 |
| 19 | build 메서드 비교 반전 | `u03_navigation_wiring_fixtures_detect_each_regression` | 검출 |
| 20 | build 인자 수 변경 | `u03_navigation_wiring_fixtures_detect_each_regression` | 검출 |
| 21 | 빌드 문맥 인자 변경 | `u03_navigation_wiring_fixtures_detect_each_regression` | 검출 |
| 22 | 중복 함수 판정 반전 | `u03_navigation_wiring_fixtures_detect_each_regression` | 검출 |
| 23 | 함수 이름 비교 반전 | `u03_navigation_wiring_fixtures_detect_each_regression` | 검출 |
| 24 | 구문 동등 비교 반전 | `u03_navigation_wiring_fixtures_detect_each_regression` | 검출 |
| 25 | 모든 보안 함수 검사 축소 | `u03_navigation_wiring_fixtures_detect_each_regression` | 검출 |
| 26 | 조립 본문 추가문 허용 | `u03_navigation_wiring_fixtures_detect_each_regression` | 검출 |
| 27 | 보안 파일 추가 항목 허용 | `u03_navigation_wiring_fixtures_detect_each_regression` | 검출 |
