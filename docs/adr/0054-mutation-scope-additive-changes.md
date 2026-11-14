# ADR 0054: H-43 변이 검사 범위: 매크로 해석과 추가 전용 변경

- 상태: 확정 (2026-11-14)
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

초기 H-43 커밋 77e22a0의 실제 domain 소스에서 portfolio_valuation.rs의 1.01/0.99/-1.01/-0.01 경계 행 네 개를 삭제한 사본을 측정했을 때, 기준·삭제 후 양쪽 Graph::read와 fallback 없는 outward 폐쇄가 성공했고 exact_arithmetic.rs가 포함됐다. 이는 해당 판의 측정 기록이다. 최신 domain 구조가 항상 같다는 테스트 전제로 사용하지 않는다. 회귀는 아래 병합 후 검증의 고정 소스로 옮겼다.

## 범위 실측(추가 전용 규칙 도입 전)

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

마지막 행은 실제 h43_probe.rs와 lib 등록·재수출을 임시로 넣어 scope와 cargo-mutants 열거를 확인한 뒤 바이트 원본을 복구했다. enum만 있어서 새 변이는 0개다. 이 1차 측정 때는 lib의 mod 선언 변경에 ADR 0042의 전체 확장 규칙을 적용했으므로 등록을 동반하는 새 모듈 추가는 절감되지 않았다. 아래 추가 전용 규칙이 이 경우를 대체한다. calibration만 변경하는 경우와 이 경우를 섞지 않는다.

money 계열은 공용 계산, 테스트 도우미와 동명 항목의 후보 합집합 때문에 크게 연결된다. macro fallback이 아니라 실제 그래프 선택이다. 이번 결과를 전반적인 큰 범위 절감으로 포장하지 않는다.

## 독립 커버리지 대조(최초 H-43 측정)

구현 전에 파일별 테스트만 독립 실행했다. 명령은 각 이름에 대해 `CARGO_LLVM_COV_TARGET_DIR=$PWD/target/h43/coverage cargo llvm-cov nextest --locked --package domain --json --output-path target/h43/coverage-<이름>.json -E 'test(/^<이름>::/)'`다. no-clean과 기존 프로필 재사용 없이 매번 도구의 기본 정리를 수행했다. 성공한 테스트 실행의 LLVM JSON에서 count>0이고 hasCount인 segment가 있는 domain 파일을 관측 집합으로 삼았다. holding_valuation에는 테스트가 없어 종료 4였으므로 표본에서 빼고 테스트가 있는 파일 열 개를 골랐다. 테스트 0개를 통과로 바꾸지 않았다.

이 표본 열 개 중 여덟 개의 외향 닫힘은 50/52파일(96.15%)로 사실상 crate 전체였다. 누락 0은 이 큰 집합에 대한 포함 결과이며, 좁게 선택할 때의 증거로는 약하다. 다음 리뷰 보완의 빠진 파일 변이 대조가 그 약점을 보완한다. 이 표는 최초 H-43의 관측으로, 아래 수정 뒤의 새 범위 수치라고 주장하지 않는다.

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


## 후속 결정: 추가 전용 변경

후속 기준은 h-43의 77e22a07a90f879f4dd6de1e273cb39c65bc03a7이다. 기존 항목과 테스트가 그대로이고 이름·메서드 해석을 바꾸지 않는 추가라면 기존 변이가 새로 생존하지 않는다는 조건으로 선택을 줄인다. 단순히 텍스트 줄이 더해졌다는 사실은 충분하지 않다. 아래 가림 공격에서 실제 반례를 찾았으므로 브리프의 무조건적인 새 파일 허용과 새 trait 가져오기를 좁힌다.

syn으로 기준·현재 판을 파싱하고 파싱한 항목 자체를 토큰 열로 출력해 비교한다. 원문 span 슬라이싱은 사용하지 않는다. 기존 항목·속성·순서가 모두 유지되는 부분 수열인지 확인한다. 기존 cfg(test) 모듈은 헤더가 동일하고 내부의 모든 기존 항목이 동일한 토큰과 순서로 남아야 한다. 공백만 달라 토큰이 같은 변경은 보수적으로 기존 그래프에 맡긴다. 주석은 도입하지 않으며 기존 주석 금지 검사도 유지한다.

| 변경 | 판정과 선택 |
|---|---|
| 기준에 없는 대상 Rust 파일 | 검증 가능한 새 선언과 자체 새 struct/enum에 대한 impl만 있으면 그 파일만. 기존·외부 타입 impl, alias impl, 매크로 정의/항목 생성, 미확인 형식은 그래프 |
| lib.rs의 등록 추가 | 기존 항목·파일 속성은 그대로. 실제 새 파일에 대응하는 외부 mod 선언과 명시적 pub use만 허용. lib 때문에 확장하지 않음 |
| 공개 이름 추가 | glob·기존 이름 충돌·trait/외부/불명확한 이름 재수출은 그래프. 기준 crate의 식별자 전체에 이미 등장한 공개 이름도 거부하여 prelude 가림까지 보수적으로 방지 |
| 기존 파일의 항목 추가 | 새 자유 함수·const·static·struct·enum·type·trait, 동일 파일의 실제 최상위 범위에 무조건 선언한 새 명목 타입에 대한 최상위 impl, 기존 cfg(test) 모듈 내부의 새 테스트·도우미만 허용. 그 파일 전체의 변이 선택. cfg 선언을 근거로 한 impl과 self 이름을 가리는 타입 매개변수는 거부 |
| 새 use | glob이 아니고 기준 파일에서 쓰지 않던 바인딩, 새 항목에서 사용하는 이름, 내부의 명확한 데이터/함수/타입/모듈 후보에 한정. trait와 외부·불명 후보 또는 이들과 이름이 겹친 후보는 거부 |
| 기존 glob/불투명 import | crate에 glob use 또는 매크로 인자의 use가 남아 있으면 기존 파일·lib 등록의 추가 전용 판정은 거부. 새 항목 이름의 가림을 확정할 수 없음 |
| 혼합 변경 | 추가 전용 파일들과 그 밖의 변경에서 얻은 기존 그래프 선택의 합집합. 대상 밖 Rust 파일도 판정과 그래프 노드에는 포함 |
| 매니페스트·설정·지원 파일·삭제·이름 변경·파싱/그래프 불확실성 | 기존 crate 전체 조건 우선. 추가 전용으로 판단해도 외향 그래프의 불확실성 검사는 통과해야 함 |

