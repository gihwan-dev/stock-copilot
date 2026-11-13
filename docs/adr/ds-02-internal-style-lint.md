# ADR 초안: DS-02 내부 스타일 검사

- 상태: 초안
- 범위: shared/ds 내부. 화면 금지는 DS-09, 공개 props는 DS-10, 컴포넌트는 DS-03, 넘침의 파일별 예외는 DS-19다.

| R-14 # | 금지 | 강제 도구 | 위반 | 통과 |
| --- | --- | --- | --- | --- |
| 1 | 생성 테마에 없는 클래스 | better-tailwindcss/no-unknown-classes, theme.css 절대 경로 고정 검사 | bg-red-500 | bg-primary |
| 2 | 임의 값·속성·변형, 변수 축약, 분수·백분율·불투명도·px | no-restricted-classes 닫힌 클래스 집합 | p-(--x), p-[13px], w-1/2 | p-space-4 |
| 3 | 숫자 직접 입력과 토큰 namespace 혼용 | 같은 닫힌 집합 | border-2, p-sidebar-width, w-space-4 | border-hairline, w-sidebar-width |
| 4 | dark 변형 | 같은 닫힌 집합, 허용 변형 목록 | dark:bg-primary | hover:bg-primary |
| 5 | 가격 파일 허용 목록 밖 가격색 | 별도 no-restricted-classes 설정. 초기 파일 목록은 비움 | text-up | text-ink |
| 6 | 동적 클래스와 검증 불가능한 전달 | 로컬 AST 규칙, no-concatenated-classes | className={value}, cn(value) | className="flex", cn('flex', active && 'block') |
| 7 | 인라인 style | 별도 로컬 AST 규칙. 초기 허용 파일 없음 | style={value} | className="flex" |
| 8 | 색·px 문자열 | 별도 로컬 AST 규칙, Literal·TemplateElement | '#fff', '13px' | 'primary' |
| 15 | CSS 색·길이 직접 입력, 미등록 var | stylelint color-no-hex, declaration-strict-value, 로컬 CSS 검사 | color: red; padding: var(--bad) | color: var(--color-primary) |

닫힌 집합은 코드용 토큰 입력에서 의미별 접두어를 조합한다. spacing은 간격에만, size/layout 길이는 크기에만 쓴다. 숫자 직접 입력·음수 접두·rounded-full은 금지하고 radius 토큰을 쓴다. flex-1과 w-full은 디자인의 flex:1, width:100%에 대응하는 닫힌 키워드 예외다. absolute, overflow 계열, h-screen은 DS-19가 파일별로 제한해야 하므로 이 작업에서 승인 목록을 만들지 않고 집합 밖에 둔다. 아직 필요한 컴포넌트가 없으며 허용 목록 추가는 후속 작업의 검토 대상이다.

className은 문자열 리터럴 또는 직접 cn 호출만 받는다. cn 인자는 문자열, 조건식의 문자열 분기, 논리 AND의 오른쪽 문자열만 받는다. 템플릿, 변수, 객체, 별칭 함수, clsx/cva 및 spread는 전달 형태 자체를 거부한다. 따라서 값 추적의 빈틈을 허용하지 않는다. cn 구현·컴포넌트는 만들지 않는다.

Tailwind는 무한한 임의 값을 생성하므로 전체 가능한 클래스 집합을 유한 스냅샷이라고 부르지 않는다. 검사 입력은 토큰별 의미 조합, 닫힌 키워드, 독립적인 DS-13 인계 반례 집합이다. 실제 컴파일에서 생성되는 선택자 집합을 고정한다. DS-13 utility-cases.json은 생성기의 토큰별 1개 대표 대응 검사이며 이 목록을 스냅샷 입력으로 복사하지 않는다. 스냅샷을 허용 목록으로 역참조하지 않는다.

생성 tokens.css는 DS-13 재생성 동일성 검사 대상이다. theme.css는 Tailwind 진입 문법 고정 검사 대상이다. 수기 CSS만 stylelint와 var 이름 검사를 적용한다.

## 사전 확인

기준 커밋은 `8b28a27a171d10b5a4bd22dc898ac42ca083594a`, 작업 브랜치는 `ds-02`다. pnpm frozen 설치와 lefthook 설치를 실행했다. 첫 전체 검사 실행 중 의존성 설치가 허용 목록 테스트에 영향을 주었으므로 매니페스트와 잠금 파일을 기준 상태로 복원하고 전체 검사를 다시 실행한다.

설치 실측에서 better-tailwindcss 4.11.0의 peer 범위는 ESLint 10과 Tailwind 4.3.5를 포함한다. 작업 디렉터리가 desktop인 실제 ESLint 실행에서 bg-red-500은 no-unknown-classes 오류, bg-primary는 통과했다. 저장소 루트에서 플러그인을 실행하면 Tailwind를 찾지 못해 규칙이 꺼지는 경고가 나왔다. 검사 실행 디렉터리도 고정해야 한다.

