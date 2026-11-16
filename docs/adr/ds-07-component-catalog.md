# DS-07 컴포넌트 목록

- 상태: 초안

## 결정

Storybook과 React Vite 연결은 architecture 5.2의 10.6.7로 정확히 고정한다. 제품 런타임 의존성은 추가하지 않는다. `.storybook`은 개발 도구 경계다. 타입 검사되는 기존 DS 이야기 배열을 소비하는 CSF 어댑터만 JavaScript 모듈로 두며, 공개 컴포넌트의 동작과 prop은 바꾸지 않는다.

구현된 레이아웃은 Card뿐이므로 첫 묶음으로 둔다. 이후 DS-03, DS-04, DS-05, DS-06, DS-18 순서로 기본·피드백·숫자·차트·콘텐츠를 배치한다. 공통 상위 묶음과 컴포넌트 순서를 Storybook storySort로 지정한다. 새 디자인과 레이아웃 컴포넌트는 만들지 않는다.

각 예제를 라이트·다크의 개별 CSF 이야기로 노출한다. 모든 상태를 한 페이지에 동시에 그리면 열린 Dialog끼리 겹치므로 이 대안은 버린다. 기존 차트의 테마 축은 목록에서 한 번만 선택하고, 선택한 CSF 테마를 차트 속성으로 전달한다. 기존 이야기의 무인자 render는 그대로 유지한다. 예제 순서는 원본 묶음 이름의 정렬과 묶음 안 순서로 고정하며, CSF는 `node harness/ds-catalog/generate.mjs` 후 Prettier로 생성한다.

`node harness/ds-catalog/check.mjs`가 실제 Storybook 빌드의 index.json을 읽는다. index에 실린 모든 export의 실제 render를 실행하여 공개 컴포넌트·예제·테마·원본 render 함수 연결을 비교한다. 원본 예제마다 index에 라이트·다크가 하나씩 있어야 한다. includeStories/excludeStories로 빠진 이야기, 추가 이야기의 잘못된 metadata, 잘못 연결된 render는 실패한다. 실제 importPath도 지정한 목록 모듈과 정확히 연결한다. decorator는 잘못된 테마를 기본값으로 숨기지 않고 거부한다. index의 dev 태그를 요구하여 실제 사이드바에서 숨긴 이야기를 거부한다. 회귀 테스트는 composeStories로 전체 project·component·story decorator를 합성한 뒤 최종 테마를 확인한다. 별도로 원본 DS 이야기와 실제 CSF 결과의 글자·요소·역할·값을 대조하여 본문을 비운 어댑터도 잡는다.

main·preview·개별 CSF는 생성기의 소유 파일로 고정한다. 생성물 일치를 등록 검사 안에서 확인하고 `generate.mjs --check`로도 확인한다. 이 목록은 현재 필요한 설정만 지원하므로 별도 manager와 head/body 삽입 설정은 거부한다. 여섯 묶음·순서·테마 이름은 생성 입력에 있다. 새 Storybook 기능이 필요하면 해당 작업에서 생성 입력과 계약을 함께 갱신한다. iframe에 실제 연결된 CSS에서 라이트·다크 배경 토큰이 모두 있고 서로 다른지도 확인한다. 이것은 CSS 포함 여부의 증거이며 픽셀 실측은 아니다.

검사는 just check-ts와 보호된 check-list에 등록한다. 이 검사는 목록과 테마 연결을 증명하며 DS-15의 픽셀 기준 화면과 DS-20의 넘침 실측을 대체하지 않는다.

## 대응

모든 공개 컴포넌트의 CSF 페이지를 이번에 추가했다. 원본 이야기에는 공개 컴포넌트 누락이 없었다. 대응 파일은 `.storybook/catalog/<공개 이름>.stories.mjs`다. 각 파일의 개별 예제에 LightN·DarkN이 있다.

| 원본 | 공개 컴포넌트 | 목록 예제 수 |
|---|---|---|
| Base | Icon, Text, Heading, Button, IconButton, Checkbox, Switch, Disclosure, SegmentedControl, Select, TextField | 393 |
| Feedback | Card, Table, Dialog, EmptyState, Skeleton | 20 |
| Status | Badge, StatusBar | 16 |
| Numbers | Money, Percent, Quantity, Probability, PriceChange, DirectionBadge | 21 |
| Charts | LineChart, AreaChart, DonutChart, Treemap | 20 (기존 테마 중복 축 제거) |
| Content | Markdown, ChatInput | 6 |
| Catalog 추가 자료 | Text, Heading, Card, Table, EmptyState, 위 차트 네 개 | 9 |

