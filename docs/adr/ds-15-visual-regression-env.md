# DS-15 시각 회귀 기준 환경

- 상태: 초안

## 결정

harness 6.6에는 비교 엔진이 미정이다. R-15의 재현 실험을 이어 Chromium을 선택한다. WKWebView 적합성과 WebKit 재현성을 증명하는 선택은 아니다. 비교는 허용 오차 없는 PNG 바이트 동등성이다. 파일 누락과 빈 파일도 실패한다. 화면 비교 자체는 제품 패키지에 들어가지 않는다. 아래 기본 글꼴 수정은 앱과 Storybook에 함께 적용한다.

이미지는 mcr.microsoft.com/playwright:v1.65.0-noble@sha256:9beb546099e11f52d62e505537914870e01a67e0e6b00ae41da4775324e73dbe, linux/arm64다. 실제 실행은 고정 참조에서 inspect한 이미지 ID로 고정하며 실행별 결과는 target/DS-15/image.json에 남긴다. 이번 실측의 실제 ID와 RepoDigest는 아래 검증 절에도 보존한다. measurement.json의 settings.image에도 고정 태그·digest가 남는다. 익명 임시 Docker 설정으로 pull하고 종료 시 설정을 지운다. Docker 자격 증명 파일을 읽지 않는다. DOCKER_HOST가 없으면 표준 Docker·Colima·Docker Desktop 소켓의 존재만 확인해 연결한다. 익명 설정이 기존 Docker context 선택도 가리는 경우를 실제로 재현했고 기본 명령이 별도 환경 설정 없이 동작하도록 했다. 컨테이너는 네트워크 없음, 루트 파일 시스템 읽기 전용이며 저장소도 읽기 전용이다. 결과와 기준 화면 디렉터리만 쓰기를 허용한다. Storybook은 호스트에서 정적 빌드하고 컨테이너 안의 루프백 HTTP 서버만 사용한다.

just visual-measure는 독립 브라우저 context 두 개에서 각각 찍어 바이트를 비교한다. just visual-update는 같은 검증 뒤 PNG와 measurement.json을 함께 생성한다. 현재 8개 파일 목록·환경·행 수·두 해시·바이트 수를 검사하고, 삭제된 이야기의 기존 linux-arm64 기준 파일을 정리한다. 알 수 없는 파일과 호스트 기준 파일은 지우지 않고 실패한다. compare와 등록 검사도 저장된 원자료와 전체 파일 목록의 일치를 요구한다. just visual-check는 두 촬영 결과와 기존 기준까지 비교한다. 기준 화면의 사용자 서명 커밋 요구는 기존 서명 정책을 따른다. 이 작업의 최초 기준 생성은 작업 지시문의 생성물 승인 범위다. 호스트에서 run.mjs를 직접 실행하면 운영체제·아키텍처·컨테이너·이미지 검증에 실패한다.

범위는 DS-07 실제 index에 등록된 Button·Card·Money·TextField의 Light0·Dark0, 8개 화면이다. 960×640, 배율 1, ko-KR, Asia/Seoul, 2026-01-02T03:04:05.000Z, 난수 0.3141592653589793을 고정한다. 애니메이션과 transition을 없애고 caret을 숨긴다. 글꼴은 촬영기에 주입하지 않는다. 앱과 Storybook이 공유하는 theme.css의 base 계층에서 html의 기본 글꼴을 기존 --font-sans 토큰으로 정한다. themeSource 생성 정책에도 같은 규칙을 넣어 손 편집이나 누락을 계속 거부한다. 페이지 실행 오류, 이야기 누락, 웹 글꼴 로딩 실패를 거부한다. 실제 렌더 텍스트의 CDP 글꼴 목록에서 시스템 글꼴 사용도 거부하며, 촬영 직전의 시계와 난수 값까지 확인한다. 빈 페이지에서 시계를 epoch로 설치한 뒤 목표 시각에 멈추고 이야기를 연다. 설치 직후 같은 시각으로 pauseAt하면 실제 시간 경과 때문에 과거로 이동 오류가 나는 경합을 통합 촬영에서 재현했기 때문이다. 테스트도 설치와 정지 사이 1ms 경과를 모사한다. IBM Plex Sans KR 및 IBM Plex Mono는 기존 design/fonts의 OFL 웹 글꼴을 Storybook이 묶는다. 외부 요청은 브라우저 라우팅과 컨테이너 네트워크 양쪽에서 막는다.