## 실측 결과와 경계

없는 CSS에서는 bg-red-500이 통과한다. reset 없는 CSS에서도 통과한다. tokens.css 단독은 reset을 포함하므로 bg-red-500을 검출한다. 따라서 세 잘못된 경로가 모두 조용히 통과한다는 가정은 채택하지 않는다. 올바른 theme.css와 cwd의 정확한 고정 검사를 별도로 두어 세 설정 모두 변경으로 실패한다.

전용 예제는 `harness/ds-style/src/shared/ds`의 52개 파일과 `cases.json` 기대 규칙 집합이다. 기존 ESLint 예제의 파일+기대 집합 구조를 따르며 DS 설정 블록을 그대로 가져온다. 타입 인식 전체 앱 설정의 DS 규칙·설정과 동일한지는 별도 테스트로 고정한다. 기존 전체 예제에는 React 통과 대조군을 하나 추가했다. 기존 spread 대조군에는 새 오류 하나만 더했다.

규칙 비활성화 검사는 각 예제의 기대 규칙마다 다른 규칙을 끈 격리 설정으로 먼저 검출을 확인하고, 해당 규칙도 끄면 진단이 없어지는 것을 확인한다. 겹치는 규칙이 있으므로 전체 설정에서 규칙 하나를 끄는 것만으로 모든 오류가 사라진다는 주장은 하지 않는다. CSS 두 플러그인 규칙도 격리 설정에서 같은 검사를 한다. CSS var 검사는 등록 목록에 없는 이름으로 실패하고, 등록된 이름은 통과한다.

생성 유틸리티 스냅샷은 실제 Tailwind 4.3.5의 4,285개 선택자다. 토큰 의미별 조합, 닫힌 키워드, 닫힌 변형과 DS-13 인계 반례가 입력이다. 무한 문법 전체를 열거한 목록이 아니다. reset 제거가 실제 선택자 집합을 바꾸는 테스트가 있다. 재생성은 `node --input-type=module`에서 `harness/ds-style/check.mjs`의 snapshot을 호출해 utilities.json에 JSON으로 기록하는 방식이며, 허용 목록은 이 출력에 의존하지 않는다. 파일은 harness 보호 범위에 있다.

알려진 경계: DOM의 classList/setAttribute 같은 직접 조작 경로를 이 작업의 클래스 표현식 분석기로 추적하지 않는다. 기존 불변·DOM·전역 규칙과 후속 DS-09/DS-10 리뷰가 함께 필요하다. cn 구현은 만들지 않았고 다른 도우미나 객체·변수 인자를 허용하지 않는다. 새 클래스나 변형은 닫힌 목록과 스냅샷을 검토해야 한다. CSS는 임의 함수·escape·주석·사용자 변수 선언·fallback을 허용하지 않는다. 공개 props, 화면 코드 금지, 실제 컴포넌트, 파일별 넘침 허용 목록은 구현하지 않았다.

기존 테스트 변경은 eslint-config.test.mjs의 끌 수 없는 규칙 배열에 여섯 규칙 이름을 추가한 것뿐이다. 기존 예제 기대값에는 ds/static-classes를 추가했다. check-list의 두 보호 목록에는 새 검사 한 줄만 등록했다. test-diff HEAD는 위반 없음이었다. 프로젝트 신뢰 및 네이티브 훅 신뢰의 런타임 적용 여부는 이 세션에서 확인할 수 없었으며 자동 종료 검사에 의존하지 않고 전체 검사를 직접 실행한다.

## 의존성

직접 개발 의존성은 better-tailwindcss 4.11.0(no-unknown-classes, no-restricted-classes, no-concatenated-classes), stylelint 18.0.0(color-no-hex), stylelint-declaration-strict-value 1.13.0(declaration-strict-value)이다. 모두 문서 지정 버전이며 MIT다. better-tailwindcss의 cwd와 entryPoint를 함께 고정한다. stylelint 플러그인의 peer 범위는 >=16 <=18이며 실제 18.0.0에서 실행했다. 아래는 잠금 파일에 새로 추가된 전이 패키지를 포함한 목록이다.

