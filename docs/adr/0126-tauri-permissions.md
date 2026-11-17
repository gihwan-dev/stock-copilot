# ADR 0126: U-03 Tauri 권한과 WebView 탐색 제한

- 상태: 확정 (2026-11-18)

- 작업: U-03
- 구현: Codex `gpt-6.1-terra` (max)
- 구현 모델 결정: 2026-11-17 사용자 결정으로 이 작업은 Terra가 직접 구현한다. 구현 모델 문서 갱신은 H-50이 최신 master에 반영했다. 적대적 리뷰는 Claude Opus 5.5, 독립 비판은 Codex `gpt-6.1-sol`이다.

## 맥락

U-02는 UI 이벤트와 명령을 등록했지만 WebView 이벤트 구독 권한은 남겨 두었다. WebView는 비신뢰 영역이므로 이벤트 발행, 외부 이동, 새 창과 원격 콘텐츠를 허용하지 않으면서 앱에서 만든 이벤트를 구독할 수 있어야 한다.

## 결정

| 항목 | 결정과 근거 |
|---|---|
| 명령 ACL | 기존 생성 명령 목록과 `AppManifest::commands`, `commands` 권한 묶음을 유지한다. `capability-check`가 등록 명령·목록·묶음과 빌드 스크립트를 대조한다. |
| capability | 설정에 `main` 하나를 명시한다. `commands`, `core:event:allow-listen`, `core:event:allow-unlisten`만 부여한다. 구독 해제를 지원하며 발행·다른 플러그인 권한은 주지 않는다. |
| 이벤트 ACL 증거 | 제품 설정에서 생성한 Tauri `RuntimeAuthority`로 주 창의 구독·해제 허용, 다른 창과 원격 출처 거부, 발행 거부를 확인한다. 제품 capability에서 구독·해제 권한을 각각 하나만 제거하고 생성된 매니페스트로 다시 해석하면 제거한 명령만 거부되며 나머지 구독 명령은 허용된다. 빈 권한 ACL에서는 둘 다 거부된다. 별도 실제 WKWebView 실행을 뜻하지는 않는다. |
| CSP | `default-src`, `script-src`, `font-src`는 앱 자체 출처, `style-src`는 앱 출처와 인라인 스타일, `img-src`는 앱 출처와 data, `connect-src`는 Tauri의 `ipc:`만 허용한다. `worker-src`, `object-src`, `frame-src`, `base-uri`, `form-action`은 none이다. 외부 네트워크 주소는 없다. |
| 프로토타입 | `freezePrototype`을 켠다. Node에서 같은 `Object.prototype` 동결 뒤 zod 파싱과 React Markdown·Base UI 버튼의 서버 렌더를 확인했다. 실제 WKWebView 검증은 O-13으로 넘긴다. 별도 개발 CSP와 CSP 수정 비활성화를 허용하지 않는다. |
| 탐색 | macOS 앱 자체 주소 `tauri://localhost`만 허용하며 포트·사용자 정보와 다른 호스트·스킴은 거부한다. 새 창은 대상에 관계없이 거부한다. 개발용 원격 URL과 원격 프런트 배포 경로는 허용하지 않는다. |
| 창 조립 | Tauri 자동 생성을 끄고 U-02 이벤트를 등록한 뒤 설정의 창을 보안 콜백과 함께 생성한다. 초기 창은 기존처럼 숨긴 상태에서 자료 준비 뒤 표시한다. |
| 검사 | `capability-check`는 CSP·프로토타입·프런트 위치·단일 수동 창·정확한 이벤트 권한을 검사한다. 창과 WebView의 직접 생성·연관 빌더 이름을 금지하며, 구문 검사는 보안 모듈 경로, 제품 조립의 이벤트 등록과 보호된 창 생성 연결, 탐색·새 창 정책을 고정한다. 독립된 정상·위반 자료로 각각을 검증한다. |

보안 검사에서 창 생성 코드의 구체적인 구문을 함께 고정하므로 창 조립을 바꿀 때는 검사의 기대 구문과 회귀 사례를 함께 검토해야 한다. 이는 외부 주소를 허용하기 위한 설정이 아니다. 새 외부 패키지는 추가하지 않았다.

