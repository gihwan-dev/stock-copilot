# H-43: 매크로 토큰을 포함하는 보수적 변이 참조 그래프

- 상태: 초안, 메인 평가 대기
- 기준: 로컬 master 4c46c32e9239384b24ca0e13869b118fe2a3a3c7에서 h-43 생성
- ADR 0042의 '매크로가 하나라도 있으면 전체' 규칙을 대체한다. 기준·현재 판의 합집합, 외향 전이 폐쇄, 내향 직접 한 단계, 마지막 대상 글롭 교차는 유지한다.
- 기존 master 테스트, 대상 글롭, 생존 허용 목록, 시간 초과 실패, jobs 및 밤 작업은 변경하지 않는다. 새 의존성·새 검사 끄기는 없다.

## 결정

매크로 인자를 포함한 모든 토큰 그룹을 재귀적으로 읽어 식별자·경로 구성요소를 모은다. 모듈, 재수출/별칭, 타입, 함수, 상수의 이름에 맞는 모든 후보 소유 파일로 간선을 건다. 구현과 테스트 토큰을 구분해서 빼지 않는다. 이름 가리기와 동명 항목도 후보 합집합으로 처리하므로 과대 근사다.

crate 내부 macro_rules 정의의 이름·소유 파일·본문 식별자를 모으고, 호출 토큰에 본문 참조를 고정점까지 합친다. 다른 파일의 정의와 재수출 별칭에도 정의 파일 간선을 더한다. 정의 본문에 항목 생성(fn/struct/mod/impl 등)이 있거나 토큰 합성·알 수 없는 매크로가 있으면 전체로 넓힌다. 현재 대상 코드에는 로컬 macro_rules 정의가 없으며 표현식형 정의/다른 파일/재수출 별칭은 자체 테스트로 확인한다.

표준·외부 매크로의 이름은 닫힌 표다. 경로의 첫 네임스페이스와 use 별칭의 원래 네임스페이스를 함께 확인한다. external::proptest, external::std처럼 익숙한 이름이 경로 중간에 있다는 이유로 신뢰하지 않는다. 표에 없는 외부 매크로·속성·derive는 전체다. 매크로 안에 중첩된 매크로와 속성도 검사한다.

super가 보인다는 이유만으로 형제 파일 전부를 넣던 확장을 제거했다. super 뒤의 항목과 부모 파일의 코드/가져오기에도 동일한 이름 후보 규칙을 적용한다. 일반 glob 가져오기는 토큰에 등장하는 이름의 후보와 모듈 소유자로 연결하지만, 공개·제한 공개 glob 재수출은 전체다. include!, #[path], mod.rs, 외부 테스트 지원 모듈과 판정 실패의 기존 전체 확장은 유지한다.

format 계열의 암시적 `{NAME}` 캡처도 문자열의 중괄호 안에서 이름을 모아 참조로 센다. 이스케이프된 중괄호 등 실제 참조가 아닌 경우도 추가할 수 있다. concat!/stringify!/env! 등으로 포맷 문자열의 이름을 합성하는 형태는 전체다.

## 매크로 허용 표와 근거

조사 범위는 대상 crate의 Rust 소스와 고정된 proptest 1.12.0 소스다. 실제 외부 호출은 아래 다섯 선언형 매크로다. 외부 절차 매크로/속성/derive의 사용은 없었다. 이름만 같은 외부 매크로를 이 표로 허용하지 않는다.

| 매크로/속성 | 처리와 근거 |
|---|---|
| assert!, assert_eq!, assert_ne!, debug_assert!, debug_assert_eq!, debug_assert_ne! | 조건/비교/메시지 인자 토큰을 읽는다. 생성 코드는 표준 라이브러리와 입력 식의 연산을 호출한다 |
| matches!, vec!, format!, format_args!, write!, writeln!, print!, println!, eprint!, eprintln!, panic!, unreachable! | 인자 전체와 암시적 포맷 캡처를 읽는다. 실제 사용은 assert 계열·format·vec이며 나머지도 표준 의미가 명확한 닫힌 목록 |
| concat!, stringify!, file!, line!, column!, module_path!, env!, option_env! | 표준 상수/문자열 생성. 인자만 읽되 포맷 식별자 합성과 include 조합은 전체 |
| proptest! | 인자의 전략·설정·본문을 읽는다. sugar.rs 151행 이후의 확장은 proptest의 $crate와 core를 쓰며 내부의 다른 앱 항목은 입력에 남는다 |
| prop_assert! | sugar.rs 748행 이후. 입력 조건/포맷을 평가하고 proptest TestCaseError와 alloc/core를 호출한다 |
| prop_assert_eq! | sugar.rs 792행 이후. 입력 두 식과 포맷을 평가하고 proptest prop_assert로 전달한다 |
| prop_assert_ne! | sugar.rs 840행 이후. eq와 같은 구조의 입력 부등 비교 |
| prop_oneof! | sugar.rs 328행 이후. 입력 전략/가중치를 proptest TupleUnion/Arc 등에 넣는다 |
| derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash) | 표준 내장 derive만. 필드 타입과 구현 소유자를 기존 이름 인덱스로 읽는다. 외부에서 같은 이름으로 가져온 derive는 신뢰하지 않는다 |
| cfg(test), test, repr, must_use, default, macro_export, macro_use | 내장 속성. 새로운 다른 내부 항목 이름을 주입하지 않는다 |
| proptest_config | proptest! 인자 안의 설정 도우미만. 독립 속성이나 가져온 동명 속성은 허용하지 않는다 |
| 그 밖의 외부 호출/속성/derive | crate 전체 |