항목 이름의 신선함은 요청의 선언 이름보다 엄격하게 기준 crate 전체의 식별자로 검사한다. 단순 타입 별칭을 새 명목 타입으로 보지 않는다. 새 타입 이름과 같은 지역 별칭으로 기존 타입을 가리킨 뒤 중첩 impl을 만드는 경우도 차단한다. 일반 모듈의 신설·변경, 기존 impl 안의 메서드 추가, 새 cfg(test) 모듈 자체의 신설은 기존 파일의 추가 전용으로 인정하지 않는다. 기존 cfg(test) 모듈 안에 타입과 impl을 함께 추가하는 경우도 명목 타입의 범위 해석을 보수적으로 유지하여 그래프에 맡긴다. 기존 테스트 모듈의 새 자유 함수와 도우미 선언은 허용한다. 추가 코드의 매크로 인자에 impl/use/mod/macro_rules/extern이 있으면 AST로 소유 범위를 확정할 수 없으므로 추가 전용을 거부한다. 일반 assert 인자에 있는 참조는 허용한다.

### 거부 회귀

기존 타입의 inherent 메서드·연관 상수·trait impl, 기존 함수 본문 한 글자 변경, 기존 테스트 삭제/변경, 기존 항목/파일 속성 변경, glob use, 다른 모듈에 이미 있는 이름, 공개 이름 재수출 충돌, 새 macro_rules와 매크로 호출 항목, 기존 impl 내부 메서드 추가, 테스트를 cfg(test) 밖으로 이동, 기존 타입을 가리키는 새 type alias impl, trait 재수출 별칭, 새 항목에서 사용하지 않는 import를 거부 예로 고정한다. 이름 충돌 없는 함수·상수·타입·새 타입 impl, 기존 테스트 모듈 내부의 새 테스트, 안전한 내부 데이터 import와 별칭, 새 파일+등록, 혼합 합집합은 통과 예로 고정한다.

H-42의 weakened_portfolio_boundary_selects_exact_arithmetic, removed_holding_reference_keeps_baseline_exact_arithmetic는 그대로 유지한다. H-43의 portfolio_boundary_deletion_reaches_exact_arithmetic_through_graph와 removed_compare_call_keeps_baseline_dependency_through_graph는 고정 소스로 같은 경로를 검증한다. 추가 전용 분기로 들어가지 않고 기준·현재 그래프에서 exact_arithmetic에 닿아야 한다.

### 가림 공격 실험

독립 Cargo fixture 여덟 개에서 같은 기존 테스트의 바이트를 유지했다. 첫 네 fixture의 기존 테스트 모듈 SHA-256은 9a6e672751cdb6d07f1225351e9b0542e3b931fde0f77f15115b15e837f303b6이다. victim::boundary의 1을 0으로 바꾼 동일한 손 변이는 추가 전에는 테스트 실패(종료 100), 추가 후에는 통과(종료 0)했다. 변이 없는 판은 양쪽 모두 통과했다.

| 추가한 코드 | 기존 변이가 생존한 이유 | 최종 판정 |
|---|---|---|
| 기존 Value에 inherent read 추가 | 기존 Old::read 대신 inherent 메서드를 선택 | 그래프 |
| 기존 Value에 inherent SCORE 연관 상수 추가 | 기존 trait 상수 대신 inherent 상수 선택 | 그래프 |
| 기존 FreshTrait을 새 ScopeFresh 이름으로 use하고 이를 쓰는 새 도우미 추가 | Value의 정확한 수신자 메서드가 기존 &Value trait보다 우선 | 그래프. 데이터로 확정되지 않은 새 trait import 금지로 규칙 강화 |
| 새 extra.rs에 기존 Value의 inherent read impl + lib mod 추가 | 새 파일의 impl이 기존 호출 해석을 변경 | 새 파일도 그래프. 새 파일의 기존 타입 impl 금지로 규칙 강화 |
| 최상위 Fresh 타입 + 기존 테스트 모듈 안의 Value as Fresh 별칭과 impl | 다른 범위의 새 타입 이름을 빌려 기존 Value의 메서드를 가림 | 그래프. 명목 타입과 impl 모두 실제 최상위 AST 항목인지 확인 |
| 새 Fresh 타입 + impl<Fresh> Candidate for Fresh | 타입 매개변수가 새 타입을 가려 실제로는 기존 Value에도 적용되는 blanket impl 생성 | 그래프. self 타입 식별자와 같은 타입 매개변수 거부 |
| cfg(not(test)) Fresh 타입 + cfg(test) Fresh = Value 별칭 + impl Fresh | 테스트 빌드에서 self가 기존 Value가 됨 | 그래프. cfg로 존재가 달라지는 선언을 명목 타입 소유 증거에서 제외 |
| 기존 std::borrow::* 옆에 Borrow 타입 추가 | glob trait을 가려 &self 호출이 기존 다른 &mut self trait 호출로 바뀜 | 그래프. glob이 남아 있는 crate의 기존 파일과 lib 등록은 추가 전용 판정 제외 |