CSP와 `freezePrototype`의 자동 검사는 설정 파일의 값을 확인한다. `main.rs`에서 생성된 실행 중 context의 보안 값을 나중에 바꾸거나 보호된 조립을 건너뛰는 경우까지 검사하지는 않는다. 현재 `product_context`는 생성된 context를 그대로 반환하며, 이 경계의 변경은 `docs/harness.md` 9장 RV-6 리뷰에서 확인한다.

## 동결 호환성 재현

저장소 루트에서 고정된 Node 버전과 설치된 의존성을 사용해 아래 명령을 실행했다. 이 실험은 Node 서버 렌더 범위이며 실제 WKWebView 실행을 대신하지 않는다.

```sh
cd apps/desktop
node --input-type=module <<'NODE'
import assert from 'node:assert/strict';
Object.freeze(Object.prototype);
const { z } = await import('zod');
const React = await import('react');
const { renderToStaticMarkup } = await import('react-dom/server');
const { default: Markdown } = await import('react-markdown');
const { Button } = await import('@base-ui/react/button');
assert.deepEqual(z.object({ value: z.string() }).parse({ value: 'ok' }), { value: 'ok' });
const markdown = renderToStaticMarkup(React.createElement(Markdown, null, '**ok**'));
assert.equal(markdown, '<p><strong>ok</strong></p>');
const button = renderToStaticMarkup(React.createElement(Button, null, 'ok'));
assert.match(button, /<button[^>]*>ok<\/button>/);
console.log('freezePrototype Node smoke: zod parse, React Markdown and Base UI button server render passed');
NODE
```

Node `v24.22.0`에서 종료 코드 0과 `freezePrototype Node smoke: zod parse, React Markdown and Base UI button server render passed`를 확인했다.

## 보호 파일과 기존 테스트

| 파일 | 이유 |
|---|---|
| `apps/desktop/src-tauri/tauri.conf.json`, `capabilities/main.json` | 명시적 CSP·프로토타입·수동 창·최소 이벤트 권한 |
| `harness/capability-check/allowed-permissions.toml`, `webview-apis.toml` | 지정한 구독 권한과 보안 창 생성 파일만 한정해 등록 |
| `crates/xtask/**`, `harness/capability-check/**` | 완료 기준을 강제하는 검사와 위반 자료 추가 |
| `.codex/config.toml` | ADR 초안 경로를 에이전트 설정의 보호 목록에 등록 |
| `harness/claude-hooks/hashes.json` | 보호 파일 등록 변경에 따른 해시 재생성 |
| `apps/desktop/src-tauri/src/main.rs`, `src/lib.rs` | 실제 ACL 검증에 기존 생성 context 재사용, 조립 시 이벤트 등록과 보호된 창 생성 연결 |

기존 테스트의 단언을 삭제하거나 완화하지 않았다.

## 인계

| 작업 | 인계 |
|---|---|
| U-05 | UI의 실제 이벤트 구독과 제품 발행 시 수명 주기 및 구독 해제를 연결하고 검증한다. 생성된 바인딩의 `emit`·`emitTo`는 노출되지만 현재 WebView ACL에서는 거부된다. |
| O-13 | 실제 WKWebView의 탐색·새 창 콜백과 `freezePrototype`의 프런트 호환성, 제품 capability 실행 경로 및 S-13 목 런타임 적용 가능성을 확인한다. |
| H-50 | 사용자가 결정한 구현 모델 허용 범위를 모델 지침에 반영했다. |

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
| 28 | 보안 모듈 선언 내용 검증 생략 | `u03_navigation_wiring_fixtures_detect_each_regression` | 검출 |
| 29 | 보안 모듈 중복 허용 | `u03_navigation_wiring_fixtures_detect_each_regression` | 검출 |
| 30 | 보안 모듈 이름 비교 반전 | `u03_navigation_wiring_fixtures_detect_each_regression` | 검출 |
| 31 | 보안 모듈 파일 경로 변경 | `u03_navigation_wiring_fixtures_detect_each_regression` | 검출 |

아래는 컴파일 대상 코드 변형과 별도로 실행한 설정 목록 제거 실험이다. 각 항목을 하나씩 지우면 위반 자료 검사가 실패했다.

| 설정에서 뺀 항목 | 검출 테스트 | 결과 |
|---|---|---|
| `WebviewWindow` | `webview_api_cases_report_exactly_their_findings` | 검출 |
| `Webview` | `webview_api_cases_report_exactly_their_findings` | 검출 |
| `Window` | `webview_api_cases_report_exactly_their_findings` | 검출 |
