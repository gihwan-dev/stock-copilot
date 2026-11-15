# DS-07 컴포넌트 목록

- 상태: 초안

## 결정

Storybook과 React Vite 연결은 설계 5.2의 10.6.7을 정확히 고정한다. 새 런타임 의존성은 없다. `.storybook`은 개발 도구 경계이고 제품 src와 함께 배포되지 않는다. 기존의 타입 검사되는 이야기 배열을 소비하는 CSF 어댑터만 JavaScript 모듈로 둔다. 공개 컴포넌트 동작과 prop은 변경하지 않는다.

architecture 5.1에서 구현된 레이아웃은 Card뿐이므로 첫 묶음으로 둔다. 이후 DS-03, DS-04, DS-05, DS-06, DS-18 순서다. 새 레이아웃 컴포넌트를 만들지 않는다. 컴포넌트마다 라이트·다크 두 페이지에 모든 기존 상태를 표시한다. 차트 render는 선택 테마를 받아 명시적 theme prop에 전달한다. 기존 호출은 기본값을 유지한다.

`node harness/ds-catalog/check.mjs`가 실제 공개 런타임 내보내기와 실제 이야기 배열·CSF 모듈을 번들로 읽고 누락을 거부한다. `just check-ts`에 등록하여 전체·프런트 빠른 검사에서 실행한다. 위반 고정 자료는 새 테스트의 cases 배열이다. 시각 회귀와 넘침은 DS-15·DS-20·DS-12 인계다.

실행: `pnpm -C apps/desktop exec storybook dev -p 6006`, 빌드: `pnpm -C apps/desktop exec storybook build --output-dir ../../target/DS-07/storybook`.

## 대응

모든 공개 컴포넌트에 CSF 페이지를 이번에 추가했다. 원본 이야기의 누락은 없었다. 대응은 `.storybook/catalog/<공개 이름>.stories.mjs`의 Light·Dark다.

| 원본 | 공개 컴포넌트 |
|---|---|
| Base | Icon, Text, Heading, Button, IconButton, Checkbox, Switch, Disclosure, SegmentedControl, Select, TextField |
| Feedback | Card, Table, Dialog, EmptyState, Skeleton |
| Status | Badge, StatusBar |
| Numbers | Money, Percent, Quantity, Probability, PriceChange, DirectionBadge |
| Charts | LineChart, AreaChart, DonutChart, Treemap |
| Content | Markdown, ChatInput |

큰 원화 금액·빈 차트·빈 표·긴 한국어 마크다운·긴 입력의 기존 가상 자료를 그대로 쓴다. 실제 계좌나 API는 사용하지 않는다.

## 보호 파일과 테스트

`package.json`, lockfile, TS 의존성 허용 목록은 승인된 Storybook 설치에 필요하다. justfile은 새 누락 검사를 등록한다. 생성 설정과 해시는 저장소 쓰기 모드로 재생성한다. 승인된 고정 목록 테스트 변경 1건: knip.test.mjs의 production 진입점 정확 기대를 [theme.css!]에서 []로 바꾼다. Storybook 직접 소비로 중복 진입점을 제거한 결과이며 동등성 단언은 유지한다. Storybook이 직접 읽는 theme.css·stories.ts의 중복 knip 진입점 두 개를 제거하고 검사의 정확한 config 기대도 동기화한다. 새 검사 단계를 check-list의 steps·justfile 기대에 추가한다. 기존 단언은 유지한다. 검사 끄기 0건이다. 프로젝트 및 세션 훅 신뢰는 확인 불가, Git 훅은 설치 완료다.

## 인계

DS-15: 고정 브라우저 양 테마 시각 회귀. DS-20: 부하 자료·넘침 실측. DS-12: 상태 테스트·기준 화면까지 통합한 공개 목록 잠금. WKWebView와 VoiceOver 실측은 이번 결과로 증명하지 않는다.

## 검증

Storybook 정적 빌드 통과. 새 회귀 19/19. 변경분 변이는 프런트 빌드 후 종료 0, 실행 대상 Rust 변이 없음. 손 변형 21/21 컴파일·검출. 정상 훅 첫 통합 검사는 신규 검사 등록 목록과 knip의 중복 진입점 때문에 실패했고, 등록 목록 및 중복 설정을 보완한다.

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

mktemp 복사본마다 node --check 후 새 테스트를 실제 실행했다. 21/21 컴파일·검출, 생존 0.

| 번호 | 바꾼 것 | 잡은 테스트 |
|---|---|---|
| 1 | `stories.length === 0` → `stories.length !== 0` | ✖ DS07 complete public catalog accepts both themes (1.36075ms) |
| 2 | `pages.length !== 2` → `pages.length === 2` | ✖ DS07 complete public catalog accepts both themes (3.554667ms) |
| 3 | `page.theme === theme` → `page.theme !== theme` | ✖ DS07 theme diagnostics name only the absent dark page (1.375708ms) |
| 4 | `).length !== 1` → `).length === 1` | ✖ DS07 complete public catalog accepts both themes (2.980959ms) |
| 5 | `!components.includes(story.component)` → `components.includes(story.component)` | ✖ DS07 complete public catalog accepts both themes (2.420375ms) |
| 6 | `typeof story.render !== "function"` → `typeof story.render === "function"` | ✖ DS07 complete public catalog accepts both themes (1.345416ms) |
| 7 | `story.themes?.length !== 2` → `story.themes?.length === 2` | ✖ DS07 complete public catalog accepts both themes (1.423ms) |
| 8 | `!story.themes.includes("light")` → `story.themes.includes("light")` | ✖ DS07 complete public catalog accepts both themes (1.334208ms) |
| 9 | `!story.themes.includes("dark")` → `story.themes.includes("dark")` | ✖ DS07 complete public catalog accepts both themes (9.02175ms) |
| 10 | `!components.includes(entry.component)` → `components.includes(entry.component)` | ✖ DS07 complete public catalog accepts both themes (2.746875ms) |
| 11 | `typeof entry.render !== "function"` → `typeof entry.render === "function"` | ✖ DS07 complete public catalog accepts both themes (2.722042ms) |
| 12 | `entry.theme !== "light"` → `entry.theme === "light"` | ✖ DS07 complete public catalog accepts both themes (2.740792ms) |
| 13 | `entry.theme !== "dark"` → `entry.theme === "dark"` | ✖ DS07 complete public catalog accepts both themes (2.491833ms) |
| 14 | `entry.theme !== "light" &&` → `entry.theme !== "light" &#124;&#124;` | ✖ DS07 complete public catalog accepts both themes (1.277958ms) |
| 15 | `story.component === component` → `story.component !== component` | ✖ DS07 complete public catalog accepts both themes (2.100292ms) |
| 16 | `entry.component === component` → `entry.component !== component` | ✖ DS07 complete public catalog accepts both themes (2.777792ms) |
| 17 | `["light", "dark"]` → `["light", "other"]` | ✖ DS07 complete public catalog accepts both themes (2.642084ms) |
| 18 | `for (const component of components)` → `for (const component of [])` | ✖ DS07 rejects missing public component (0.366833ms) |
| 19 | `for (const story of samples)` → `for (const story of [])` | ✖ DS07 rejects missing light (0.713792ms) |
| 20 | `return problems;` → `return [];` | ✖ DS07 rejects missing public component (0.381667ms) |
| 21 | `const problems = [];` → `const problems = ["false positive"];` | ✖ DS07 complete public catalog accepts both themes (1.379208ms) |