| 패키지 | 잠금 버전 | 라이선스 |
| --- | --- | --- |
| `@csstools/css-calc` | 3.4.3 | MIT |
| `@csstools/css-parser-algorithms` | 4.0.2 | MIT |
| `@csstools/css-syntax-patches-for-csstree` | 1.1.16 | MIT-0 |
| `@csstools/css-tokenizer` | 4.0.2 | MIT |
| `@csstools/media-query-list-parser` | 5.0.2 | MIT |
| `@csstools/selector-resolve-nested` | 4.0.2 | MIT-0 |
| `@csstools/selector-specificity` | 6.0.1 | MIT-0 |
| `@dual-bundle/import-meta-resolve` | 4.2.1 | MIT |
| `@eslint/css-tree` | 4.1.2 | MIT |
| `@pkgr/core` | 0.3.6 | MIT |
| `@valibot/to-json-schema` | 1.8.0 | MIT |
| `argparse` | 3.0.4 | PSF-2.0 |
| `array-buffer-byte-length` | 1.0.2 | MIT |
| `arraybuffer.prototype.slice` | 1.0.4 | MIT |
| `async-function` | 1.0.0 | MIT |
| `available-typed-arrays` | 1.0.7 | MIT |
| `call-bind-apply-helpers` | 1.0.2 | MIT |
| `call-bind` | 1.0.9 | MIT |
| `call-bound` | 1.0.4 | MIT |
| `colord` | 2.12.0 | MIT |
| `cosmiconfig` | 10.0.1 | MIT |
| `css-tree` | 3.2.1 | MIT |
| `data-view-buffer` | 1.0.2 | MIT |
| `data-view-byte-length` | 1.0.2 | MIT |
| `data-view-byte-offset` | 1.0.1 | MIT |
| `define-data-property` | 1.1.4 | MIT |
| `define-properties` | 1.2.1 | MIT |
| `dunder-proto` | 1.0.1 | MIT |
| `env-paths` | 2.2.1 | MIT |
| `es-abstract-get` | 1.0.0 | MIT |
| `es-abstract` | 1.24.2 | MIT |
| `es-define-property` | 1.0.1 | MIT |
| `es-object-atoms` | 1.1.2 | MIT |
| `es-set-tostringtag` | 2.1.0 | MIT |
| `es-to-primitive` | 1.3.4 | MIT |
| `eslint-plugin-better-tailwindcss` | 4.11.0 | MIT |
| `for-each` | 0.3.5 | MIT |
| `function.prototype.name` | 1.2.0 | MIT |
| `functions-have-names` | 1.2.3 | MIT |
| `generator-function` | 2.0.1 | MIT |
| `get-intrinsic` | 1.3.0 | MIT |
| `get-proto` | 1.0.1 | MIT |
| `get-symbol-description` | 1.1.0 | MIT |
| `globalthis` | 1.0.4 | MIT |
| `gopd` | 1.2.0 | MIT |
| `has-bigints` | 1.1.0 | MIT |
| `has-property-descriptors` | 1.0.2 | MIT |
| `has-proto` | 1.2.0 | MIT |
| `has-symbols` | 1.1.0 | MIT |
| `has-tostringtag` | 1.0.2 | MIT |
| `import-meta-resolve` | 4.2.0 | MIT |
| `internal-slot` | 1.1.0 | MIT |
| `is-array-buffer` | 3.0.5 | MIT |
| `is-async-function` | 2.1.1 | MIT |
| `is-bigint` | 1.1.0 | MIT |
| `is-boolean-object` | 1.2.2 | MIT |
| `is-callable` | 1.2.7 | MIT |
| `is-data-view` | 1.0.2 | MIT |
| `is-date-object` | 1.1.0 | MIT |
| `is-document.all` | 1.0.0 | MIT |
| `is-finalizationregistry` | 1.1.1 | MIT |
| `is-generator-function` | 1.1.2 | MIT |
| `is-map` | 2.0.3 | MIT |
| `is-negative-zero` | 2.0.3 | MIT |
| `is-number-object` | 1.1.1 | MIT |
| `is-regex` | 1.2.1 | MIT |
| `is-set` | 2.0.3 | MIT |
| `is-shared-array-buffer` | 1.0.4 | MIT |
| `is-string` | 1.1.1 | MIT |
| `is-symbol` | 1.1.1 | MIT |
| `is-typed-array` | 1.1.15 | MIT |
| `is-weakmap` | 2.0.2 | MIT |
| `is-weakref` | 1.1.1 | MIT |
| `is-weakset` | 2.0.4 | MIT |
| `isarray` | 2.0.5 | MIT |
| `js-yaml` | 5.5.0 | MIT |
| `math-intrinsics` | 1.1.0 | MIT |
| `mdn-data` | 2.27.1 | CC0-1.0 |
| `mdn-data` | 2.37.2 | CC0-1.0 |
| `object-inspect` | 1.13.4 | MIT |
| `object-keys` | 1.1.1 | MIT |
| `object.assign` | 4.1.7 | MIT |
| `own-keys` | 1.0.2 | MIT |
| `possible-typed-array-names` | 1.1.0 | MIT |
| `postcss-safe-parser` | 7.1.0 | MIT |
| `postcss-selector-parser` | 7.1.8 | MIT |
| `postcss-value-parser` | 4.2.0 | MIT |
| `reflect.getprototypeof` | 1.0.10 | MIT |
| `regexp.escape` | 2.0.1 | MIT |
| `regexp.prototype.flags` | 1.5.4 | MIT |
| `safe-array-concat` | 1.1.4 | MIT |
| `safe-push-apply` | 1.0.0 | MIT |
| `safe-regex-test` | 1.1.0 | MIT |
| `set-function-length` | 1.2.2 | MIT |
| `set-function-name` | 2.0.2 | MIT |
| `set-proto` | 1.0.0 | MIT |
| `side-channel-list` | 1.0.1 | MIT |
| `side-channel-map` | 1.0.1 | MIT |
| `side-channel-weakmap` | 1.0.2 | MIT |
| `side-channel` | 1.1.1 | MIT |
| `signal-exit` | 4.1.0 | ISC |
| `stop-iteration-iterator` | 1.1.0 | MIT |
| `string.prototype.trim` | 1.2.11 | MIT |
| `string.prototype.trimend` | 1.0.10 | MIT |
| `string.prototype.trimstart` | 1.0.8 | MIT |
| `strip-bom` | 3.0.0 | MIT |
| `stylelint-declaration-strict-value` | 1.13.0 | MIT |
| `stylelint` | 18.0.0 | MIT |
| `synckit` | 0.12.3 | MIT |
| `tailwind-csstree` | 0.3.3 | Apache-2.0 |
| `tsconfig-paths-webpack-plugin` | 4.2.0 | MIT |
| `tsconfig-paths` | 4.2.0 | MIT |
| `typed-array-buffer` | 1.0.3 | MIT |
| `typed-array-byte-length` | 1.0.3 | MIT |
| `typed-array-byte-offset` | 1.0.5 | MIT |
| `typed-array-length` | 1.0.8 | MIT |
| `unbox-primitive` | 1.1.0 | MIT |
| `util-deprecate` | 1.0.2 | MIT |
| `valibot` | 1.5.0 | MIT |
| `which-boxed-primitive` | 1.1.1 | MIT |
| `which-builtin-type` | 1.2.1 | MIT |
| `which-collection` | 1.0.2 | MIT |
| `which-typed-array` | 1.1.24 | MIT |
| `write-file-atomic` | 8.0.0 | ISC |

