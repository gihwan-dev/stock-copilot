# H-45: 테스트 속성과 실제 crate의 추가 전용 변이 범위
- 상태: 초안

## 결정

ADR 0054의 추가 전용 규칙을 실제 domain에서도 적용하도록 그래프 입력과 위치 의존 판정을 고친다. 변경 전·후 그래프의 합집합, 외향 전이 폐쇄·내향 직접 참조, 해석 실패 시 전체 확장, 기존 테스트 보호, 생존·시간 초과 정책은 유지한다.

### A: 그래프와 통합 테스트의 경계

snapshot은 crate 전체 Rust 파일을 보존한다. Graph::read에 전달할 때만 정확히 crate/src/ 아래로 한정한다. 통합 테스트는 별도 crate여서 src가 직접 참조할 수 없다. #[path]가 우회하면 기존 그래프 해석 실패로 전체 확장하고 include!는 기존 source-check가 금지한다. 두 전제는 새 고정 회귀로 검사한다.

Additions의 prior_ids와 Symbols 입력에는 tests/를 계속 넣는다. 통합 테스트가 사용하던 이름을 새 공개 이름으로 가리는 등록은 여전히 거부한다. tests/·examples/·benches/·build.rs·Cargo.toml·지원 자료가 변경되면 그래프 계산 전에 기존 규칙대로 전체 확장한다. src 내부의 mod.rs·미지 속성·해석 불가 매크로는 종전대로 전체 확장한다.

### B: 위치 의존의 의미

기존 소스의 모든 토큰 그룹을 재귀적으로 읽는다. line!·file!·module_path!·column!은 식별자 뒤 !와 토큰 그룹이 실제로 있을 때만 위치 의존이다. 일반 변수·클로저 매개변수 line, file, module_path와 line != 0은 위치 의존이 아니다.

Location의 한정 경로와 track_caller 속성, 위치 매크로·Location import 및 재내보내기를 감지한다. import의 원래 이름을 읽으므로 별칭도 포함한다. std/core 자체 별칭, panic 모듈 별칭 및 표준 glob import는 보수적으로 위치 의존으로 본다. 다른 파일의 alias chain도 최초 위치 import가 기존 소스에 남으므로 감지한다. 매크로 안의 매크로 호출과 macro_rules 본문도 토큰 재귀로 읽는다. use 토큰을 완전히 해석하지 못하면 위치 의존으로 판정한다.

이 판정은 정밀한 Rust 이름 해석기가 아니다. 다른 namespace의 Location 경로, 동명 항목 import 등 확신할 수 없는 입력은 과선택할 수 있다. 알 수 없는 매크로·속성은 별도의 기존 그래프 규칙도 전체 확장한다.

### 속성

