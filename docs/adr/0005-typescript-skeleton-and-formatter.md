# ADR 0005: TypeScript 뼈대, 형식 도구, `vite/client` 타입 (실측 S-18)

- 상태: 제안
- 날짜: 2026-11-12
- 결정한 사람: 메인 에이전트(사용자 확인 대기)
- 관련 작업: H-21, H-06
- 반영한 문서: 없음 (harness.md 4.3의 "형식 도구 미정"과 open-issues S-18은 이 ADR이 답한다. 문서 본문 반영은 사용자 확인 뒤)

## 맥락

H-21은 pnpm 작업 공간, Node 고정, TS 7과 TS 6 API의 동시 설치, architecture.md 5장의 `apps/desktop/src` 폴더를 만든다. H-06은 harness.md 4.1의 tsconfig를 켜고 위반 예제로 확인한다. 두 문서가 정하지 않은 것이 셋 있었다.

1. 형식 도구. harness.md 4.3은 "Prettier와 Biome 형식 기능을 H-06에서 ADR로 정한다"고 적었다.
2. 문서에 고정값이 없는 도구 버전: pnpm, Vite.
3. 실측 S-18: 실제 앱에서 `vite/client` 타입을 tsconfig `types`에 넣어야 하는지. R-08 7.1은 미검증으로 남겼다.

## 실측

환경: macOS 26 (Apple Silicon), Node 24.22.0, pnpm 12.18.0, `tsc` 7.0.2, `tsc6` 6.0.3(`@typescript/typescript6` 6.0.2), Vite 8.3.8, React 19.3.1.