합계 공개 컴포넌트 30개, 목록 예제 485개, 실제 indexed CSF 970개다. 가상 보유 표는 긴 한국어 종목명과 987654321098원, 가상 날짜별 추이·종목별 구성은 서로 다른 이름과 값을 사용한다. 화면 문구는 messages.ts에만 추가한다. 실제 계좌·키·API는 사용하지 않는다.

실행: `pnpm -C apps/desktop exec storybook dev -p 6006`. 빌드: `pnpm -C apps/desktop exec storybook build --output-dir ../../target/DS-07/storybook`.

## 보호 파일과 기존 테스트

package.json·lockfile·TS 의존성 허용 목록은 승인된 Storybook 설치에 필요하다. justfile과 check-list 기대는 새 누락 검사 등록에 필요하다. 생성물 동일성과 CSS 포함 검사는 그 등록 단계 내부에서 함께 실행한다. 하네스 설정과 해시는 기존 생성기로 재생성한다.

normal Knip은 Storybook의 실제 진입점에서 theme.css·stories.ts를 소비하므로 중복 수동 entry 두 개를 제거한다. productionConfig는 theme.css!를 명시적으로 유지하여 ADR 0026의 CSS 운영 진입점 정책을 보존한다. 기존 production 진입점 단언은 기준 master와 동일하다.

승인된 기존 테스트 변경은 knip.test.mjs의 `the production allowlist holds exactly the documented skeleton entries, each with a reason` 테스트에서 정확한 production 유예 목록에 Catalog.stories.tsx 한 항목을 추가한 것뿐이다(72 → 73). 기존 항목과 동등성 단언을 유지한다. 보호된 유예 목록에도 새 이야기 파일 한 항목만 등록한다. 검사 끄기와 훅 생략은 없다. 프로젝트·세션 훅 신뢰는 확인 불가이며, Git 훅은 설치했다.

## 검증과 인계

실제 index·CSF·전체 decorator·대표 자료 회귀를 추가했고, Knip·타입·린트·생성물 동일성을 확인했다. 자료 파싱 실패는 TS에서 예외를 던지지 않고 테스트에서 세 날짜·종목·금액을 독립적으로 검증한다. 컴파일되는 점 누락 변형과 예제 본문을 비우는 변형이 실제 렌더 회귀에서 실패한다. 하위 Node 테스트를 실행할 때 부모의 NODE_TEST_CONTEXT를 전달하지 않아 실제로 테스트가 실행되게 한다. 독립 검사의 첫 전체 실행 중 기존 Rust 하네스 테스트 자식이 신호 9로 종료됐다. 해당 테스트의 단독 재실행과 이후 정상 커밋 훅 전체 검사는 통과했다. 원인은 확정하지 않았다.

DS-15: 고정 브라우저의 양 테마 시각 회귀. DS-20: 전용 부하 자료·넘침 실측. DS-12: 상태 테스트·기준 화면까지 통합한 공개 목록 잠금. 실제 WKWebView와 VoiceOver는 NOT_RUN이다.

## 의존성

직접 개발 의존성: storybook 10.6.7, @storybook/react-vite 10.6.7. 추가 feature 설정 없음. 아래는 기존 lockfile 대비 새 패키지(선택 플랫폼 패키지 포함)다.