macOS 기준 파일은 Node 등록 검사와 xtask source-check에서 거부한다. 검사 고정 자료에는 대소문자 확장자와 다른 디렉터리를 포함한다. 기존 테스트 단언은 바꾸지 않는다. check-ts에 별도 Node 검사 단계를 등록한다. 새 visual 명령은 justfile의 정확한 기대 목록에도 등록한다.

## 검증

시작 frozen 설치와 lefthook 설치 성공. 프로젝트 신뢰와 세션 훅 신뢰·현재 세션 적용 여부는 확인할 수 없다. 지시문의 예외에 따라 계속하고 Git 훅을 사용한다. 설치된 Colima를 사용자 권한으로 시작했다. 시스템 설정 변경·데몬 설치는 하지 않았다.

이번 통합 실측에서 inspect한 식별 정보다. 임시 target 자료를 지워도 이 값은 ADR에 남는다.

```json
{
  "tag": "mcr.microsoft.com/playwright:v1.65.0-noble@sha256:9beb546099e11f52d62e505537914870e01a67e0e6b00ae41da4775324e73dbe",
  "id": "sha256:9beb546099e11f52d62e505537914870e01a67e0e6b00ae41da4775324e73dbe",
  "digests": [
    "mcr.microsoft.com/playwright@sha256:9beb546099e11f52d62e505537914870e01a67e0e6b00ae41da4775324e73dbe"
  ]
}
```

동일 이미지의 Chromium에서 독립 context 두 개로 찍은 8쌍 모두 바이트가 일치했다. 다음은 실제 SHA-256과 바이트 수이며 원자료는 harness/visual/baselines/measurement.json에 있다.

| 컴포넌트 | 테마 | 바이트 | 두 촬영의 SHA-256 (동일) |
|---|---|---|---|
| Button | light | 5498 | 267bdcda3bf78111797749786f9054cfa000b9988b9f23a90d5163e603f42188 |
| Button | dark | 5454 | 4c3926bdd7ffdf7d05caab0ef5c59ff402e8350d585e9cc7da03050e69ed9810 |
| Card | light | 20559 | 30ebd21cd6259b7cc6840752e98abf2f43ed44464aaf3a0714becc4250a3a50b |
| Card | dark | 21037 | 932c69b9a22da0af1ecc195800947dae1689fabb2320cc77eb253948e0f5c999 |
| Money | light | 6489 | 28e5f4db79c024f93c85844c447fab6b96c41cb56491f3622933753ab18eadf4 |
| Money | dark | 6540 | 42645f4f694ff7e25edd1422962ae02080d03be5d77cdf492f0aebc2fd3f8101 |
| TextField | light | 5201 | dbc23bca9e6205d149a0a1a3c6317595633027f0b210f3fd5e884bb0c2fa7920 |
| TextField | dark | 5260 | 71489712506bf539c408d7d5093bf5e8e71b0aebf0902dfa378c3f9fe8d5e84f |

실제 등록 검사 CLI를 임시 Git 저장소에서 실행해 위반 고정 파일을 거부하는 것을 검증한다. 브라우저 시작 실패를 주입한 경계 회귀는 HTTP 서버까지 닫혀 프로세스가 종료되는지 확인한다. Node 회귀는 촬영·서버·CI·실측 기록을 포함해 22개다. 새 체크아웃에서도 임시 디렉터리의 부모를 준비한다. Rust 단위 회귀 2개와 실제 walker를 거치는 작업 공간 겹침 고정 자료 3개로 Darwin 파일을 검증한다. 최초 전체 검사에서 발견한 목록 복사본 의존, Knip 자동 탐색, 새 Rust 린트 위반은 기존 테스트와 검사 규칙을 유지하며 수정했다.

## 보호 파일과 의존성