다섯 번째 fixture도 [0, 100, 0, 0]을 재현했고, 기존 테스트 함수 SHA-256은 02beae7da214f66b478039a61d26bc8dc74c6fc97d4ea97c3eb23b2c5f2b3d3b다. 원본은 nested-attack.py/json과 attacks/nested_alias에 있다. 마지막 세 fixture도 [0, 100, 0, 0]을 재현했다. more-attacks.py, glob-attack.py, extra-attacks.json과 attack-*.log에 남겼다. 별도 대조로 IntoIterator 타입 이름 추가를 시도했지만 [0, 100, 0, 100]이어서 기존 변이는 계속 검출됐다. 이를 생존 재현으로 세지 않았다. 첫 두 개는 제안된 impl 제한으로 차단된다. 다음 두 개는 제안 규칙의 실제 반례이며 위 추가 제한의 근거다. 나머지는 자체 최종 점검에서 확인한 추가 반례다. 실제 최상위 AST 소속, generic shadowing, cfg와 glob의 불확실성을 추가로 제한했다. 변경 줄의 추가 여부와 test-diff만으로는 기존 메서드 해석 보존을 증명할 수 없다. 결과와 원본 fixture는 target/h43-additions/attacks.json, attacks/*, attacks.py에 남겼다. 커밋의 actual_shadowing_attacks_use_graph_instead_of_addition_shortcuts, new_files_cannot_add_existing_type_impls_or_global_macros와 import 회귀는 강화한 판정을 고정한다.

### test-diff와 같은 기준

justfile의 check는 test-diff에 전역 base를 넘기고, check-full은 그 check 뒤에 같은 base로 mutants-diff를 실행한다. mutants-diff는 이를 xtask mutants --in-diff에 넘긴다. 추가로 mutants 명령 자체의 checked_scope가 기준을 한 번 40자리 SHA로 확정하고 crate::test_diff::check(root, 그 SHA)를 성공시킨 뒤 scope_for에도 같은 SHA를 전달한다. test-diff 발견 사항이나 테스트 열거 실패는 변이를 실행하기 전에 오류로 반환한다. diff_command_requires_test_diff_at_the_resolved_base가 기존 테스트를 약화한 fixture에서 이 차단을 확인한다.

따라서 별도 mutants 호출에서도 이 의존성을 잃지 않는다. 이 내부 검사를 끄거나 범위 판정에 다른 기준을 넘기는 변경은 추가 전용 규칙의 전제를 깨므로 허용하지 않는다. 기존 테스트가 기준과 같은 토큰이어도 지원 코드·목록의 약화가 있으면 test-diff의 기존 규칙으로 실패한다. 작업 중 소스가 동시에 바뀌지 않는다는 기존 검사 전제는 유지한다.

### 실제 병합 이력 여섯 건

로컬 master의 첫 부모 이력 중 domain을 변경한 최근 병합 여섯 건(D-05, D-20, D-23, D-06, D-28, D-32)을 사용한다. DS·하네스만 바꾼 병합을 끼워 0변이 사례로 절감을 부풀리지 않는다. 각 병합의 실제 첫 부모를 git rev-parse로 구하고 양쪽 domain 소스를 같은 선택 함수에 전달했다. 각 병합의 git archive 사본에서 cargo mutants --no-config --gitignore=false --manifest-path crates/domain/Cargo.toml --list로 전체와 선택 파일의 변이를 각각 열거했다. 현재 domain에 과거 변경을 덮어씌운 수치가 아니다.

| 병합 | 커밋 | 이전 파일/변이 | 추가 규칙 후 파일/변이 | 변이 비율 |
|---|---|---:|---:|---:|
| D-05 | 8da33c4c6a1515cc47cf4c56b8493fb1a48f6ccc | 47 / 1124 | 46 / 1116 | 99.29% |
| D-20 | af70b46fc6c62a0d1a7b036c9977b3d2f3d47366 | 51 / 1200 | 4 / 76 | 6.33% |
| D-23 | 6e417f0796ab8f58cbcd96f335dec169312e6947 | 52 / 1289 | 1 / 89 | 6.90% |
| D-06 | 09bc27a58efa647b041effcdcbf6528124879cd3 | 54 / 1392 | 53 / 1384 | 99.43% |
| D-28 | 9f5dc21a664fb8b66b1f751ff2a38cee072686e9 | 58 / 1710 | 57 / 1702 | 99.53% |
| D-32 | e7a2064afc584dd464cad73f845d35fb6669d76e | 61 / 1784 | 60 / 1776 | 99.55% |

이전 선택은 lib mod 등록 변경으로 crate 전체가 된다. D-20의 네 새 파일과 D-23 calibration은 새 규칙으로 줄어든다. D-05의 fx_conversion은 기존 항목을 바꾸고, D-06은 기존 계산 파일에 새 cfg(test) 모듈을 더하며, D-28은 기존 ApproximateRatio impl 안에 new 메서드를 넣고, D-32는 proposal.rs에 새 테스트 지원 모듈을 넣으므로 그래프를 유지한다. 이 경우까지 추가 전용으로 넓혀 인정하지 않는다. 최초 D-20 분석에서는 기존 필드 이름 horizon을 새 mod 이름 충돌로 과도하게 거부했다. 실제 새 파일 mod 조건으로 바로잡은 뒤 여섯 건을 전부 다시 측정했다.

D-23은 jobs=6, timeout-multiplier=30으로 89개를 308.49초에 완료했다: caught 45, unviable 44, missed 0, timeout 0, 종료 0. 가장 작은 D-20도 동일 옵션으로 76개를 433.23초(7분 13초)에 완료했다: caught 61, unviable 15, missed 0, timeout 0, 종료 0. 병렬 작업 부하와 변이별 빌드 차이가 있어 서로 다른 두 범위의 시간을 변이 수에 비례한다고 보지 않는다. 여섯 이력의 전체 변이 실행 시간은 실측하지 않았다. 열거와 실행 로그는 target/h43-additions/history.json, history-selections.txt, D-*-list.txt, actual-D*.json/log와 각 mutants.out에 있다.

### 후속 규칙의 남는 구멍

추가 전용은 Rust의 모든 이름·메서드 해석을 증명하는 일반 분석기가 아니다. 외부/trait import, 기존 타입 impl, 매크로 정의와 모듈 구조 변경을 거부하고 알려지지 않은 형식은 그래프로 돌린다. 이 때문에 안전한 추가도 범위가 크게 잡힐 수 있다. 식별자 신선함을 과도하게 잡은 경우와 새 테스트 모듈 신설이 대표적이다. 다른 crate·외부 생성 코드·동시 소스 변경에 관한 앞 절의 한계는 유지한다. 가림 반례를 새로 찾으면 그 형태를 추가 전용에서 제외하고 회귀 테스트를 더한다. 밤 전체 검사와 실패 이슈를 다음 병합 전에 해결하는 운영 규칙도 그대로 유지한다.


### 후속 자체 검증(78e5ad5 당시)

scope·graph·새 판정의 손 변형 28개를 각각 적용해 컴파일한 뒤 관련 테스트가 종료 100으로 실패하는 것을 확인했다. 새 파일 검증 제거, 등록 단축 제거, 파일 속성 비교 제거, 항목 토큰 비교 뒤집기, 기존 항목 삭제 허용, 기존 테스트 모듈 토큰 비교 무시, 전역 이름 충돌 무시, 기존 타입 impl 허용, 중첩 impl 허용, trait import 허용, 미사용 import 허용, 공개 이름 충돌 허용, glob 가져오기 허용, 별칭 종류 전파 제거, 단일 파일 선택 제거, 혼합 그래프 합집합 제거, 추가 파일의 그래프 불확실성 무시, 기준 그래프 간선 제거, 외향 전이 제거, test-diff 차단 제거에 더해 명목 타입의 실제 범위, impl의 실제 범위, self 이름의 generic 가림, cfg 명목 타입, 기존 glob, 불투명 매크로 안의 항목/import, 루트 절대 경로의 거부 분기를 제거한 변형도 검출했다. 컴파일 실패를 검출 성공으로 세지 않았다. 기록은 target/h43-additions/manual.json과 manual-*.log다.

손 변형은 매번 원본으로 복구했고 최종 코드에서 다시 형식·Clippy·관련 52개 테스트가 통과함을 확인했다. 기존 master 테스트·보호 정책·글롭·허용 목록·의존성은 바꾸지 않았다. 일반 커밋 훅의 전체 검사도 생략하지 않는다.

최종 추가 제한을 적용한 뒤 이력 여섯 건의 선택 파일 목록을 다시 계산해 앞 측정과 바이트 단위로 일치함을 확인했다. 실제 가림 fixture 여덟 건은 RUSTFLAGS="-D warnings"에서도 [0, 100, 0, 0]을 재확인했다. glob 사례는 기존 BorrowMut 사용을 유지해 unused import 경고로 우연히 막히는 사례가 아니게 했다. 원본 소스를 그대로 읽은 임시 분류 감사에서도 여덟 건 모두 Graph, 기존 변이가 계속 잡히는 IntoIterator 대조만 OnlyFile이었다(attack-classifications-final.log). 감사용 테스트는 제거했다.


## 78e5ad5 적대적 리뷰 보완

### 추가 전용의 전제와 강제 범위

| 전제 | 강제 방법과 한계 |
|---|---|
| 기존 항목의 토큰 동일성이 정확하다 | syn AST를 `syn::parse_quote!(#item)`으로 proc_macro2::TokenStream에 보간한 뒤 문자열을 비교한다. 이 공용 매크로의 보간이 ToTokens를 사용한다. 항목·속성·테스트 모듈 헤더·공개 use 제거 뒤 lib 비교에서 모두 원문 위치 슬라이싱을 없앴다. quote 직접 의존성을 추가하지 않았다 |
| 이름 동일성이 정확하다 | 공통 rust_names에서 모든 식별자 색인의 r# 접두를 제거한다. 추가 판정, use/별칭/재수출, 그래프 정의·참조·매크로, test-diff 이름/참조/가림에 적용한다. ASCII가 아닌 식별자는 NFC를 임의로 추정하지 않고 추가 전용 거부·crate 전체 또는 test-diff 가림으로 처리한다. 한글 문자열 리터럴은 이 제한에 해당하지 않는다 |
| 암시적 이름 해석이 바뀌지 않는다 | Rust 2021/2024 prelude, 확인한 std/core/alloc 루트 이름, primitive 이름, 기준 판의 외부 use 원래 이름·별칭·루트·외부 경로와 extern crate 이름을 예약한다. fn drop, struct Some, raw Option/Iterator와 mod core/std/time, raw 재수출 등도 추가 전용에서 제외한다 |
| 기존 테스트와 지원 코드가 유지된다 | mutants --in-diff가 같은 확정 SHA로 test-diff를 먼저 실행하고 오류·발견 사항이 있으면 변이를 시작하지 않는다. 이를 끄는 옵션·환경 변수는 없다. 테스트 열거도 성공해야 한다 |
| Rust가 아닌 실행 입력이 유지된다 | 매니페스트·잠금·알려진 전역 설정은 전체 검사, 대상 crate 안의 src 밖 입력과 새 대상 밖 입력은 crate 전체다. build.rs/examples/benches도 포함한다. 임의의 crate 밖 파일·환경·다른 crate가 제공하는 입력까지 동일함을 증명하는 기능은 없으며, 이 전제가 깨지면 추가 전용 논증을 적용할 수 없다 |
| 위치 의존 매크로의 값이 유지된다 | 기준 crate에서 file/line/module_path 식별자를 발견하면 기존 파일과 lib 등록의 추가 전용 판정을 중지한다. 별칭·로컬 매크로 전달도 놓치지 않도록 crate 단위로 보수적으로 처리하여 같은 이름의 일반 변수도 과선택할 수 있다 |
| impl과 메서드 해석이 유지된다 | 기존 타입/외부 타입·generic self 가림·cfg 별칭·중첩 범위 impl을 추가 전용에서 제외한다. 그래프는 trait 경로 이름에도 구현 파일을 색인한다. self가 타입 매개변수인 blanket impl과 trait 객체는 정적 호출 소유를 확정하지 않고 crate 전체로 넓힌다. Proposal<State>의 Debug처럼 명목 타입에 대한 generic impl은 trait 이름 색인을 사용한다 |
| 검사 중 입력과 도구가 고정된다 | 두 판은 같은 고정 Cargo.lock·툴체인·동일한 nextest 패키지/프로필·샌드박스로 비교한다. 작업 사본의 동시 편집은 허용하지 않는다. 새 의존성·허용 목록·대상 글롭·시간 초과 통과는 추가하지 않았다 |

근거: [syn 2.0.119 parse_quote의 보간](https://docs.rs/syn/2.0.119/syn/macro.parse_quote.html), [Rust 2021 core prelude](https://doc.rust-lang.org/core/prelude/rust_2021/index.html), [Rust 2024 core prelude](https://doc.rust-lang.org/core/prelude/rust_2024/index.html), [std v1 prelude](https://doc.rust-lang.org/std/prelude/v1/index.html), [std](https://doc.rust-lang.org/std/index.html)·[core](https://doc.rust-lang.org/core/index.html)·[alloc](https://doc.rust-lang.org/alloc/index.html) 루트 목록. 이름 표는 고정 툴체인 1.99.1의 prelude와 확인한 표준 루트의 보수적 상위 집합이다. 툴체인·외부 매크로·가져오기를 바꾸면 다시 검토한다.

### Unicode 재현과 회귀

`pub const LIMIT: usize = "가".len() + 17;`을 +18로 바꾸고 새 함수를 더한 재현은 이제 Graph이며 consumer가 선택된다. 한글·이모지·결합 문자·여러 줄 문자열·raw 문자열 다섯 가지 모두 고정했다. 독립 fixture에서도 기존 consumer의 >를 >=로 바꾼 같은 변이는 기준 판에서 테스트 실패, 상수 변경 뒤에는 생존했다. RUSTFLAGS="-D warnings"에서 다섯 경우 모두 [기준 0, 기준 변이 100, 변경 0, 변경 변이 0]을 확인했다. 판정은 그 생존을 선택에서 숨기지 않아야 한다. 위치 슬라이싱으로 이를 놓치던 78e5ad5의 추가 전용 판정은 잘못이었다.

회귀 이름은 unicode_changes_cannot_be_hidden_by_item_additions, unicode_exports_do_not_erase_changes_to_other_items, raw_and_plain_prelude_names_are_never_fresh, raw_definition_and_reference_spellings_share_root_owners, test_diff_raw_calls_and_new_names_share_shadow_identity, test_diff_raw_support_names_keep_the_same_support_key, external_aliases_and_fully_qualified_roots_are_not_modules, prelude_reexports_cannot_replace_implicit_names, existing_location_macros_disable_addition_shortcuts, blanket_impls_and_trait_objects_expand_uncertain_dispatch, trait_names_reach_their_implementation_files다.

리뷰에서 살아남은 다섯 조건은 import_origin_and_data_only_guards_are_observable, registration_import_origin_and_mixed_kinds_are_observable, only_exact_test_modules_may_extend_unchanged_headers에 분리된 반례를 더해 고정했다. 테스트 모듈은 경로가 cfg이고 payload가 정확히 test이고 속성이 하나여야 하며, visibility를 포함한 헤더 토큰이 같아야 한다. non_target_additions_expand_the_crate_without_other_checks, new_non_target_rust_files_expand_partial_crate_targets, existing_build_scripts_are_non_rust_execution_inputs는 다른 검사 없이 범위를 넓히는지 검증한다.

### 좁게 선택된 이력에서 빠진 변이 대조

D-20은 8da33c4c6a1515cc47cf4c56b8493fb1a48f6ccc → af70b46fc6c62a0d1a7b036c9977b3d2f3d47366, D-23은 5eec80d7e15b9b2645b94742eeba8a898f299a25 → 6e417f0796ab8f58cbcd96f335dec169312e6947이다. 실제 첫 부모를 각각 별도 git archive에 풀었다. 공통 seed 0x483433으로 우선 공통 파일 20개에서 각 5개를 뽑고, seed 0x483434로 빠진 나머지 모든 생성 가능 파일의 표본을 보충했다. 변이가 5개 미만이면 전수 검사했다. 선택 파일을 다시 변이한 수치가 아니다.

각 판에서 cargo mutants 27.1.0 --no-config --gitignore=false --manifest-path crates/domain/Cargo.toml --list의 동일한 이름을 무작위로 선택했고, --file과 --re를 함께 사용해 실제 열거 집합을 확인했다. --re만 사용하면 새 투자 프로필의 Self 필드 삭제 변이 세 개가 추가로 열거되는 동작을 관측했으므로 파일 교집합도 확인했다. 각 실행은 --test-tool=nextest --cap-lints=true --jobs 2 --timeout-multiplier 30 --no-shuffle이다. 각 배치의 변이 없는 baseline도 성공했다. 샤드·baseline skip·iterate·시간 초과 통과는 쓰지 않았다.

| 이력 | 좁은 선택 | 빠진 파일 중 변이 생성 파일 | 표본 | 기준/변경 검출 | 기준/변경 컴파일 불가 | 결과 차이 | 새 생존 | 시간(기준/변경) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| D20 | 4파일 | 45 | 217 | 107/107 | 110/110 | 0 | 0 | 485.75s/491.99s |
| D23 | 1파일 | 49 | 236 | 114/114 | 122/122 | 0 | 0 | 953.12s/944.39s |

D-20의 빠진 47파일 중 45파일, D-23의 빠진 51파일 중 49파일에서 변이가 생성됐다. lib.rs와 thesis.rs에는 변이가 0개여서 실행 표본을 만들 수 없다. proposal_id는 1개, ratio는 4개, volatility_display는 2개이며 D-23의 horizon도 4개뿐이어서 모두 전수 실행했다. 나머지 파일은 각각 5개 이상이다. 컴파일 불가 110/122개를 검출 성공으로 합치지 않았다. 실제 테스트로 검출된 표본은 각 107/114개이며, 전체 표본의 상태가 판 사이에 모두 같았다. 이 결과는 표본과 고정된 두 이력에 대한 증거이며 모든 미래 추가의 증명은 아니다.

| 빠진 파일(src/ 아래) | D20 표본/검출/컴파일 불가(기준=변경) | D23 표본/검출/컴파일 불가(기준=변경) | 새 생존 |
|---|---:|---:|---:|
| account_id.rs | 5/4/1 | 5/5/0 | 0 |
| candle.rs | 5/1/4 | 5/1/4 | 0 |
| client_order_id.rs | 5/5/0 | 5/5/0 | 0 |
| cluster.rs | 5/4/1 | 5/3/2 | 0 |
| collected_item.rs | 5/3/2 | 5/0/5 | 0 |
| disclosure.rs | 5/5/0 | 5/4/1 | 0 |
| domain_time.rs | 5/0/5 | 5/0/5 | 0 |
| exact_arithmetic.rs | 5/2/3 | 5/2/3 | 0 |
| exclusions.rs | 선택 파일 | 5/3/2 | 0 |
| fx_conversion.rs | 5/0/5 | 5/0/5 | 0 |
| fx_rate.rs | 5/1/4 | 5/1/4 | 0 |
| holding.rs | 5/1/4 | 5/1/4 | 0 |
| holding_valuation.rs | 5/1/4 | 5/0/5 | 0 |
| horizon.rs | 선택 파일 | 4/3/1 | 0 |
| instrument_id.rs | 5/4/1 | 5/4/1 | 0 |
| investor_profile.rs | 선택 파일 | 5/5/0 | 0 |
| market_session.rs | 5/5/0 | 5/5/0 | 0 |
| money.rs | 5/2/3 | 5/1/4 | 0 |
| money_display.rs | 5/1/4 | 5/3/2 | 0 |
| news_item.rs | 5/5/0 | 5/4/1 | 0 |
| numeric_input.rs | 5/3/2 | 5/3/2 | 0 |
| order_amount.rs | 5/3/2 | 5/3/2 | 0 |
| order_intent.rs | 5/0/5 | 5/0/5 | 0 |
| order_price.rs | 5/4/1 | 5/4/1 | 0 |
| order_quantity.rs | 5/2/3 | 5/0/5 | 0 |
| portfolio.rs | 5/1/4 | 5/0/5 | 0 |
| portfolio_valuation.rs | 5/0/5 | 5/0/5 | 0 |
| price.rs | 5/1/4 | 5/1/4 | 0 |
| probability.rs | 5/3/2 | 5/4/1 | 0 |
| proposal.rs | 5/3/2 | 5/3/2 | 0 |
| proposal_facts.rs | 5/2/3 | 5/4/1 | 0 |
| proposal_id.rs | 1/0/1 | 1/0/1 | 0 |
| proposal_range.rs | 5/2/3 | 5/2/3 | 0 |
| quantity.rs | 5/4/1 | 5/3/2 | 0 |
| quote.rs | 5/2/3 | 5/2/3 | 0 |
| ratio.rs | 4/1/3 | 4/1/3 | 0 |
| settled_row_set.rs | 5/3/2 | 5/3/2 | 0 |
| symbol.rs | 5/5/0 | 5/5/0 | 0 |
| thesis.rs | 5/4/1 | 5/4/1 | 0 |
| thesis/evidence.rs | 5/4/1 | 5/4/1 | 0 |
| thesis/model.rs | 5/2/3 | 5/2/3 | 0 |
| transaction.rs | 5/3/2 | 5/3/2 | 0 |
| transaction_kind.rs | 5/3/2 | 5/3/2 | 0 |
| transaction_order.rs | 5/1/4 | 5/1/4 | 0 |
| transaction_response.rs | 5/1/4 | 5/1/4 | 0 |
| volatility.rs | 5/4/1 | 5/2/3 | 0 |
| volatility_arithmetic.rs | 5/0/5 | 5/0/5 | 0 |
| volatility_display.rs | 2/2/0 | 2/2/0 | 0 |
| weight_caps.rs | 선택 파일 | 5/4/1 | 0 |

표본 원본, 명령, outcome JSON, 파일별 비교는 target/h43-review/sample-selection.json, D20-extra-selection.json, D23-extra-selection.json, sample-comparison.json, sample-*/mutants.out, extra-*/mutants.out에 남겼다. 총 변이 실행은 두 이력의 217/236개를 각 두 판에서 실행한 906개다. 실제 Cargo-mutants의 caught/unviable/missed/timeout 상태를 이름으로 맞춰 비교했다.