## 규칙 비활성화 증거

`DS02 examples match exact rule sets and disabling each enforcing rule loses its witness`는 아래 예제마다 해당 규칙만 켜면 오류, 해당 규칙도 끄면 진단 0개임을 직접 확인한다. 기대 집합 비교는 켠 상태의 오류가 없어지면 실패한다.

| R-14 # | 대표 예제 | 비활성화한 규칙 | 결과 |
| --- | --- | --- | --- |
| 1 | class-0 (bg-red-500) | no-unknown-classes | 진단 0개 |
| 2 | class-7 (p-(--x)) | no-restricted-classes | 진단 0개 |
| 3 | class-9 (z-50) | no-restricted-classes | 진단 0개 |
| 4 | class-21 (dark:bg-primary) | no-restricted-classes | 진단 0개 |
| 5 | class-29 (text-up) | no-restricted-classes | 진단 0개 |
| 6 | dynamic-variable / dynamic-template | static-classes / no-concatenated-classes | 각각 진단 0개 |
| 7 | inline | no-inline-style | 진단 0개 |
| 8 | literal / literal-template | no-style-literals | 진단 0개 |
| 15 | CSS #fff / padding:13px | color-no-hex / declaration-strict-value | 각각 진단 0개 |

미등록 var는 `--missing`을 등록 목록에 넣는 변형에서 통과한다. 생성 토큰 목록에 없는 var가 실제 정상 설정에서 실패하는 검사를 별도로 둔다.

최종 검증의 로그는 작업 사본 `target/ds-02/`에 둔다. 시작 기준 상태 전체 검사는 baseline.log(종료 0), DS 전용 검사는 green7.log(6/6), ESLint 전체 예제는 violations3.log(0 failure), Knip 변형은 knip-test2.log(7/7)이다. 앞선 전체 검사 두 회는 각각 의존성의 동적 로드와 lint 설정 경로의 URL 추적으로 실패했으며, 두 원인은 기존 테스트를 약하게 바꾸지 않고 구현에서 수정했다.

cn의 로컬 선언·매개변수·가져오기는 스코프 바인딩 검사로 거부한다. 승인된 cn 구현은 아직 없다. 전용 예제의 미해결 cn은 문법 검사 대조군이며 실제 앱에서는 타입 검사도 통과해야 한다. cn-binding 회귀 예제는 수정 전 실패와 수정 후 통과를 확인한다.