should_panic은 인자 없음 또는 괄호 안 expected = 문자열 리터럴 하나만 허용한다. ignore는 인자 없음 또는 = 문자열 리터럴만 허용한다. 일반·raw 문자열 모두 접미사가 없어야 한다. 문자열은 패닉 메시지 부분 비교나 무시 이유 자료이며 앱 참조를 생성하지 않는다. 근거는 [Rust Reference](https://doc.rust-lang.org/reference/attributes/testing.html)다.

경로·매크로·비문자열·괄호 식·알 수 없는 키·중복 키·뒤 쉼표·빈 목록·다른 구분자·한정 속성 경로·접미사 문자열은 보수적으로 전체 확장한다. 제품 소스의 ignore 금지는 유지한다. 새 파일 내부의 속성과 기존 테스트 모듈에 추가한 새 테스트의 속성도 고정 입력으로 검사한다.

### application 대상과 검출력의 한계

실제 파일이 없는 application/order_gate.rs·order_gate/**의 대상을 application/src/**/*.rs로 바꾼다. 현재 lib.rs·observations.rs·port_error.rs·error_class.rs와 앞으로 추가할 유스케이스·포트가 자동 포함된다. 기존 테스트 변경은 메인이 승인한 대상 기대 목록·상수 값 변경만 한다. 네 조각과 CI·justfile 실행 방식은 유지한다.

error_class.rs의 현재 생성 변이 2개는 Default::default()로 본문을 대체하며 모두 ErrorClass: Default 미구현(E0277)으로 unviable이다. 따라서 오류별 매핑 검증은 crates/application/tests/error_class.rs의 exhaustive expected_class·원인별 기대 함수와 every_leaf_matches_the_decision_table에 의존한다. 자동 변이에서 missed 0이라는 결과가 매핑 검출력을 증명하지 않는다.

application 전체의 실제 실행은 총 13개 중 caught 1·unviable 12·missed 0·timeout 0이다. 새로 포함된 11개는 caught 1·unviable 10이다. 잡힌 변이는 HoldingsObservation::holdings가 빈 슬라이스를 돌려주도록 바꾼 것이다. 미검출이 없어 제품 테스트를 추가하지 않았다. 기존 제품 테스트 본문과 생존 허용 목록은 바꾸지 않았다.

## 회귀와 우회 입력

실제 제품 소스를 읽는 영구 그래프 테스트는 없으므로 새 회귀는 고정 Sources를 쓴다. tests/x/mod.rs와 |line| 클로저가 있는 crate에 should_panic 테스트를 가진 새 파일과 mod 등록을 추가하면 새 파일만 선택한다. 실제 최신 domain을 읽은 별도 일회성 실측도 같은 결과를 단언한다.

A의 전체 확장을 유지해야 하는 입력 10개는 tests/x/mod.rs·tests/other.rs·build.rs·tests/renamed.rs·tests/deleted.rs·tests/nested/data.rs·examples/demo.rs·benches/speed.rs·Cargo.toml·settings.json 변경이다. 추가로 통합 테스트의 기존 이름을 새 공개 타입으로 가리는 등록, src의 #[path]·include! 우회를 검사한다.

B의 새 테스트는 line/file/module_path/column 호출, 표준 한정 호출, 중첩 column!, macro alias import, core 재내보내기, group import, Location 타입·panic 모듈·std 자체 별칭, panic self 별칭, raw line 별칭, 표준 glob, Location::caller, track_caller, macro_rules 본문, 본문 안 alias import, 해석 불가 use 메타변수, alias chain을 각각 위치 의존으로 직접 단언하고 추가 전용 판정을 막는다. 다른 파일의 재내보내기도 별도로 검사한다.

weak_test_example_leaves_surviving_mutants는 이 작업 사본의 관련 테스트 실행에서 통과했다. 리뷰어 임시 사본의 실패 원인은 재현되지 않아 확정하지 않았다. 해당 기존 테스트는 수정하지 않았다.

## 미해결과 유지한 경계

원인 A(통합 테스트 mod.rs 유입)와 B(일반 line 식별자 오인)는 해결했다. 반면 src 내부의 모듈·매크로 불확실성과 실제 위치 의존·이름 가리기·기존 impl 변경 등 기존 보수적 규칙은 유지한다. 모든 변경이 추가 전용이라는 주장을 하지 않는다.

새 의존성·새 검사 끄기·제품 소스 변경은 없다. docs/tasks.md·docs/progress.md·H-44 작업 파일은 수정하지 않는다.

## 실제 범위와 분할 실측

로컬 master는 257bd4c94999034745f75fb7df2c1fb545138919이며 git merge --no-ff --no-edit master는 이미 포함된 상태로 끝났다. 실제 domain의 이 판에 새 h45_scope_probe.rs(should_panic 테스트 포함)와 lib 끝의 mod 한 줄만 추가한 입력을 선택기에 주어 Ok({새 파일})를 확인했다. 영구 회귀는 위 고정 입력을 사용한다.

세 변경의 기준은 각 head와 위 master의 실제 merge-base다. 선택기 소스 사본으로 변경 전·후 git 객체를 읽고, 각 head의 git archive에서 cargo mutants --no-config --list --json으로 열거했다. 변이 본체 실행 수가 아니다.

| 변경 head | 기준 | 수정 전 파일/변이 | 수정 후 파일/변이 |
|---|---|---|---|
| D-35 0e2d0ccfff2aef4ba314bb646d920857a6a74a76 | 856cad43913e3275e835f9c2151d801c32d7cc29 | 4 / 71 | 4 / 71 |
| D-37 59bac7204792b0920bf3586402cbe856c50285d1 | 257bd4c94999034745f75fb7df2c1fb545138919 | 118 / 3762 | 117 / 3761 |
| D-29 506a953696530563f280b1c6e8096eb70a87745b | 856cad43913e3275e835f9c2151d801c32d7cc29 | 113 / 3631 | 113 / 3631 |

D-35는 이전 판에도 좁았고 그대로 유지한다. D-37은 mod.rs 전체 fallback이 해소되고 lib 등록도 Registration이 됐으나 investor_flow·market_index·market_ranking의 테스트 도우미 num이 기존 예약 이름과 충돌해 Graph로 판정되므로 넓게 남는다. D-29는 bootstrap의 Cluster·sample, signal_return_contract의 task·candle, signal_return_scenarios의 task·sample이 기존 예약 이름과 충돌해 Graph로 판정되며 나머지 추가 항목과 혼합 선택된다. 이 예약 집합은 타입·외부 경로·별칭의 가림을 막는 기존 과대 근사다. 이 별도 보수성은 A·B 수정 범위에서 약하게 바꾸지 않는다.

현재 공통 설정은 전체 3546개, 네 조각 887·887·887·885개이며 합집합이 전체와 같고 서로 겹치지 않는다. application 13개는 0/4에 들어간다. 실측·변이·반례 로그는 작업 사본 target/h45-review에 둔다.

최종 손 변형 15개는 실제 xtask 소스에서 그래프 src 필터·기준/현재 필터·기존 이름 보존·위치 탐지·호출 구분·토큰 재귀·import·column·Location·track_caller·별칭 원래 이름·미해석 use·문자열 접미사 검사를 제거하거나 넓혀 실행했다. 모두 컴파일 후 새 회귀 테스트 실패로 검출했고 원복 SHA-256을 확인했다. 실험에는 cargo-mutants와 같은 cap-lints를 사용했으며 커밋된 검사 끄기는 추가하지 않았다.