### 실제 cargo-mutants를 호출하는 기존 테스트의 실패 조사

weak_test_example_leaves_surviving_mutants는 수정 전 전체 검사에서 16.599초에 통과했고, 수정 후 관련 검사에서도 7.868초·11.672초·8.908초에 통과했다. 78e5ad5를 별도 archive에서 빌드한 원본 테스트도 단독 실행 성공(빌드 포함 49.99초), 같은 작업 사본에서 두 nextest 명령을 겹친 대조도 모두 성공(12.34/23.76초)이었다. 부하를 추가한 현재 표본 실험 중에도 시간 초과를 재현하지 못했다. 리뷰어의 실패 원본 로그가 없어 부하인지 기존 fixture 공유인지 변경 때문인지 단정할 수 없다. 기존 master 테스트와 생존 기대값은 바꾸지 않았다. weak-diagnosis.json과 weak-baseline-*.log에 기록했다.

### 남는 구멍과 운영

확장 그래프와 추가 전용 판정은 Rust 전체의 이름 해석·I/O·빌드 환경을 증명하는 분석기가 아니다. crate 밖 testkit/설정 파일이나 환경이 domain의 기존 테스트 의미를 바꾸는 경우, 새로운 외부 매크로 허용 표의 오류, 컴파일러·툴체인 변경은 별도 검토가 필요하다. 알려진 입력 경로를 제외한 임의의 외부 입력은 이 판정만으로 불변임을 증명하지 못한다. 그런 변경을 추가 전용의 근거로 승인하면 안 된다. blanket/dyn, 위치 의존, 비ASCII 이름과 불투명 입력은 일부 안전한 변경까지 전체 또는 그래프로 넓혀 성능 절감이 작아질 수 있다. 밤 전체 검사와 실패 이슈의 다음 병합 전 해결 규칙을 유지한다.