| 패키지 | 버전 | 라이선스 |
|---|---|---|
| @adobe/css-tools | 4.5.0 | MIT |
| @emnapi/core | 1.11.0 | 미설치 선택 패키지 |
| @emnapi/core | 1.9.2 | 미설치 선택 패키지 |
| @emnapi/runtime | 1.11.0 | 미설치 선택 패키지 |
| @emnapi/runtime | 1.9.2 | 미설치 선택 패키지 |
| @emnapi/wasi-threads | 1.2.1 | 미설치 선택 패키지 |
| @esbuild/aix-ppc64 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/android-arm64 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/android-arm | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/android-x64 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/darwin-arm64 | 0.28.2 | MIT |
| @esbuild/darwin-x64 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/freebsd-arm64 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/freebsd-x64 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/linux-arm64 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/linux-arm | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/linux-ia32 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/linux-loong64 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/linux-mips64el | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/linux-ppc64 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/linux-riscv64 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/linux-s390x | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/linux-x64 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/netbsd-arm64 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/netbsd-x64 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/openbsd-arm64 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/openbsd-x64 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/openharmony-arm64 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/sunos-x64 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/win32-arm64 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/win32-ia32 | 0.28.2 | 미설치 선택 패키지 |
| @esbuild/win32-x64 | 0.28.2 | 미설치 선택 패키지 |
| @joshwooding/vite-plugin-react-docgen-typescript | 0.7.0 | MIT |
| @oxc-parser/binding-android-arm-eabi | 0.127.0 | 미설치 선택 패키지 |
| @oxc-parser/binding-android-arm64 | 0.127.0 | 미설치 선택 패키지 |
| @oxc-parser/binding-darwin-arm64 | 0.127.0 | MIT |
| @oxc-parser/binding-darwin-x64 | 0.127.0 | 미설치 선택 패키지 |
| @oxc-parser/binding-freebsd-x64 | 0.127.0 | 미설치 선택 패키지 |
| @oxc-parser/binding-linux-arm-gnueabihf | 0.127.0 | 미설치 선택 패키지 |
| @oxc-parser/binding-linux-arm-musleabihf | 0.127.0 | 미설치 선택 패키지 |
| @oxc-parser/binding-linux-arm64-gnu | 0.127.0 | 미설치 선택 패키지 |
| @oxc-parser/binding-linux-arm64-musl | 0.127.0 | 미설치 선택 패키지 |
| @oxc-parser/binding-linux-ppc64-gnu | 0.127.0 | 미설치 선택 패키지 |
| @oxc-parser/binding-linux-riscv64-gnu | 0.127.0 | 미설치 선택 패키지 |
| @oxc-parser/binding-linux-riscv64-musl | 0.127.0 | 미설치 선택 패키지 |
| @oxc-parser/binding-linux-s390x-gnu | 0.127.0 | 미설치 선택 패키지 |
| @oxc-parser/binding-linux-x64-gnu | 0.127.0 | 미설치 선택 패키지 |
| @oxc-parser/binding-linux-x64-musl | 0.127.0 | 미설치 선택 패키지 |
| @oxc-parser/binding-openharmony-arm64 | 0.127.0 | 미설치 선택 패키지 |
| @oxc-parser/binding-wasm32-wasi | 0.127.0 | 미설치 선택 패키지 |
| @oxc-parser/binding-win32-arm64-msvc | 0.127.0 | 미설치 선택 패키지 |
| @oxc-parser/binding-win32-ia32-msvc | 0.127.0 | 미설치 선택 패키지 |
| @oxc-parser/binding-win32-x64-msvc | 0.127.0 | 미설치 선택 패키지 |
| @oxc-project/types | 0.127.0 | MIT |
| @oxc-resolver/binding-android-arm-eabi | 11.21.2 | 미설치 선택 패키지 |
| @oxc-resolver/binding-android-arm64 | 11.21.2 | 미설치 선택 패키지 |
| @oxc-resolver/binding-darwin-arm64 | 11.21.2 | MIT |
| @oxc-resolver/binding-darwin-x64 | 11.21.2 | 미설치 선택 패키지 |
| @oxc-resolver/binding-freebsd-x64 | 11.21.2 | 미설치 선택 패키지 |
| @oxc-resolver/binding-linux-arm-gnueabihf | 11.21.2 | 미설치 선택 패키지 |
| @oxc-resolver/binding-linux-arm-musleabihf | 11.21.2 | 미설치 선택 패키지 |
| @oxc-resolver/binding-linux-arm64-gnu | 11.21.2 | 미설치 선택 패키지 |
| @oxc-resolver/binding-linux-arm64-musl | 11.21.2 | 미설치 선택 패키지 |
| @oxc-resolver/binding-linux-ppc64-gnu | 11.21.2 | 미설치 선택 패키지 |
| @oxc-resolver/binding-linux-riscv64-gnu | 11.21.2 | 미설치 선택 패키지 |
| @oxc-resolver/binding-linux-riscv64-musl | 11.21.2 | 미설치 선택 패키지 |
| @oxc-resolver/binding-linux-s390x-gnu | 11.21.2 | 미설치 선택 패키지 |
| @oxc-resolver/binding-linux-x64-gnu | 11.21.2 | 미설치 선택 패키지 |
| @oxc-resolver/binding-linux-x64-musl | 11.21.2 | 미설치 선택 패키지 |
| @oxc-resolver/binding-openharmony-arm64 | 11.21.2 | 미설치 선택 패키지 |
| @oxc-resolver/binding-wasm32-wasi | 11.21.2 | 미설치 선택 패키지 |
| @oxc-resolver/binding-win32-arm64-msvc | 11.21.2 | 미설치 선택 패키지 |
| @oxc-resolver/binding-win32-x64-msvc | 11.21.2 | 미설치 선택 패키지 |
| @rollup/pluginutils | 5.5.1 | MIT |
| @storybook/builder-vite | 10.6.7 | MIT |
| @storybook/global | 5.0.0 | MIT |
| @storybook/icons | 2.2.0 | MIT |
| @storybook/react-dom-shim | 10.6.7 | MIT |
| @storybook/react-vite | 10.6.7 | MIT |
| @storybook/react | 10.6.7 | MIT |
| @testing-library/jest-dom | 6.9.1 | MIT |
| @types/babel__core | 7.20.5 | MIT |
| @types/babel__generator | 7.27.0 | MIT |
| @types/babel__template | 7.4.4 | MIT |
| @types/babel__traverse | 7.28.0 | MIT |
| @types/doctrine | 0.0.9 | MIT |
| @types/resolve | 1.20.6 | MIT |
| @vitest/expect | 3.2.4 | MIT |
| @vitest/pretty-format | 3.2.4 | MIT |
| @vitest/spy | 3.2.4 | MIT |
| @vitest/utils | 3.2.4 | MIT |
| @webcontainer/env | 1.1.1 | MIT |
| ast-types | 0.16.3 | MIT |
| bundle-name | 4.1.1 | MIT |
| chai | 5.3.3 | MIT |
| check-error | 2.1.3 | MIT |
| css.escape | 1.5.1 | MIT |
| deep-eql | 5.0.2 | MIT |
| default-browser-id | 5.0.1 | MIT |
| default-browser | 5.5.1 | MIT |
| define-lazy-prop | 3.0.0 | MIT |
| doctrine | 3.0.0 | Apache-2.0 |
| dom-accessibility-api | 0.6.3 | MIT |
| empathic | 2.1.0 | MIT |
| esbuild | 0.28.2 | MIT |
| esprima | 4.0.1 | BSD-2-Clause |
| estree-walker | 2.0.2 | MIT |
| glob | 13.0.7 | BlueOak-1.0.0 |
| indent-string | 4.0.0 | MIT |
| is-docker | 3.0.0 | MIT |
| is-inside-container | 1.0.0 | MIT |
| is-wsl | 3.1.1 | MIT |
| jsonc-parser | 3.3.1 | MIT |
| loupe | 3.2.1 | MIT |
| min-indent | 1.0.1 | MIT |
| minipass | 7.1.3 | BlueOak-1.0.0 |
| open | 10.2.0 | MIT |
| oxc-parser | 0.127.0 | MIT |
| oxc-resolver | 11.21.2 | MIT |
| path-scurry | 2.0.2 | BlueOak-1.0.0 |
| pathval | 2.0.1 | MIT |
| react-docgen-typescript | 2.4.0 | MIT |
| react-docgen | 8.0.4 | MIT |
| recast | 0.23.21 | MIT |
| redent | 3.0.0 | MIT |
| run-applescript | 7.1.0 | MIT |
| storybook | 10.6.7 | MIT |
| strip-indent | 3.0.0 | MIT |
| strip-indent | 4.1.1 | MIT |
| tiny-invariant | 1.3.3 | MIT |
| tinyrainbow | 2.0.0 | MIT |
| tinyspy | 4.0.6 | MIT |
| ts-dedent | 2.3.0 | MIT |
| ws | 8.23.0 | MIT |
| wsl-utils | 0.1.0 | MIT |