근거: [proptest 1.12.0의 실제 확장 소스](https://docs.rs/crate/proptest/1.12.0/source/src/sugar.rs), [Rust 내장 derive](https://doc.rust-lang.org/reference/attributes/derive.html). proptest 매크로가 추가하는 고정 이름은 proptest/core/alloc 쪽이며 호출 crate의 다른 항목을 입력 없이 새로 참조하지 않는 것을 위 다섯 본문에서 확인했다. 버전·기능·매크로 사용이 바뀌면 이 판단을 다시 한다.

## 회귀 검증

기존 H-42 회귀 (a)(b)는 변경 없이 유지한다. 새 weakened_macro_only_tests_keep_the_referenced_module은 assert_eq!, proptest!, matches! 안에만 있는 다른 모듈 참조를 지운 변경에서 기준 판의 간선이 남는지 검사한다.

actual_portfolio_boundary_deletion_reaches_exact_arithmetic_through_graph는 실제 domain 소스를 읽고 portfolio_valuation.rs의 1.01/0.99/-1.01/-0.01 경계 행 네 개를 사본에서 삭제한다. 기준·삭제 후 양쪽 Graph::read가 성공해야 하며, fallback 없이 outward 결과에 exact_arithmetic.rs가 있어야 한다. 실제 작업 파일과 기존 테스트는 고치지 않는다.

## 범위 실측

현재 기준 master에는 calibration이 포함되어 **52파일·1,289변이**다. H-42 당시의 51파일·1,200개를 현재 기준 값으로 쓰지 않았다. 변이 수는 cargo mutants --no-config --list의 열거값이며 전체 변이를 여섯 번 실행했다고 주장하지 않는다.

| 변경 경로 | H-42 규칙의 현재 범위 | H-43 선택 파일 | H-43 변이 |
|---|---|---:|---:|
| account_id.rs | 52/52, 1289/1289 | 6/52 (11.54%) | 180/1289 (13.96%) |
| money.rs | 52/52, 1289/1289 | 51/52 (98.08%) | 1281/1289 (99.38%) |
| exact_arithmetic.rs | 52/52, 1289/1289 | 51/52 (98.08%) | 1281/1289 (99.38%) |
| portfolio_valuation.rs | 52/52, 1289/1289 | 51/52 (98.08%) | 1281/1289 (99.38%) |
| money.rs의 테스트만 | 52/52, 1289/1289 | 51/52 (98.08%) | 1281/1289 (99.38%) |
| calibration.rs만 | 52/52, 1289/1289 | 51/52 (98.08%) | 1281/1289 (99.38%) |
| 새 enum 모듈 + lib.rs 등록 | 53/53, 1289/1289 | 53/53 (100%) | 1289/1289 (100%) |

마지막 행은 실제 h43_probe.rs와 lib 등록·재수출을 임시로 넣어 scope와 cargo-mutants 열거를 확인한 뒤 바이트 원본을 복구했다. enum만 있어서 새 변이는 0개다. lib의 mod 선언 변경은 ADR 0042의 전체 확장 규칙을 유지하므로 등록을 동반하는 새 모듈 추가는 절감되지 않는다. calibration만 변경하는 경우와 이 경우를 섞지 않는다.

money 계열은 공용 계산, 테스트 도우미와 동명 항목의 후보 합집합 때문에 크게 연결된다. macro fallback이 아니라 실제 그래프 선택이다. 이번 결과를 전반적인 큰 범위 절감으로 포장하지 않는다.

## 독립 커버리지 대조

구현 전에 파일별 테스트만 독립 실행했다. 명령은 각 이름에 대해 `CARGO_LLVM_COV_TARGET_DIR=$PWD/target/h43/coverage cargo llvm-cov nextest --locked --package domain --json --output-path target/h43/coverage-<이름>.json -E 'test(/^<이름>::/)'`다. no-clean과 기존 프로필 재사용 없이 매번 도구의 기본 정리를 수행했다. 성공한 테스트 실행의 LLVM JSON에서 count>0이고 hasCount인 segment가 있는 domain 파일을 관측 집합으로 삼았다. holding_valuation에는 테스트가 없어 종료 4였으므로 표본에서 빼고 테스트가 있는 파일 열 개를 골랐다. 테스트 0개를 통과로 바꾸지 않았다.

대조 대상은 내향 파일을 더하기 전 **Graph::outward**다. 각 관측 집합이 그 집합에 포함되는지 검사했고 누락은 0이다. 이 실행 표본의 포함 검증이며 모든 미래 입력/동적 호출의 건전성 증명은 아니다.

| 테스트 파일 | 실행된 파일(자기 포함) | 다른 파일 | 외향 닫힘 파일 | 누락 |
|---|---:|---:|---:|---:|
| account_id | 1 | 0 | 1 | 0 |
| money | 1 | 0 | 50 | 0 |
| exact_arithmetic | 6 | 5 | 50 | 0 |
| portfolio_valuation | 15 | 14 | 50 | 0 |
| fx_rate | 7 | 6 | 50 | 0 |
| calibration | 8 | 7 | 50 | 0 |
| symbol | 1 | 0 | 1 | 0 |
| ratio | 3 | 2 | 50 | 0 |
| probability | 1 | 0 | 50 | 0 |
| price | 3 | 2 | 50 | 0 |

예를 들어 portfolio의 다른 관측 파일은 domain_time, exact_arithmetic, fx_conversion, fx_rate, holding, holding_valuation, money, numeric_input, order_intent, portfolio, price, quantity, ratio, symbol이다. 나머지 원본 관측 집합과 보고서는 target/h43/coverage-files.json 및 coverage-*.json, 그래프 대조는 soundness.tsv, 범위는 ranges.tsv/addition.tsv에 있다. 이 측정용 임시 Rust 테스트는 결과를 얻은 뒤 제거했고 커밋에는 재현 가능한 회귀 테스트만 남긴다.

## 남는 구멍

다른 crate의 도우미 변경(예: testkit)이 domain 테스트의 의미를 바꾸는 간선은 여전히 별도다. 현재 후보 이름 방식은 지역 가리기·동명 도우미에서 과선택하므로 money 계열의 절감이 작다. 외부 매크로의 표를 늘릴 때 호출 crate의 고정 항목 이름을 주입하는 확장을 잘못 허용하면 토큰에 없는 간선을 놓칠 수 있다. 이번 허용은 위 고정 버전과 표준 의미에 한정한다.

매크로의 항목/식별자 생성, 미등록 외부 매크로, 중첩된 미등록 호출, 모호한 glob 재수출, include/path/mod.rs는 전체로 넓힌다. 이 경우 절감을 보장하지 않는다. LLVM 대조는 실제로 실행된 줄의 하한만 관측하며 테스트가 실행하지 않은 경로·향후 입력을 증명하지 않는다. cargo-mutants가 생성하지 않는 변이와 외부 생성 코드 자체도 검증 범위 밖이다. ADR 0042의 독립 밤 전체 검사와 실패 이슈의 병합 전 해결 규칙은 유지한다.

## 자체 검증

기준 전체 check는 통과했다. 기존 master 테스트는 바꾸지 않았고 H-42의 (a)(b)를 포함한 변이 하네스 관련 37개가 통과했다. 손 변형 16개(표준 허용, proptest 허용, derive/속성 검증, 그룹 식별자 수집, 포맷 캡처, 로컬 본문 합치기, 정의 소유 파일, 별칭 전파, 중첩 검사, 미등록 호출 차단, 잘못된 가져오기/네임스페이스 차단, 합성 포맷 차단, 항목 생성 차단, glob 재수출 차단)는 모두 컴파일 후 실제 테스트 실패(종료 100)로 검출했다. 최초 본문 합치기 변형은 unused_mut로 컴파일에서 막혔으므로 빈 참조를 합치는 동등한 손 변형으로 다시 실행해 테스트 실패를 확인했다. 원복 후 형식·Clippy·관련 37개를 다시 통과했다. 로그는 target/h43/manual.json, manual-*.log, post-manual-*.log다.

최종 전체 검사는 훅을 건너뛰지 않는 일반 커밋의 fresh-build → commit-guard → just check로 수행한다. 새 ADR을 추적한 뒤 설정·잠금 조각·훅 해시는 기존 스크립트의 쓰기 모드로 재생성한다. 원시 커버리지/열거 결과를 새 테스트의 기대값으로 복사하지 않았고, 커버리지 관측은 구현 전에 별도 테스트 실행으로 만들었다.