### 최종 분류와 손 변형

외부 prelude 수집에서 비-lib 파일의 명시적인 하위 모듈 재수출도 내부 이름으로 해석하도록 바로잡았다. thesis.rs의 `mod evidence; pub use evidence::Evidence`를 외부 crate evidence로 오인하면 calibration의 새 테스트 도우미 evidence를 잘못 거부한다. 선언된 자식 모듈이라는 구조적 근거가 있는 현재 파일 최상위 import만 내부로 인정하며, 그 밖의 불명확한 이름은 여전히 거부한다. declared_child_module_imports_are_not_external_preludes가 이를 고정한다. 최종 코드로 실제 여섯 이력을 다시 분류했고 D-20은 4파일, D-23은 1파일을 유지했다(final-history-scope2.log). 두 이력의 기존 테스트 변경 검사도 위반 0이었다(D20-test-changes.log, D23-test-changes.log).

최종 코드에서 손 변형 42개 모두 컴파일 후 테스트 실패(종료 100)로 검출했다. 리뷰에서 살아남은 내부 import, Data만 허용, cfg 경로/payload, 헤더, 속성 개수 조건을 포함한다. AST 동일성, 공개 use 제거, raw 이름 정의/참조/파일 경로, 전체 prelude/표준 루트, 외부 crate·별칭·원래 이름·경로, 위치 의존, 대상 밖 입력, blanket/dyn, trait 구현 소유, test-diff 키·참조·연결, 비ASCII 불확실성, 혼합 합집합, 기준 그래프, 외향 폐쇄도 각각 변형했다. 컴파일 오류를 검출 성공으로 세지 않았다. trait 이름 색인의 경우 다른 경로 식별자만으로 구현에 도달하는 약한 예제를 발견하여 trait을 별도 파일에 두고 super 경로로 사용하는 회귀로 교체한 뒤 재검출했다. 표본의 기대값을 구현 출력에서 복사하지 않았다. 원본은 target/h43-review/manual.json과 manual-*.log다.