package.json, pnpm-lock.yaml, harness/ts-deps/allowlist.json은 문서가 허용한 Playwright 개발 의존성의 정확한 버전 등록이다. @playwright/test 1.65.0 및 playwright 1.65.0, playwright-core 1.65.0은 Apache-2.0이다. 런타임 의존성은 없다. justfile과 그 기대 JSON은 세 visual 명령 추가에 필요하다. Codex 설정·sandbox fragment·Claude 해시는 기존 생성기로 갱신한다. 검사 규칙 삭제나 허용 범위 확대는 없다.

Knip은 Playwright 패키지 추가로 활성화되는 자동 테스트 탐색을 끈다. 기존 명시적 tests/*.test.mjs 진입점을 계속 사용하며, 같은 설정을 위반 자료의 정확한 기대값에도 등록한다. 기존 테스트 단언은 바꾸지 않는다.

## 인계

DS-12: 전체 공개 목록의 기준 화면 잠금. pauseAt가 타이머와 rAF를 멈추므로 타이머로 크기를 측정하거나 열리는 이야기의 마운트 완료 조건을 범위 확대 전에 확인한다. 현재 8개 화면은 실제 촬영으로 확인했다. DS-20: 부하 자료와 넘침 실측. DS-11: 본문 폭 구간별 기준 추가. DS-19: 정적 넘침 규칙. 실제 WKWebView와 WebKit 비교, 전체 970 이야기 기준 확대는 이번 검증 범위 밖이다. CI Linux arm64 작업은 기존 CI 명령 경계에 맞는 node harness/ci/visual.mjs를 호출한다. 이 진입점은 frozen 의존성 설치가 성공한 뒤 just visual-check를 실행하며 양쪽 실패 코드를 전달한다. 도구와 취약점 DB 준비 뒤에 배치한다. check-full에도 같은 비교를 등록한다. GitHub 실행 자체는 푸시 금지 범위이므로 NOT_RUN이며, 같은 명령의 로컬 컨테이너 비교와 워크플로 정책·누락 회귀를 검증한다.

## 손 변형

초기 22개 정책 변형은 2차 비판 뒤 현재 정규식과 테스트를 사용하는 임시 복사본에서 다시 실행했다. 모두 문법 검사 뒤 검출됐다. 아래 darwin 행은 이 재실행의 정규식이다.

| 변형 | 바꾼 것 | 잡은 검사 | 결과 |
|---|---|---|---|
| image-tag | `v1.65.0-noble` → `v1.64.1-noble` | ds15-visual.test.mjs | 검출 |
| engine | `engine: "chromium"` → `engine: "webkit"` | ds15-visual.test.mjs | 검출 |
| width | `width: 960` → `width: 961` | ds15-visual.test.mjs | 검출 |
| height | `height: 640` → `height: 641` | ds15-visual.test.mjs | 검출 |
| scale | `scale: 1` → `scale: 2` | ds15-visual.test.mjs | 검출 |
| locale | `locale: "ko-KR"` → `locale: "en-US"` | ds15-visual.test.mjs | 검출 |
| timezone | `timezone: "Asia/Seoul"` → `timezone: "UTC"` | ds15-visual.test.mjs | 검출 |
| clock | `2026-01-02T03:04:05.000Z` → `2026-01-02T03:04:06.000Z` | ds15-visual.test.mjs | 검출 |
| random | `0.3141592653589793` → `0.5` | ds15-visual.test.mjs | 검출 |
| animation | `animations: "disabled"` → `animations: "allow"` | ds15-visual.test.mjs | 검출 |
| caret | `caret: "hide"` → `caret: "initial"` | ds15-visual.test.mjs | 검출 |
| component | `"TextField"` → `"Text"` | ds15-visual.test.mjs | 검출 |
| theme | `["light", "dark"]` → `["light"]` | ds15-visual.test.mjs | 검출 |
| darwin | `/-darwin(?:[-.][^/]*)?\.(png|jpe?g|webp)$/i` → `/-windows(?:[-.][^/]*)?\.(png|jpe?g|webp)$/i` | ds15-visual.test.mjs | 검출 |
| platform | `platform: "linux"` → `platform: "darwin"` | ds15-visual.test.mjs | 검출 |
| arch | `arch: "arm64"` → `arch: "x64"` | ds15-visual.test.mjs | 검출 |
| container | `container: "present"` → `container: "absent"` | ds15-visual.test.mjs | 검출 |
| image-check | `image: settings.image` → `image: "wrong"` | ds15-visual.test.mjs | 검출 |
| first-missing | `first === undefined` → `first !== undefined` | ds15-visual.test.mjs | 검출 |
| second-missing | `second === undefined` → `second !== undefined` | ds15-visual.test.mjs | 검출 |
| empty | `first.length === 0` → `first.length < 0` | ds15-visual.test.mjs | 검출 |
| equal | `!first.equals(second)` → `first.equals(second)` | ds15-visual.test.mjs | 검출 |

추가 정리 코드 제거 실험은 집계에서 제외했다. server.close만 지워도 Node 최상위 모듈 예외가 HTTP 서버와 함께 프로세스를 종료하므로, 시작 실패 시 종료된다는 외부 계약에 대해 동등했다. 이 변형을 검출했다고 보고하지 않는다.

## 정상 촬영 경로 손 변형 (1차 리뷰 수정)

초기 정책 모듈 22개와 별도로, 실제 촬영 함수의 입력 고정·글꼴 판정·컨텍스트 정리 경계를 임시 복사본에서 28개 변형했다. 모두 Node 문법 검사를 통과한 뒤 정상 촬영 회귀에 검출됐다. 2차 비판 뒤 현재 소스와 보완된 테스트로 대조군 통과를 확인하고 28개를 다시 실행해 모두 검출했다. 통합 시계 경합 수정 뒤 아래 clock-origin 1개도 더해 현재 촬영 경로 29개를 다시 검증했다.

| 변형 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| random-hook | `Math.random = () => random;` → `Math.random = () => 0.7;` | ds15-capture.test.mjs | 검출 |
| caret-option | `caret: settings.caret,` → `` | ds15-capture.test.mjs | 검출 |
| animation-option | `animations: settings.animations,` → `` | ds15-capture.test.mjs | 검출 |
| motion-context | `reducedMotion: "reduce",` → `` | ds15-capture.test.mjs | 검출 |
| scale | `deviceScaleFactor: settings.scale` → `deviceScaleFactor: 2` | ds15-capture.test.mjs | 검출 |
| locale | `locale: settings.locale` → `locale: "en-US"` | ds15-capture.test.mjs | 검출 |
| timezone | `timezoneId: settings.timezone` → `timezoneId: "UTC"` | ds15-capture.test.mjs | 검출 |
| color | `colorScheme: theme` → `colorScheme: "light"` | ds15-capture.test.mjs | 검출 |
| serviceworkers | `serviceWorkers: "block"` → `serviceWorkers: "allow"` | ds15-capture.test.mjs | 검출 |
| route-origin | `new URL(route.request().url()).origin === origin` → `new URL(route.request().url()).origin !== origin` | ds15-capture.test.mjs | 검출 |
| clock-install | `await page.clock.install({ time: new Date(0) });` → `` | ds15-capture.test.mjs | 검출 |
| clock-pause | `await page.clock.pauseAt(new Date(settings.time));` → `` | ds15-capture.test.mjs | 검출 |
| time-check | `inputs.time !== Date.parse(settings.time)` → `inputs.time === Date.parse(settings.time)` | ds15-capture.test.mjs | 검출 |
| random-check | `inputs.random !== settings.random` → `inputs.random === settings.random` | ds15-capture.test.mjs | 검출 |
| font-condition | `used.some((font) => font.glyphCount > 0 && !font.isCustomFont)` → `false` | ds15-capture.test.mjs | 검출 |
| font-glyph | `font.glyphCount > 0` → `font.glyphCount >= 0` | ds15-capture.test.mjs | 검출 |
| font-origin | `!font.isCustomFont` → `font.isCustomFont` | ds15-capture.test.mjs | 검출 |
| mono-required | `!fonts.includes("IBM Plex Mono")` → `false` | ds15-capture.test.mjs | 검출 |
| page-errors | `errors.length !== 0` → `false` | ds15-capture.test.mjs | 검출 |
| selector-theme | `data-catalog-theme="${theme}"` → `data-catalog-theme="light"` | ds15-capture.test.mjs | 검출 |
| selector-component | `data-catalog-component="${component}"` → `data-catalog-component="Button"` | ds15-capture.test.mjs | 검출 |
| cdp-detach | `await session.detach();` → `` | ds15-capture.test.mjs | 검출 |
| cdp-nodes | `for (const nodeId of nodeIds)` → `for (const nodeId of nodeIds.slice(0, 1))` | ds15-capture.test.mjs | 검출 |
| context-close | `await context.close();` → `` | ds15-capture.test.mjs | 검출 |
| style-animation | `animation: none` → `animation: initial` | ds15-capture.test.mjs | 검출 |
| style-transition | `transition: none` → `transition: initial` | ds15-capture.test.mjs | 검출 |
| style-caret | `caret-color: transparent` → `caret-color: auto` | ds15-capture.test.mjs | 검출 |
| clock-origin | `await page.clock.install({ time: new Date(0) });` → `await page.clock.install({ time: new Date(settings.time) });` | DS15 normal capture applies inputs and checks actual fonts before its screenshot | 검출 |
| return-bytes | `return bytes;` → `return undefined;` | ds15-capture.test.mjs | 검출 |

## 승인된 기존 기대 목록 갱신

사전 승인 (c): apps/desktop/tests/ds-generation.test.mjs의 CSS entry is confined to DS and Vite bundles the six protected fonts with OFL에서 고정된 CSS 선언 목록에 html의 기본 font-family: var(--font-sans) 규칙 1개를 추가한다. 전: imports/source 다음 utilities. 후: imports/source 다음 base 글꼴 규칙, 그 뒤 동일 utilities. 전체 바이트 동등성 단언과 모든 기존 선언·글꼴·OFL 단언은 보존한다. 제품의 실제 기본 글꼴을 묶인 웹 글꼴로 정해야 하는 DS-15 완료 기준에 기계적으로 필요한 추가이며, test-diff가 이 한 항목만 지적하면 나머지 검사를 확인하고 그 커밋에만 승인 예외를 사용한다.

## 2차 비판 뒤 보완

Sans 누락·글꼴 load 호출 삭제·촬영 직전 난수 검증 삭제는 각각 독립 회귀를 추가한 뒤 임시 복사본에서 다시 실행해 3개 모두 검출했다. 난수 불일치 자료는 시계를 정상 값으로 유지하므로 시계 검사로 잘못 검출하지 않는다.

| 변형 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| sans-required | Sans 필수 포함 조건 삭제 | DS15 normal capture rejects a missing bundled family or page error | 검출 |
| font-loads | 두 fonts.load 호출 삭제 | DS15 normal capture applies inputs and checks actual fonts before its screenshot | 검출 |
| random-validation | 촬영 직전 난수 일치 조건 삭제 | DS15 normal capture rejects a random source that did not freeze | 검출 |
| selector-equivalent | CDP 선택자를 바깥 요소 하나로 축소 | 기존 회귀 통과. 독립 Chromium 실험에서 부모도 자식 시스템 글꼴 검출 | 동등, 검출 집계 제외 |

기준 자료 무결성 회귀는 8개 이미지에 서로 다른 바이트·해시·크기를 사용한다. 누락·중복 행, 환경 변경, 첫째·둘째 해시 변경, 크기 변경, 이미지 뒤바꿈, 오래된 PNG, 손상 JSON, 알 수 없는 파일을 검증한다. 실제 update로 원자료를 재생성한 뒤 compare가 8쌍 일치를 다시 확인했다. 이미지 바이트는 이전 기준과 동일하다. GitHub arm64 러너와 로컬 Colima 사이 바이트 동일성은 실제 CI 실행 전이므로 NOT_RUN이다.

## 실측 자료 무결성 손 변형

현재 코드의 임시 복사본에서 각 변형의 문법 검사 뒤 ds15-records.test.mjs를 실행했다. 아래 10개는 테스트의 실제 단언 실패로 검출됐으며 모듈 로딩 실패를 집계하지 않았다.

| 변형 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| settings | `!isDeepStrictEqual(record.settings, settings)` → `false` | ds15-records.test.mjs | 검출 |
| first-hash | `rows[0].first !== hash` → `false` | ds15-records.test.mjs | 검출 |
| second-hash | `rows[0].second !== hash` → `false` | ds15-records.test.mjs | 검출 |
| byte-count | `rows[0].bytes !== bytes.length` → `false` | ds15-records.test.mjs | 검출 |
| unique-row | `rows.length !== 1` → `rows.length === 1` | ds15-records.test.mjs | 검출 |
| digest | `createHash("sha256")` → `createHash("sha512")` | ds15-records.test.mjs | 검출 |
| foreign | `if (foreign.length || unmanaged.length)` → `if (false)` | ds15-records.test.mjs | 검출 |
| prune | `rmSync(join(folder, name));` → `;` | ds15-records.test.mjs | 검출 |
| metadata-write | `writeFileSync(join(folder, "measurement.json"), text);` → `;` | ds15-records.test.mjs | 검출 |
| image-write | `writeFileSync(join(folder, name), bytes);` → `;` | ds15-records.test.mjs | 검출 |

통합 재검증에서 발견한 시계 초기화 경합은 촬영기의 오류를 재시도하거나 무시하지 않고 초기 시각을 epoch로 분리해 고쳤다. 원래 코드에서 1ms 경과 회귀가 실패하는 red를 확인했다. 목표 시각과 고정 조건은 바꾸지 않는다.

## 3차 비판 뒤 회귀 보완

3차 비판은 first-parent diff에서 병합 수정이 보이지 않는다는 주장과 커밋 메시지 중복 주장을 실제 Git 실행으로 기각했다. 이력을 다시 쓰지 않는다. 수정 경계는 78a53cdd → d8edb0cb이며, git log --first-parent -p 또는 git diff d8edb0cb^ d8edb0cb로 확인할 수 있다. 이후 수정은 DS-15 신규 테스트 세 파일과 ADR·리뷰 기록에 한정된다. 촬영·등록·기록 코드와 기준 PNG는 바꾸지 않는다.

foreign은 unmanaged의 부분집합이 아니다. Button-darwin-chromium-linux-arm64.png는 소유 파일 접미사로 끝나므로 unmanaged는 허용하지만 foreign은 거부한다. 기존 표의 foreign 행은 두 거부 조건 전체를 false로 바꾼 실제 변형이다. 단독 foreign 삭제는 별도 변형으로 아래에 더하며 동등 변형으로 제외하지 않는다.

임시 복사본의 원본 대조군 22/22 통과 뒤 아래 여섯 변형이 모두 생존하는 red를 재현했다. 테스트를 보완한 뒤 같은 여섯 변형은 모두 해당 테스트 본문에서 검출됐다. 각 변형은 Node 문법 검사를 통과했고 모듈 로딩 실패를 집계하지 않았다. 기존 64개도 현재 테스트로 다시 실행해 모두 검출했다. 총 70개이며, 앞서 설명한 동등 변형 두 개는 별도 제외다.

| 변형 | 바꾼 것 | 잡은 테스트 | 결과 |
|---|---|---|---|
| checker-record | check.mjs의 storedProblems 연결 삭제 | DS15 actual registered checker rejects fixed darwin files in another repository | 생존 → 검출 |
| extra-row | measurements 행 수 검사 삭제 | DS15 measurement requires exact scope and both hashes for distinct files | 생존 → 검출 |
| extra-key | 최상위 키 집합 검사 삭제 | DS15 measurement requires exact scope and both hashes for distinct files | 생존 → 검출 |
| empty-png | bytes.length === 0 검사 삭제 | DS15 measurement requires exact scope and both hashes for distinct files | 생존 → 검출 |
| foreign-only | foreign.length 조건만 삭제하고 unmanaged 검사 보존 | DS15 comparison rejects stale extras and update refuses foreign or unmanaged files | 생존 → 검출 |
| pause-after-navigation | pauseAt을 goto 뒤로 이동 | DS15 normal capture applies inputs and checks actual fonts before its screenshot | 생존 → 검출 |

마운트 회귀는 촬영 직전 값만 보지 않고 goto 시점에 관측한 Date.now를 별도로 저장해 목표 시각 1767323045000과 비교한다. 빈 PNG 반례는 빈 바이트에 맞는 두 해시와 크기 0을 함께 넣어 다른 조건으로 잘못 검출하지 않는다. 등록 CLI 반례도 정상 자료 0 → 손상 JSON 1 → 정상 자료 0을 실제 CLI로 확인한다. 기존 단언은 모두 유지하고 반례를 추가했다.
