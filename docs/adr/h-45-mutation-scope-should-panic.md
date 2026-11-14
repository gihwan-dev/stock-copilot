# H-45: 테스트 속성 해석과 오류 분류의 변이 범위
- 상태: 초안

## 결정

ADR 0054의 닫힌 속성 표에 아래 형태만 추가한다. 기준·현재 Graph::read가 성공해야 추가 전용 판정으로 진행하는 순서는 유지한다.

| 속성 형태 | 안전 논증 |
|---|---|
| should_panic | 테스트 실행기의 패닉 기대 여부를 표시한다. 다른 앱 항목을 새로 참조하지 않는다 |
| should_panic(expected = "문자열") | 단일 expected 키와 문자열 리터럴만 허용한다. 문자열은 패닉 메시지 부분 비교용 자료이며 코드 경로로 해석되지 않는다 |
| ignore | 테스트 실행기의 기본 실행 여부를 표시하며 참조를 주입하지 않는다 |
| ignore = "문자열" | 문자열은 무시 이유 자료다. 다른 앱 항목을 참조하지 않는다 |

일반 문자열과 raw 문자열을 허용한다. should_panic 목록은 괄호만 허용하고 MetaNameValue 하나를 완전히 파싱한다. 키는 expected 하나이며 값은 Expr::Lit의 Lit::Str만 허용한다. ignore의 목록형은 허용하지 않는다. 매크로 호출·경로·괄호 식·바이트 문자열·숫자·불린·알 수 없는 키·중복 키·뒤 쉼표·빈 목록·대괄호/중괄호·한정 경로는 보수적으로 전체 범위로 돌린다. 표준 의미보다 좁은 허용이다.

이 판정은 참조 그래프를 만들 수 있는지에 관한 것이다. 테스트의 실행 여부를 승인하지 않는다. source-check의 ignore 금지와 기존 테스트 변경 금지는 유지한다. 새 테스트 속성 외의 기존 허용 표는 넓히지 않는다.

## 바꾼 것과 유지한 것

.cargo/mutants.toml의 공통 대상에 application/src/error_class.rs를 추가한다. ADR 0063에서 인계한 포트 오류 분류는 사용자 조치·대기·버그·데이터 없음과 재시도 대기 자료를 결정한다. 주문 게이트만 검사해서는 이 분류의 오류를 탐지하지 못하므로 변경분과 밤 전체 변이에 모두 포함한다. 동일 목록을 사용하는 0/4~3/4 분할은 자동으로 새 파일의 변이를 나누므로 CI나 justfile 변경은 필요 없다.

고정 대상 목록을 검증하는 기존 테스트의 상수에 새 파일을 추가한다. 메인이 승인한 범위대로 any_change_in_a_target_crate_mutates_its_whole_scope의 기대 목록에도 같은 파일 한 항목만 추가한다. 그 밖의 기존 본문과 단언은 유지한다. 회귀는 실제 domain을 읽지 않는 고정 Sources 입력에 새 함수로 추가한다. 제품 코드·제품 테스트·승인된 대상 목록 외 기존 mutants_check 테스트 본문·생존 허용 목록·조각 수·시간 제한·jobs는 바꾸지 않는다. docs/tasks.md·docs/progress.md와 H-44 파일은 수정하지 않는다.

## ADR 0054와의 관계

0054의 미등록 should_panic 때문에 전체 fallback이 발생한다는 미해결을 닫는다. 참조 토큰 수집·전후 그래프 합집합·추가 전용 검증·불확실성 fallback은 그대로다. 모든 domain 변경이 좁아진다고 주장하지 않는다. 기존 항목 변경과 불확실한 입력은 여전히 넓게 선택될 수 있다.

## 검증

고정 fixture에서 허용 여섯 형태는 lib 등록과 새 파일 추가만 선택하며, 비리터럴·알 수 없는 키·중복·잘못된 구분자 등은 select_sources 오류로 crate fallback을 요구한다. 별도 실측과 손 변형의 명령·결과는 작업 사본 target/h45에 둔다.

## 미해결

표준 속성 전체나 허용하지 않은 구문의 정밀 해석은 다루지 않는다. 뒤 쉼표 등 추가 형태가 필요하면 안전 근거와 독립 회귀를 먼저 추가한다. ignore가 제품 소스에서 필요하다는 결정은 하지 않는다.