로컬 master의 진행분 f9f0c5f0b7dff04ad4a184aadf882f4af1fee376을 끝에 병합했다. 생성물 .codex/config.toml·harness/claude-hooks/hashes.json의 충돌은 기존 쓰기 스크립트로 재생성했다. 기존 master 테스트 본문을 수정하지 않았다. 최종 커밋은 일반 커밋 훅의 fresh-build·commit-guard·전체 check를 통과해야 반영한다.


### master 4c9428d 병합 후 고정 fixture와 실제 소스 대조

로컬 master 4c9428d790553d1d253c27e962b949bc8b229814(D-27·D-31·T-01)을 병합한 상태에서 기존 actual_portfolio_boundary_deletion_reaches_exact_arithmetic_through_graph의 panic을 재현했다. 경계 테스트 행 네 개의 원문은 그대로였다. 새 valuation_snapshot.rs:2537의 `#[should_panic(expected = "확정")]`가 기존 매크로/속성 표에 없어서 Graph::read가 `glob or macro may hide an incoming reference`를 반환했다. 스코프 선택기는 이를 crate 전체로 넓히지만 테스트는 실제 domain에서 fallback이 없다고 단정하고 있었다. 진단용 사본에서 이 속성 하나만 제거하면 Graph::read가 성공하고 portfolio 외향 폐쇄는 81파일이며 exact_arithmetic을 포함했다. 속성 허용 표나 실제 domain 코드는 바꾸지 않았다.

