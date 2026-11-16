# DS-15 시각 회귀 기준 환경

- 상태: 초안

## 결정

harness 6.6에는 비교 엔진이 미정이다. R-15의 재현 실험을 이어 Chromium을 선택한다. WKWebView 적합성과 WebKit 재현성을 증명하는 선택은 아니다. 비교는 허용 오차 없는 PNG 바이트 동등성이다. 파일 누락과 빈 파일도 실패한다. 제품 동작과 패키지에는 영향이 없다.

이미지는 mcr.microsoft.com/playwright:v1.65.0-noble@sha256:9beb546099e11f52d62e505537914870e01a67e0e6b00ae41da4775324e73dbe, linux/arm64다. 실제 실행은 태그에서 inspect한 이미지 ID로 고정하며 digest와 ID를 target/DS-15/image.json에 남긴다. 익명 임시 Docker 설정으로 pull하고 종료 시 설정을 지운다. Docker 자격 증명 파일을 읽지 않는다. DOCKER_HOST가 없으면 표준 Docker·Colima·Docker Desktop 소켓의 존재만 확인해 연결한다. 익명 설정이 기존 Docker context 선택도 가리는 경우를 실제로 재현했고 기본 명령이 별도 환경 설정 없이 동작하도록 했다. 컨테이너는 네트워크 없음, 루트 파일 시스템 읽기 전용이며 저장소도 읽기 전용이다. 결과와 기준 화면 디렉터리만 쓰기를 허용한다. Storybook은 호스트에서 정적 빌드하고 컨테이너 안의 루프백 HTTP 서버만 사용한다.

just visual-measure는 독립 브라우저 context 두 개에서 각각 찍어 바이트를 비교한다. just visual-update는 같은 검증 뒤 기준 파일을 쓴다. just visual-check는 두 촬영 결과와 기존 기준까지 비교한다. 기준 화면의 사용자 서명 커밋 요구는 기존 서명 정책을 따른다. 이 작업의 최초 기준 생성은 작업 지시문의 생성물 승인 범위다. 호스트에서 run.mjs를 직접 실행하면 운영체제·아키텍처·컨테이너·이미지 검증에 실패한다.

범위는 DS-07 실제 index에 등록된 Button·Card·Money·TextField의 Light0·Dark0, 8개 화면이다. 960×640, 배율 1, ko-KR, Asia/Seoul, 2026-01-02T03:04:05.000Z, 난수 0.3141592653589793을 고정한다. 애니메이션과 transition을 없애고 caret을 숨긴다. 페이지 실행 오류, 이야기 누락, 웹 글꼴 로딩 실패를 거부한다. 실제 렌더 텍스트의 CDP 글꼴 목록에서 시스템 글꼴 사용도 거부하며, 촬영 직전의 시계와 난수 값까지 확인한다. IBM Plex Sans KR 및 IBM Plex Mono는 기존 design/fonts의 OFL 웹 글꼴을 Storybook이 묶는다. 외부 요청은 브라우저 라우팅과 컨테이너 네트워크 양쪽에서 막는다.

macOS 기준 파일은 Node 등록 검사와 xtask source-check에서 거부한다. 검사 고정 자료에는 대소문자 확장자와 다른 디렉터리를 포함한다. 기존 테스트 단언은 바꾸지 않는다. check-ts에 별도 Node 검사 단계를 등록한다. 새 visual 명령은 justfile의 정확한 기대 목록에도 등록한다.

## 검증

시작 frozen 설치와 lefthook 설치 성공. 프로젝트 신뢰와 세션 훅 신뢰·현재 세션 적용 여부는 확인할 수 없다. 지시문의 예외에 따라 계속하고 Git 훅을 사용한다. 설치된 Colima를 사용자 권한으로 시작했다. 시스템 설정 변경·데몬 설치는 하지 않았다.

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

실제 등록 검사 CLI를 임시 Git 저장소에서 실행해 위반 고정 파일을 거부하는 것을 검증한다. 브라우저 시작 실패를 주입한 경계 회귀는 HTTP 서버까지 닫혀 프로세스가 종료되는지 확인한다. 새 Node 회귀 7개와 Rust 회귀 2개가 통과했다. 최초 전체 검사에서 발견한 목록 복사본 의존, Knip 자동 탐색, 새 Rust 린트 위반은 기존 테스트와 검사 규칙을 유지하며 수정했다.

## 보호 파일과 의존성

package.json, pnpm-lock.yaml, harness/ts-deps/allowlist.json은 문서가 허용한 Playwright 개발 의존성의 정확한 버전 등록이다. @playwright/test 1.65.0 및 playwright 1.65.0, playwright-core 1.65.0은 Apache-2.0이다. 런타임 의존성은 없다. justfile과 그 기대 JSON은 세 visual 명령 추가에 필요하다. Codex 설정·sandbox fragment·Claude 해시는 기존 생성기로 갱신한다. 검사 규칙 삭제나 허용 범위 확대는 없다.

Knip은 Playwright 패키지 추가로 활성화되는 자동 테스트 탐색을 끈다. 기존 명시적 tests/*.test.mjs 진입점을 계속 사용하며, 같은 설정을 위반 자료의 정확한 기대값에도 등록한다. 기존 테스트 단언은 바꾸지 않는다.

## 인계

DS-12: 전체 공개 목록의 기준 화면 잠금. DS-20: 부하 자료와 넘침 실측. DS-11: 본문 폭 구간별 기준 추가. DS-19: 정적 넘침 규칙. 실제 WKWebView와 WebKit 비교, 전체 970 이야기 기준 확대는 이번 검증 범위 밖이다. CI의 컨테이너 실측 등록은 검증된 동일 명령을 소비하도록 후속 통합에서 평가한다.

## 손 변형

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
| darwin | `/-darwin\.(png|jpe?g|webp)$/i` → `/-windows\.(png|jpe?g|webp)$/i` | ds15-visual.test.mjs | 검출 |
| platform | `platform: "linux"` → `platform: "darwin"` | ds15-visual.test.mjs | 검출 |
| arch | `arch: "arm64"` → `arch: "x64"` | ds15-visual.test.mjs | 검출 |
| container | `container: "present"` → `container: "absent"` | ds15-visual.test.mjs | 검출 |
| image-check | `image: settings.image` → `image: "wrong"` | ds15-visual.test.mjs | 검출 |
| first-missing | `first === undefined` → `first !== undefined` | ds15-visual.test.mjs | 검출 |
| second-missing | `second === undefined` → `second !== undefined` | ds15-visual.test.mjs | 검출 |
| empty | `first.length === 0` → `first.length < 0` | ds15-visual.test.mjs | 검출 |
| equal | `!first.equals(second)` → `first.equals(second)` | ds15-visual.test.mjs | 검출 |

추가 정리 코드 제거 실험은 집계에서 제외했다. server.close만 지워도 Node 최상위 모듈 예외가 HTTP 서버와 함께 프로세스를 종료하므로, 시작 실패 시 종료된다는 외부 계약에 대해 동등했다. 이 변형을 검출했다고 보고하지 않는다.