## 실측과 검증 결과

Rust Reference의 테스트 속성 설명에 따르면 should_panic의 expected 문자열은 패닉 메시지의 부분 문자열 비교 자료이며 ignore의 문자열은 무시 이유다: https://doc.rust-lang.org/reference/attributes/testing.html . 허용 표는 요청된 형태만 수용하며 표준의 다른 형태를 자동으로 추가하지 않는다.

시작 기준 856cad43913e3275e835f9c2151d801c32d7cc29의 전체 check는 1,058개 테스트와 커버리지 실행 722개, coverage-check 및 test-diff를 통과했다. 이후 master 257bd4c94999034745f75fb7df2c1fb545138919를 정상 병합 훅으로 받았다.

다음 값은 각 병합의 첫 부모와 해당 병합 판 사이 domain 변경을 실제 git 객체에서 읽고, 동일한 scope/graph/additions 소스의 select_sources를 실행한 뒤 해당 판의 git archive에서 cargo mutants --no-config --list --json으로 열거한 결과다. 변이 본체를 실행한 수가 아니다. 일회성 계산기는 target/h45/audit에 있고 제품 소스를 변경하지 않는다.

| 변경 | 기준 첫 부모 | 수정 전 파일/변이 | 수정 후 파일/변이 |
|---|---|---|---|
| D-17 09583ca386f869a5c7bdbb6a20e6d920ef98bd22 | 792a35b5da51e9a8fff146793a172d1c2e210fc1 | 90 / 2860 | 89 / 2852 |
| D-25 c1b3a5992a3047649d31f56bae8539d1c62f1287 | 6a33bc1b1ec4ef9dd055b60f10c0f93aba350a20 | 99 / 3040 | 99 / 3040 |
| D-36 6e966736fd33f1aa9143c595835016be0fbb5dd7 | c1b3a5992a3047649d31f56bae8539d1c62f1287 | 102 / 3066 | 102 / 3066 |
| D-18 257bd4c94999034745f75fb7df2c1fb545138919 | edd57f84fc68b66a417291424006f6f463b67938 | 114 / 3533 | 114 / 3533 |

D-17・D-25・D-36은 수정 후 그래프 읽기에 성공한다. D-25와 D-36은 기존 항목 수정과 추가 전용 판정 실패 때문에 그래프 범위가 넓게 유지된다. D-36에서 새 세 파일과 lib 등록만 분리한 진단 입력도 broker_account.rs와 cash_observation.rs가 Graph, cash_observation_failure.rs만 OnlyFile, lib.rs는 Registration으로 판정됐다. 이 진단 입력은 실제 전체 diff와 구분한다.

최신 D-18은 D-34에서 추가된 tests/order_gate_support/mod.rs 때문에 Graph::read의 기존 mod.rs 확장 규칙에 걸린다. should_panic 문제는 고쳤지만 최신 domain의 모든 추가 전용 작업이 좁아졌다고 주장하지 않는다. 다른 보수적 규칙을 약하게 바꾸지 않았다.

새 속성 분기를 제거하거나 넓히는 손 변형 15개는 동일한 scope/graph/additions 소스와 새 고정 테스트 두 함수를 복사한 독립 scratch에서 모두 컴파일 후 테스트 실패로 검출했다. 원복 뒤 두 테스트가 통과했다. 분기 제거, bare/payload 거부, 임의 키/값/구분자 허용, name-value/list 과허용, 임의 리터럴/경로/매크로/괄호 식 허용을 각각 검사했다. target/h45/manual.json에 소스 SHA-256과 개별 결과를 기록했다. 기존 테스트 본문을 손 변형하지 않았다.

error_class.rs 단독 cargo-mutants 실행은 2개 모두 ErrorClass: Default 미구현(E0277)으로 unviable이었다. baseline은 통과했고 missed 0, timeout 0, caught 0이다. 두 변이는 ClassifiedError::class와 classify_port_error의 본문을 Default::default()로 대체한다. 테스트가 분기 변이를 잡았다는 증거로 해석하지 않는다. 제품 테스트나 생존 허용 목록을 추가하지 않았다.

남는 범위 문제는 최신 테스트 지원 mod.rs와 기존 추가 전용 검증의 보수성이다. 이 작업에서는 별도 그래프 정책 변경을 하지 않는다.
