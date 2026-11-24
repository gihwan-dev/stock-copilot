# ADR H-62 초안: 승인된 기존 테스트 변경의 승인 묶음(Approval Bundle) 정식 절차

- 상태: 제안
- 날짜: 2026-11-24
- 결정한 사람: 메인 에이전트(사용자 확인 대기)
- 관련 작업: H-62 (ADR 0153 후속)
- 반영한 문서: `crates/xtask/src/test_diff/**`, `harness/claude-hooks/edit-cases.json`, `justfile`, `docs/adr/h-62-test-diff-approval-bundles.md`

## 맥락

H-55·H-56(ADR 0153)은 `harness/test-diff/approvals.toml`의 승인 행으로 정적 구조 보존 규칙을 만족하는 기존 테스트 변경을 해소하도록 했다. 그러나 실제 `master` 병합에서 아래 네 사례가 구조 보존 규칙에 걸려 사람이 병합 커밋을 별도 검사 스레드로 35분씩 대조하고 훅을 우회해 올린 뒤 `ci`가 빨강으로 남는 문제가 반복됐다.

- **D-55 (`45342efc`, 30건)**: 테스트 준비 코드의 후보 생성·호출 인자 변경.
- **D-56 (`8a1b8bbc`, 76건)**: 포트 서명 고정 문자열, 반환 타입 이름(`RaisedSignal`→`HistoricalSignal`), 호출 인자 추가.
- **S-21 (`d29b9c8f`, 2건)**: `approvals.toml`에 정확한 전·후 SHA-256의 `assertion_change = true` 승인 행이 있는데도 보호 단언 문장에 제어 흐름(`for`·`?`·클로저)이 있어 거부됨(실제 변경은 고정 숫자 한 줄씩 7→8, 42→55).
- **D-57 (`a5ebb1d0`, 5건)**: 기대 오류 변형 이름 변경 2건, 준비 함수 필드 추가 3건.

판정기(`crates/xtask/src/test_diff/approval.rs`)의 구조 보존 규칙을 느슨하게 넓히면 제어 흐름 내부의 테스트 약화 우회 면적이 커진다. 따라서 판정기를 넓히지 않는 것을 기본으로 두고, **병합 커밋 한 구간에 한해 승인된 지적 문자열 집합과 파일 전·후 SHA-256이 정확히 일치할 때만 통과시키는 승인 묶음 도구**를 도입한다.

## 언제 승인 행, 언제 승인 묶음

| 변경 형태 | 사용 절차 | 비고 |
|---|---|---|
| 단순 단언 값·고정 목록 추가형(예: 열거형·허용 목록에 항목 추가, 구조 보존 규칙 통과) | `harness/test-diff/approvals.toml`의 `assertion_change = true` 행 | 기존 H-55 규칙 그대로 해소된다. 묶음을 만들지 않는다 |
| `impl` 지원 코드 변경 등 구조 보존 규칙을 통과하는 소규모 변경 | `harness/test-diff/approvals.toml` 승인 행 | 파일 해시·지적 일치·구조 보존 검증 모두 유지 |
| `match`·`if let`·`matches!`·`let … else` 안 타입·변형 이름 변경, 단언 안 튜플 변형 생성자 변경 | `harness/test-diff/bundles/<작업 ID>.toml` 승인 묶음 | 판정기를 넓히지 않고 병합 구간 정확 일치로 해소 |
| 보호 단언 문장에 `for`·`?`·클로저 등 제어 흐름이 있어 `approvals.toml`이 거부하는 경우(S-21) | `harness/test-diff/bundles/<작업 ID>.toml` 승인 묶음 | `approval.rs` 구조 보존 규칙은 그대로 유지(S-21 재현 테스트로 고정) |
| 포트 서명·반환 타입·준비 함수 인자 변경으로 다수 테스트가 함께 바뀌는 경우(D-55·D-56·D-57) | `harness/test-diff/bundles/<작업 ID>.toml` 승인 묶음 | 지적 문자열 전체 목록을 묶음에 고정해 누락·추가를 차단 |
| 기존 `#[test]` 삭제(`removed`), 목록 누락(`not-listed`), 테스트 수 감소(`count-decreased`), 도달 불가(`unreachable`), 가림(`shadowing`) | **허용 불가** | 승인 행·승인 묶음 모두 거부(닫힌 실패) |

## 실측

실제 `master` 이력의 4개 병합 구간 전·후 소스를 얕은 복제 환경에서도 재현할 수 있는 고정 자료(`harness/test-diff/fixtures/h62-{d55,d56,s21,d57}/`)와 실제 저장소 커밋에서 검증했다.