회귀 입력은 테스트 안의 고정된 5파일 Sources(lib, portfolio_valuation, holding_valuation, exact_arithmetic, unrelated)로 옮겼다. portfolio의 네 경계 호출은 assert_eq! 토큰 안에만 있고 루트 재수출을 거쳐 holding에서 exact로 이어진다. 경계 호출을 지운 뒤의 그래프만으로는 exact에 도달하지 못하며 기준 그래프를 합치면 도달해야 한다. 무관한 파일과 lib가 외향 폐쇄에 포함되지 않는 것도 단언하여 전체 fallback으로 통과할 수 없게 했다. 두 번째 고정 회귀는 holding의 exact 호출을 내용이 같은 지역 함수로 교체하여 기준 판 간선이 필요한 (b)도 검증한다. 기존 scope_tests의 (a)(b)는 변경하지 않았다.

mutants_check 아래에서 실제 제품 crate의 소스를 그래프 입력으로 읽는 도우미는 domain_sources 한 곳이었다. 이를 제거했고 나머지 범위/추가 전용/매크로/리뷰 회귀는 문자열 Sources 또는 독립 임시 저장소를 사용한다. environment.rs 호출 개수를 검사하는 기존 테스트는 허용 목록의 실제 사용을 제한하는 정책 검사이며 그래프 입력 fixture가 아니다. 이 검사를 고정 문자열로 바꿔 실제 경계 검사를 잃지는 않았다. justfile·변이 허용 목록·하네스 예제에 대한 정책 검사도 유지했다.