| 항목 | 명령 | 결과 |
|---|---|---|
| 앱 타입 검사 | `tsc -p apps/desktop`, `tsc6 -p apps/desktop` | 둘 다 오류 없음 |
| CSS 가져오기와 `import.meta.env`, `types: []` | 임시 사본에 `import "./app.css"`와 `import.meta.env.MODE`를 넣고 두 컴파일러로 검사 | 둘 다 TS2882(부수 효과 가져오기 해석 실패), TS2339(`ImportMeta`에 `env` 없음) |
| 같은 코드, `types: ["vite/client"]` | 같은 사본 | 둘 다 오류 없음 (`skipLibCheck: false`에서도 Vite 선언 통과) |
| CSS를 TS 대신 `index.html`의 `<link rel="stylesheet">`로 | `vite build`, `tsc` | Vite가 CSS를 묶어 `dist/assets/*.css`로 냈고 `types: []`로 `tsc` 통과 |
| Vite가 JSX를 처리하는가 | 플러그인 없이 `vite build` | tsconfig의 `jsx: react-jsx`를 따라 자동 런타임으로 빌드. `@vitejs/plugin-react` 불필요 |
| Tauri 빌드 | `dist` 삭제 뒤 `pnpm exec tauri build --no-bundle` (`apps/desktop`) | `beforeBuildCommand`가 `dist`를 다시 만들고 `<저장소>/target/release/src-tauri` 생성, 35.7초 |
| 형식 도구 결과 차이 | 같은 파일 43개(TS, TSX, MJS, JSON, HTML)에 Prettier 3.9.10과 Biome 2.6.6(린트·assist 끔, 줄 폭 100)을 적용 | TS·MJS·JSON 결과가 같았다. 차이는 HTML 빈 요소 표기(`<meta ... />`와 `<meta ...>`) 하나 |
| 형식 도구 속도 | 파일 하나 `--check` 3회 | Prettier 0.07~0.09초, Biome 0.04~0.05초 |
| 설치 크기와 의존 | `pnpm add -D` 뒤 `node_modules/.pnpm` | Prettier: 패키지 1개, 의존 0개, 9.6MB. Biome: 본체 1MB + 플랫폼별 실행 파일 패키지(darwin-arm64 81MB). 잠금 파일에 플랫폼 패키지 10개가 선택 의존으로 들어간다 |
| pnpm 최소 공개 기간 | Biome 2.6.7(공개 하루 미만)을 `pnpm install` | pnpm 12가 `pnpm-workspace.yaml`에 `minimumReleaseAgeExclude` 11줄을 스스로 추가했다 |
| `tsc --showConfig` | 두 컴파일러 | TS 7은 `noErrorTruncation`을 출력하지 않는다(설정은 받아들인다). TS 6은 출력한다 |
| tsc 옵션 끄기 | 4.1의 옵션을 하나씩 반대로 바꾸고 `harness/tsc-violations/check.mjs` | 31개 모두 실패. 위반 예제가 직접 잡지 못하는 7개(`strict`, `isolatedModules`, `noErrorTruncation`, `libReplacement`, `types`, `skipLibCheck`, `allowJs`)는 설정 값 비교가 실패시킨다 |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| 형식 도구는 Prettier 3.9.10, 기본 설정(설정 파일 없음). `just check`가 `prettier --check`로 `apps/desktop`, `harness`, 루트 `package.json`, `pnpm-workspace.yaml`을 본다 | 이 저장소 파일에서 결과가 사실상 같고 속도 차이는 파일당 0.05초 미만이다. Prettier는 의존이 0개인 패키지 하나라 TS 의존성 허용 목록(H-10)에 한 줄이다. Biome은 린트 기능을 끄는 설정을 따로 지켜야 하고, 린트는 ESLint로 정했다(harness 4.3) | Biome 형식 기능(빠르지만 플랫폼 실행 파일 패키지가 잠금 파일에 10개 들어가고, 린트·assist를 끄는 설정이 보호 대상이 된다) | 실측 |
| Prettier 3.10.0이 아니라 3.9.10 | 3.10.0은 공개 하루 미만이라 pnpm 12의 최소 공개 기간에 걸린다. 예외 목록을 늘리지 않는다 | 3.10.0 + `minimumReleaseAgeExclude` | 실측 |
| pnpm 12.18.0을 루트 `packageManager`로 고정하고 `.npmrc`에 `manage-package-manager-versions=true` | 이 PC의 pnpm 9.12.3이 이 설정으로 고정 버전으로 바꿔 실행한다. 공개 기간 검사와 의존성 설치 스크립트 기본 차단이 있는 현재 판이다 | 전역 pnpm 갱신(사용자 환경을 바꾼다), corepack | 실측 |
| Node는 `.node-version`(24.22.0)과 루트 `engines.node`로 고정하고, `just check`의 첫 단계 `check-node`가 `node --version`과 비교해 다르면 실패 | harness 1.3의 24.22.0. pnpm의 엔진 검사는 설치 때만 돈다 | `engine-strict`만 쓰기 | harness 1.3 |
| Vite 8.3.8, 플러그인 없음. Vite 설정 파일도 두지 않는다(기본값: 진입 `index.html`, 산출 `dist`) | 빈 앱의 빌드에 필요한 것이 없다. 파일이 적을수록 보호 대상이 적다 | `@vitejs/plugin-react` | 실측 |
| `frontendDist`는 `../dist`, `beforeBuildCommand`는 `pnpm exec vite build`. `just check`는 cargo 단계 전에 `vite build`를 돌린다 | `generate_context!`가 컴파일 때 `frontendDist`를 읽는다(ADR 0004). 깨끗한 사본에서도 cargo 단계가 컴파일된다 | 자리용 폴더 유지 | ADR 0004 |
| `types: []`를 유지한다 (S-18 답: 지금은 넣지 않아도 된다) | 지금 앱은 CSS 가져오기와 `import.meta.env`를 쓰지 않는다. 앞으로 CSS는 `index.html`의 `<link>`로 넣으면 `vite/client` 없이 된다 | 지금 `vite/client` 추가(`*.svg` 등의 모듈 선언이 `string`을 돌려줘 원시 타입 규칙과 부딪힌다) | 실측 |
| tsc 위반 예제는 옵션마다 한 파일, 기대 코드는 `harness/tsc-violations/expected.json`. 실행기는 앱 tsconfig의 옵션 값도 목록과 비교한다 | 행동으로 드러나지 않는 옵션도 끄면 실패해야 한다 | 행동 예제만 | harness 6.8 |

## 결과

- `just check`는 `check-node` → `pnpm install --frozen-lockfile` → Prettier → `tsc` → `vite build` → tsc 위반 예제 → 하네스 테스트(`node --test`) → cargo 단계 순서로 돈다. 처음 실행에는 pnpm 12 내려받기에 네트워크가 필요하다.
- `src/features/`에는 아직 기능이 없어 빈 폴더를 git에 남기려고 `.gitkeep`을 두었다. 첫 기능이 생기면 지운다. `shared/*`에는 `export {};`만 있는 `index.ts`를 두었다.
- 디자인 시스템 작업이 CSS를 TS에서 가져오거나 `import.meta.env`를 쓰게 되면 S-18을 다시 본다. 그때 `types`를 바꾸면 harness 4.1 변경이다.
- `lint-file`에 TS 파일 형식 검사를 붙이는 일은 하지 않았다(ESLint를 붙이는 H-07 이후).
- 바뀌면 다시 볼 조건: Prettier가 TS 7 문법을 따라가지 못할 때, TS 의존성 허용 목록에서 Biome 쪽이 더 단순해질 때, pnpm의 `packageManager` 처리 방식이 바뀔 때.