| 항목 | 명령 | 결과 |
|---|---|---|
| 4개 실제 사례 재현 + 거부 경계 18건 | `cargo nextest run --locked -p xtask test_diff::h62_tests` | 18 passed, 0 failed (D-55 30건, D-56 76건, S-21 2건, D-57 5건 정확 일치 통과 및 변조·재사용·비병합·미신뢰·트레일러 누락·미푸시 묶음 거부 확인) |
| 실제 저장소 D-57 초안 생성 | `cargo run --locked --quiet --package xtask -- test-diff-bundle draft D-57 4c13d306ed6190f2fbd970822a92ddf88f1a7529 a5ebb1d03fd351bc3fce6f2177508636d02170d8` | 표준 출력으로 정규 TOML 출력(지적 5건, 파일 2건 전·후 SHA-256), 작업 트리 무변경 |
| 보호 경로 검증 | `node --test harness/claude-hooks/tests/h62-bundle-protection.test.mjs harness/claude-hooks/tests/edit-guard.test.mjs` | 통과 (`harness/test-diff/bundles/**`가 `approvals.toml`과 동일하게 `agent-writable.txt` 밖 `protected-path`·샌드박스·Codex·편집 가드·merge-gate·서명 검사로 보호됨) |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| `harness/test-diff/bundles/<작업 ID>.toml`은 `harness/test-diff/approvals.toml`과 완전히 동일한 보호 계층(`harness/agent-writable.txt` 밖 `protected-path`, 샌드박스 `denyWrite`, Codex `"harness" = "read"`, `merge-gate`, `signatures.yml` 서명 검사)으로 보호한다 | 구현 담당·리뷰어는 물론 잠금 상태(`lock=on`)의 메인 세션까지 동일 규칙으로 보호하고 `approvals.toml`과의 보호 비대칭을 없앤다 | `rules.json`의 `paths.mainOnly`에 올려 `lock=on`에서도 메인 쓰기를 우회 허용하기, 또는 작업 브랜치에 묶음 두기 | H-62 메인 결정 1·3, 1회차 리뷰 지적 5 |
| 묶음은 `task`, `base_commit`(40자리), 대상 지정자(`merge_head` \| `target_commit` \| `target_tree` 중 정확히 하나), `reason`, `findings`(전체 지적 문자열 목록), `[[file]]`(`path`, `before_sha256`, `after_sha256`)로 고정하고 정확 일치(all-or-nothing)로만 해소한다 | 지적 하나라도 더 있거나 빠지거나 파일 SHA-256이 1바이트라도 다르면 전부 거부해 부분 승인·누락을 막는다 | 부분 해소(일부 지적만 제거하고 나머지 남김) | H-62 메인 결정 1 |
| 묶음은 해당 병합 커밋의 첫 부모 → 병합 커밋 한 구간(또는 진행 중 `MERGE_HEAD` 병합)에만 유효하다. 비병합 커밋·후속 커밋·`master`가 진행된 뒤 병합에서는 적용하지 않는다 | 묶음이 다른 구간이나 후속 커밋의 무단 테스트 변경에 재사용되는 경로를 차단한다 | `base_commit`의 모든 자손 커밋에 묶음 허용 | H-62 메인 결정 2 |
| 신뢰 검증은 `approvals.toml`과 같은 방식(`refs/remotes/origin/master`의 `trusted_authority`에서 읽고, 얕은 저장소 거부, 단일 부모 직계 이력만 허용, 묶음 도입 커밋에 테스트 파일 변경 금지, `merge_head` 모드는 도입 커밋이 해당 묶음 파일 하나만 변경)을 쓴다 | 작업 트리나 미푸시 로컬 커밋·작업 브랜치에서 위조한 묶음이 `test-diff`를 통과하지 못하게 한다 | 작업 트리나 로컬 미푸시 `master`의 묶음 파일을 그대로 신뢰 | H-62 메인 결정 3, 1회차 리뷰 지적 2 |
| `merge_head` 모드의 커밋된 병합 커밋은 트레일러 `Test-Diff-Bundle: harness/test-diff/bundles/<작업 ID>.toml`과 `Test-Diff-Findings: <건수>`를 **필수**로 요구한다(진행 중 `MERGE_HEAD` 병합은 아직 커밋 메시지가 없으므로 커밋 후 훅·CI·`test-diff-bundle check`에서 검증). 과거 불변 커밋 사후 검증용 `target_commit`·`target_tree` 모드는 두 트레일러가 모두 없거나 모두 정확히 일치할 때만 허용하며 `git show` 실패는 닫힌 실패로 거부한다 | 감사 이력을 git 로그에 강제하고 의도치 않은 묶음 매칭·트레일러 누락·도구 실패 시 열린 통과를 원천 차단한다 | 트레일러가 없으면 통과시키기 | H-62 메인 결정 4, 1회차 리뷰 지적 1 |
| `approval.rs`의 구조 보존 규칙은 S-21 등 기존 거부 케이스에 대해 넓히지 않는다 | 제어 흐름 안의 단언 약화 우회를 막는다 | `for`/`?`/클로저 내부 리터럴 변경 허용 | H-62 메인 결정 4·ADR 0153 |
| 메인용 보조 명령 3종(`just test-diff-bundle-draft`, `just test-diff-bundle-check`, `just test-diff-bundle-message`)을 제공하고 `draft`는 파일을 직접 쓰지 않고 표준 출력으로만 TOML을 낸다 | 메인만 승인 묶음을 검토·저장하는 경계를 지키면서 수작업 35분 대조를 도구로 대체한다 | 도구가 `harness/test-diff/bundles/`에 직접 파일 쓰기 | H-62 메인 결정 4 |