병합된 실제 domain 83파일에서 다음 대조를 일회성으로 실행했다. 작업 사본에서 한 파일씩 임시 변경하고 실제 diff_scope에 master 전체 SHA를 기준으로 전달한 뒤 원복했다. 감사용 테스트 코드는 커밋에 남기지 않는다.

| 재현 | 실제 임시 변경 | 선택 결과 | exact_arithmetic 포함 |
|---|---|---|---|
| (a) | portfolio 경계 행 1.01/0.99/-1.01/-0.01 삭제 | 미등록 should_panic으로 domain 전체 글롭, 83/83파일 | 예 |
| (b) | holding compare_money의 exact 호출을 같은 본문의 local_compare_reported 호출로 대체 | 같은 보수적 fallback, 83/83파일 | 예 |

따라서 현재 판은 넓게 선택하며 두 공격의 exact 파일을 놓치지 않았다. 현재 실제 domain이 좁은 그래프로 선택된다는 결과로 해석하지 않는다. 제품 소스의 임시 변경은 모두 원복하여 master와 domain 차이가 0임을 확인했다. 재현·원인 분리·선택 로그는 target/h43-merge-fix/manual-domain.log다.

검사 중 master가 A-01과 DS-03을 포함한 503261f48fe530378274307dfda375c0eba2b9f4로 전진하여 최종 통합 기준은 그 판으로 고정했다. 4c9428d와 503261f 사이 domain 변경은 0이므로 위 실제 소스 대조의 입력은 동일하다. 4c9428d 병합 상태의 전체 check는 881개 테스트와 커버리지·test-diff·훅 설치 확인까지 통과했으며, 최종 병합 상태는 일반 커밋 훅에서 다시 전체 검사한다.