## 손 변형

현재 검사 복사본마다 node --check 후 새 단위 회귀를 실제 실행했다. 30/30 컴파일·검출, 생존 0. 별도 통합 회귀의 열 가지 변형은 각각 JavaScript 구문 검사 또는 TypeScript 컴파일 후 등록 checker·실제 합성 렌더 검사에서 거부된다. Node 하위 테스트가 실행되지 않은 초기 경고 실행은 성공 실적으로 세지 않았다.

| 번호 | 바꾼 것 | 잡은 테스트 |
|---|---|---|
| 1 | `stories.length === 0` → `stories.length !== 0` | ✖ DS07 complete public catalog accepts both themes (1.3515ms) |
| 2 | `pages.length !== 2` → `pages.length === 2` | ✖ DS07 complete public catalog accepts both themes (3.4915ms) |
| 3 | `page.theme === theme` → `page.theme !== theme` | ✖ DS07 theme diagnostics name only the absent dark page (0.846041ms) |
| 4 | `).length !== 1` → `).length === 1` | ✖ DS07 complete public catalog accepts both themes (1.372583ms) |
| 5 | `!components.includes(story.component)` → `components.includes(story.component)` | ✖ DS07 complete public catalog accepts both themes (1.437459ms) |
| 6 | `typeof story.render !== "function"` → `typeof story.render === "function"` | ✖ DS07 complete public catalog accepts both themes (1.397542ms) |
| 7 | `story.themes?.length !== 2` → `story.themes?.length === 2` | ✖ DS07 complete public catalog accepts both themes (1.417542ms) |
| 8 | `!story.themes.includes("light")` → `story.themes.includes("light")` | ✖ DS07 complete public catalog accepts both themes (1.399542ms) |
| 9 | `!story.themes.includes("dark")` → `story.themes.includes("dark")` | ✖ DS07 complete public catalog accepts both themes (1.348125ms) |
| 10 | `!components.includes(entry.component)` → `components.includes(entry.component)` | ✖ DS07 complete public catalog accepts both themes (3.3195ms) |
| 11 | `typeof entry.render !== "function"` → `typeof entry.render === "function"` | ✖ DS07 complete public catalog accepts both themes (1.361083ms) |
| 12 | `entry.theme !== "light"` → `entry.theme === "light"` | ✖ DS07 complete public catalog accepts both themes (1.458209ms) |
| 13 | `entry.theme !== "dark"` → `entry.theme === "dark"` | ✖ DS07 complete public catalog accepts both themes (1.404708ms) |
| 14 | `entry.theme !== "light" &&` → `entry.theme !== "light" &#124;&#124;` | ✖ DS07 complete public catalog accepts both themes (1.397042ms) |
| 15 | `story.component === component` → `story.component !== component` | ✖ DS07 complete public catalog accepts both themes (3.158209ms) |
| 16 | `entry.component === component` → `entry.component !== component` | ✖ DS07 complete public catalog accepts both themes (2.887958ms) |
| 17 | `["light", "dark"]` → `["light", "other"]` | ✖ DS07 complete public catalog accepts both themes (1.379875ms) |
| 18 | `for (const component of components)` → `for (const component of [])` | ✖ DS07 rejects missing public component (0.387292ms) |
| 19 | `for (const story of samples)` → `for (const story of [])` | ✖ DS07 rejects missing light (0.3825ms) |
| 20 | `return problems;` → `return [];` | ✖ DS07 rejects missing public component (0.410709ms) |
| 21 | `const problems = [];` → `const problems = ["false positive"];` | ✖ DS07 complete public catalog accepts both themes (1.357ms) |
| 22 | `entry.title !== entry.component` → `entry.title === entry.component` | ✖ DS07 complete public catalog accepts both themes (1.543083ms) |
| 23 | `entry.observedComponent !== entry.component` → `entry.observedComponent === entry.component` | ✖ DS07 complete public catalog accepts both themes (4.296667ms) |
| 24 | `entry.observedTheme !== entry.theme` → `entry.observedTheme === entry.theme` | ✖ DS07 complete public catalog accepts both themes (1.407583ms) |
| 25 | `entry.observedSample !== entry.sample` → `entry.observedSample === entry.sample` | ✖ DS07 complete public catalog accepts both themes (2.690083ms) |
| 26 | `entry.observedRender !== story?.render` → `entry.observedRender === story?.render` | ✖ DS07 complete public catalog accepts both themes (3.1405ms) |
| 27 | `story === undefined` → `story !== undefined` | ✖ DS07 complete public catalog accepts both themes (1.354958ms) |
| 28 | `!entry.tags?.includes("dev")` → `entry.tags?.includes("dev")` | ✖ DS07 complete public catalog accepts both themes (4.7125ms) |
| 29 | `entry.tags?.includes("dev")` → `entry.tags?.includes("test")` | ✖ DS07 rejects a story hidden from the actual sidebar (0.358084ms) |
| 30 | `entry.tags?.includes("dev")` → `entry.tags?.includes("manifest")` | ✖ DS07 dev visibility alone is enough without optional test tags (0.679291ms) |