## 메인의 병합 절차

`trusted_authority`가 `refs/remotes/origin/master`에서 묶음을 읽으므로, **묶음 도입 커밋 `I`를 먼저 `origin/master`에 단독 푸시·갱신한 뒤** 병합 커밋 `M`을 만들어 푸시한다(`I`와 `M`을 로컬에 연달아 만들고 한꺼번에 푸시하면 로컬 `origin/master`가 아직 `I` 이전이므로 훅이 닫힌 실패로 거부한다).

1. 초안 생성(표준 출력 검토 후 메인이 직접 `harness/test-diff/bundles/<작업 ID>.toml` 저장):
   - `just test-diff-bundle-draft <작업 ID> <기준 40자리 SHA> <브랜치 40자리 SHA> [merge-head|target-commit|target-tree]`
2. 사전 대조 확인(병합 전 작업 브랜치 끝 커밋 기준):
   - `just test-diff-bundle-check harness/test-diff/bundles/<작업 ID>.toml <기준 40자리 SHA> <브랜치 40자리 SHA>`
3. 묶음 단독 서명 커밋 `I` 생성 후 `origin/master`에 단독 푸시 및 원격 추적 참조 갱신:
   - `git add harness/test-diff/bundles/<작업 ID>.toml && git commit -S -m "..."`
   - `git push origin master && git fetch origin master`
4. 병합 커밋 메시지 트레일러 생성 및 `--no-ff` 병합 커밋 `M` 생성·푸시:
   - `just test-diff-bundle-message harness/test-diff/bundles/<작업 ID>.toml`
   - 커밋 본문 끝에 아래 트레일러 두 줄을 붙여 병합한다(`merge-head` 모드에서 커밋된 병합 커밋에 필수):
     ```text
     Test-Diff-Bundle: harness/test-diff/bundles/<작업 ID>.toml
     Test-Diff-Findings: <건수>
     ```
   - 병합 커밋 `M`에서 `just test-diff-bundle-check harness/test-diff/bundles/<작업 ID>.toml <I의 SHA> <M의 SHA>`와 `just base=<I의 SHA> test-diff`, `mutants-diff` 선행 검사, CI `ci.yml`이 그대로 통과한다.

## 보류 결정과 선택지

1. **대상 지정자 운용 범위**: `merge-head`가 실시간 병합의 기본이자 유일한 사전 고정 모드다(`target-tree`는 병합 결과 트리 안에 묶음 파일 자신이 들어 있어 트리 해시 순환 때문에 실시간 사전 병합에는 사용할 수 없으며, `target-commit`·`target-tree`는 이미 존재하는 과거 병합 커밋의 사후 재현·감사용으로만 사용한다).
   - 선택지 A(구현값): 실시간 병합은 `merge-head` 고정, 과거 병합 사후 검증만 `--mode target-commit` 또는 `--mode target-tree` 사용.
   - 선택지 B: 사후 검증 모드를 `target-commit` 하나로 줄이고 `target-tree` 제거.
2. **작업 브랜치 사전 검증**: 현재 묶음은 병합 커밋(또는 진행 중 `MERGE_HEAD`)에만 적용되며, 작업 브랜치 자체에서는 의도대로 거부된다.
   - 선택지 A(구현값): 병합 커밋·진행 중 병합만 허용(재사용·확장 불가 엄격 유지), 병합 전 확인은 `just test-diff-bundle-check`로 수행.
   - 선택지 B: `test-diff` 자체에 작업 브랜치 사전 검증 플래그를 추가(재사용 방지 경계가 느슨해질 위험 있어 비권장).

## 결과

- 승인된 구조 외 테스트 변경이 있는 병합에서 별도 35분 검사 스레드와 훅 우회 없이 정식 CI(`ci.yml`)가 초록으로 통과한다.
- H-63의 Linux/macOS 병렬 변경분 변이 선행 `test-diff`도 동일한 `approval::apply_for_base`를 호출해 승인 묶음 구간에서 즉시 변이를 시작한다.
- 바뀌면 다시 볼 조건: `cargo-nextest` 출력 형식이나 `test-diff` 지적 문자열 형식이 바뀌어 기존 묶음 호환성이 깨질 때, 또는 승인 묶음 수가 누적되어 보관·아카이브 정책이 필요해질 때.
