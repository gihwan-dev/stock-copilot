# ADR 0015: `xtask lint-file`, lefthook git 훅, TS 끄기 보고

- 상태: 제안
- 날짜: 2026-11-12
- 결정한 사람: 메인 에이전트(사용자 확인 대기)
- 관련 작업: H-30, H-14, H-40(TS 쪽)
- 반영한 문서: 없음 (harness.md 1.1~1.3, 2.1, 6.8, 7.1~7.3, 7.7, R-10 Q7·3.4, ADR 0005·0009·0010·0013을 따르고 문서가 정하지 않은 곳만 여기 적는다)

## 맥락

harness.md 1.1은 편집 훅이 부르는 `just lint-file <경로>`, 커밋 훅의 `just check`, 푸시 훅의 `just check-full`을 정했고, 7.2는 훅이 `target/` 아래 산출물을 직접 부르지 말라고 정했다. ADR 0013은 커밋 훅의 기준(`HEAD`)과 푸시 훅의 기준(푸시 전 원격 SHA), 낡은 빌드(mtime을 되돌린 소스) 문제를 H-14로 넘겼다. harness.md 2.1은 `disable-report`가 TS 끄기도 같은 표에 내라고 정했다. 정하지 않은 것: lint-file이 파일 종류마다 무엇을 돌리는가, 새 브랜치를 푸시할 때의 기준, 훅 설치를 무엇과 비교하는가, lefthook 고정 버전, 낡은 빌드를 막는 방법, "새 끄기"를 무엇으로 맞추는가.

## 실측

환경: macOS 26 (Apple Silicon), lefthook 2.2.2(Homebrew `lefthook` bottle), just 1.59.0, git 2.45.2, Rust 1.99.1, prettier 3.9.10, ESLint(앱 `node_modules`), cargo-llvm-cov 0.9.1.

| 항목 | 명령 | 결과 |
|---|---|---|
| lefthook 버전 | `brew info lefthook`, `lefthook version` | Homebrew 안정판이 2.2.2(R-10이 쓴 버전)다 |
| 생성되는 훅 | 임시 저장소에서 `lefthook install` | 설정한 훅마다 한 파일(`pre-commit`, `pre-merge-commit`, `pre-push`). 내용에 lefthook 실행 파일 경로(`/opt/homebrew/bin/lefthook`)가 들어간다. 저장소 경로는 들어가지 않는다. `.git/info/lefthook.checksum`도 만든다 |
| 작업 사본 | `git worktree add` 뒤 `git rev-parse --git-path hooks` | 공통 `.git/hooks`. 작업 사본에서 `lefthook install`해도 같은 곳에 쓴다 |
| 설정 파일 이름 | 이름마다 파일을 두고 `lefthook run pre-commit --force` | `lefthook`·`.lefthook`·`.config/lefthook`와 `-local` 짝, 확장자 `yml`·`yaml`·`toml`·`json`·`jsonc` 30개 모두 읽힌다. `-local`은 `lefthook.yml`의 같은 이름 작업을 덮어쓴다 |
| 숨긴 변경 | 일부만 스테이징하고 커밋 | lefthook이 스테이징하지 않은 추적 파일 변경을 커밋 훅 동안 숨긴다(`git stash create`). 추적하지 않는 파일은 그대로 있다 |
| 훅의 git 환경 | 훅 안에서 `env` | `GIT_INDEX_FILE`(보통 `.git/index`, `commit -a`면 `index.lock`), 작업 사본이면 `GIT_DIR`, `git -c`가 있으면 `GIT_CONFIG_PARAMETERS`가 설정된다 |
| 훅 안의 하네스 테스트 | 위 환경을 지우지 않고 커밋 훅에서 `just check` | 임시 저장소에서 `git worktree add`하던 테스트가 `.git/index: index file open failed`로 실패했다. 작업 사본에서는 `GIT_DIR`가 진짜 저장소를 가리켜 임시 저장소용 git 명령이 진짜 저장소에 닿는다 |
| stdin 입력 형식 | `use_stdin: true`인 푸시 작업 | `refs/heads/main <로컬 SHA> refs/heads/main <원격 SHA>`, 새 브랜치면 원격 SHA가 0 40개 |
| 낡은 빌드 | 작은 작업 공간에서 빌드 → 소스 수정 → mtime 되돌림 → `cargo run` | 고친 소스가 반영되지 않는다. `cargo clean --workspace` 뒤에는 반영된다 |
| 정리 비용 | `cargo clean --locked --workspace`, `cargo llvm-cov clean --locked --workspace` | 약 4초. 이어지는 `just base=HEAD check` 1분 46초(정리 없이 통과하던 상태 1분 40초 안팎) |
| 커밋 훅 전체 | 이 판을 복제한 저장소에 `lefthook install` 후 `git commit` | 2분 4초(commit-guard 0.07초, fresh-build 3.9초, check 120초). 구현 소스를 고치고 mtime을 되돌린 커밋은 다시 빌드한 테스트가 실패해 112초 만에 거부됐다 |
| rustfmt 표준 입력 | `rustfmt --check` < 파일 | 차이가 있어도 종료 0이다. 그래서 `--emit stdout` 출력과 입력을 비교한다 |
| ESLint 표준 입력 | `--stdin --stdin-filename` | 디스크에 없는 TS 파일은 타입 정보 프로젝트가 찾지 못해 실패한다. 있는 파일 이름으로는 준 내용을 검사한다 |
| ESLint가 다루지 않는 파일 | `--print-config` | 무시 목록이나 맞는 `files`가 없으면 `undefined`를 낸다 |
| lint-file 소요(데운 뒤) | `just lint-file …` | `.rs`(작업 공간 멤버) 0.9초, `apps/desktop/src`의 `.tsx` 2.8초, 하네스 `.mjs` 1.3초, `.md` 0.8초, 하네스 `.toml` 1.0초 |

## 결정

| 결정 | 근거 | 버린 대안 | 출처 |
|---|---|---|---|
| `just lint-file`은 `cargo run --locked --quiet --package xtask -- lint-file "$1"`다. 경로는 인자로만 넘기고 셸 문자열에 끼우지 않는다 | 훅이 `target/` 산출물을 직접 부르지 않는다. 기존 명령 주입 테스트가 그대로 통과한다 | `target/debug/xtask` 직접 실행 | harness.md 7.2 |
| 경로는 현재 폴더 기준으로 절대 경로로 만든 뒤 `..`을 풀고, 저장소 루트 밖이면 "저장소 밖", 없으면 "없는 파일", 폴더면 "폴더", 마지막 요소나 중간 폴더가 심볼릭 링크면 "심볼릭 링크"로 실패한다 | 편집 훅에 들어오는 경로를 그대로 믿지 않는다 | 실제 경로로만 풀기(링크를 따라 저장소 안으로 들어오는 경로를 통과시킨다) | harness.md 7.2 |
| Rust 파일: 작업 공간 멤버 안의 `.rs`이면 `rustfmt`(표준 입력, 루트 `rustfmt.toml`), `source-check`, `raw-types`(대상 crate의 모듈 트리에 있는 파일만), `disable-check`, `sql-check`를 그 파일의 내용에 돌린다. 멤버 밖 `.rs`(위반 예제, `target/`)는 `just check`도 보지 않으므로 검사하지 않는다 | `just check`와 같은 판정을 파일 하나에 준다. `raw-types`는 모듈 트리 밖 파일(테스트 모듈 포함)을 보지 않으므로 같은 범위만 본다 | 파일 경로로 `rustfmt --check`(하위 모듈 파일까지 검사한다) | harness.md 1.1 |
| `source-check`는 그 파일이 규칙 대상(멤버 `.rs`, 매니페스트, 대상 소스, 하위 폴더의 설정 파일 이름, `.cargo/`, `.config/nextest.toml`)일 때만 적용한다 | 규칙이 없는 파일에 "통과"를 내지 않는다 | 모든 파일에 적용 | 브리프 |
| TS 쪽: `harness/lint-file/targets.toml`의 prettier 대상(`check-ts`의 `prettier --check` 인자)이면 `prettier --file-info`로 무시·미지원을 먼저 보고, 아니면 `--check --stdin-filepath`로 검사한다. ESLint 대상(`check-ts`의 ESLint 두 줄: 앱 설정의 `apps/desktop/src`, 스크립트 설정의 다섯 경로)이면 `--print-config`로 설정이 다루는지 보고 `--max-warnings 0 --stdin`으로 검사한다. 테스트 `targets_mirror_the_check_ts_recipe`가 이 목록이 `just --show check-ts`와 같은지 본다 | `just check`가 보는 파일만 같은 설정으로 본다. 설정이 다루지 않는 파일은 "적용하지 않음"으로 보인다 | 확장자로 판정(설정과 어긋난다), `--no-warn-ignored`(무시된 파일이 통과로 보인다) | harness.md 1.1, 4장 |
| 적용한 검사가 없으면 `lint-file: <경로>: 적용할 검사 없음`과 종료 0. 하나라도 위반이면 검사별 메시지와 종료 1 | 가짜 통과를 내지 않는다 | - | 브리프 |
| `disable-report`의 TS 수집은 추적 파일과, 저장소 루트의 추적된 `.gitignore`만 무시 규칙으로 쓴 추적하지 않는 파일(`git_base::listed_paths`) 중 `ts`·`tsx`·`mts`·`cts`·`js`·`jsx`·`mjs`·`cjs`이고 `harness/eslint-violations/src/` 밖인 것을 훑는다. 주석은 앱이 쓰는 TypeScript 파서(`apps/desktop/node_modules/typescript`)로 뽑는다(`harness/disable-control/comments.mjs`, 표준 입력으로 파일 이름과 내용을 받는다). JSX 텍스트 안에서 시작하는 주석 모양은 버린다. 주석의 줄마다(앞의 공백·`*`·`/` 제외) `eslint-disable-next-line`, `eslint-disable-line`, `eslint-disable`, `eslint-enable`, `eslint`, `@ts-ignore`, `@ts-expect-error`, `@ts-nocheck`로 대소문자 없이 시작하면 행을 낸다. 규칙과 이유는 ESLint처럼 공백으로 둘러싼 `--`로 나눈다 | 형태 판정은 ESLint가 한다(ADR 0009). 처음 판의 손으로 쓴 렉서는 JSX 텍스트 안의 백틱 하나 뒤의 지시문을 숨겼다(리뷰 재현). TypeScript는 블록 주석의 마지막 줄과 대소문자가 다른 `@ts-` 지시문도 따른다 | 손으로 쓴 렉서(첫 판), 줄 단위 문자열 검색 | harness.md 2.1, 리뷰 |
| ESLint가 그 파일을 다루지 않으면(`--print-config`가 `undefined`이거나 ESLint 대상 경로 밖) 종류에 ` (ESLint 밖)`을 붙인다. 개수 줄은 기존 `disable-report: 끄기 N건 (종류별)` 뒤에 `disable-report: Rust N건, TS M건`을 더한다 | 기존 표 형식과 테스트를 그대로 둔다(ADR 0010) | 종류 이름만 | ADR 0010 |
| `disable-report --since <HEAD|40자리 커밋 ID>`는 `git_base`의 기준 규칙을 쓰고, 기준 대비 바뀐 파일(위 기준의 추적하지 않는 파일 포함)마다 현재 행과 기준 커밋의 같은 경로 내용의 행을 (종류, 경로, 규칙, 이유, 적용되는 줄의 내용)으로 맞춘다. 적용되는 줄은 TS는 `eslint-disable-line`이면 주석이 있는 줄, 나머지는 주석 뒤 첫 비지 않은 줄, Rust는 `#[expect]`와 뒤따르는 속성·빈 줄을 건넌 첫 줄이다. 맞는 짝이 없는 현재 행만 "새 끄기"다. 기준 커밋을 찾지 못하면 git의 원인을 메시지에 넣는다. `just check`는 `disable-report` 뒤에 `disable-report-since {{base}}`를 돈다 | 줄 번호는 위아래 편집으로 바뀐다. 기존 끄기를 새 코드로 옮긴 것이 기존으로 보였다(리뷰 재현) | 줄 번호까지 비교, (종류, 경로, 규칙, 이유)만 비교(첫 판) | harness.md 2.1, 리뷰 |
| lefthook 고정 버전은 2.2.2다. `harness/git-hooks/lefthook.toml`에 두고 `hooks-installed`가 `lefthook version`과 비교하며, `lefthook.yml`의 `min_version`도 2.2.2다 | harness.md 1.3 표에 lefthook 행이 없다. 1.3은 "리서치에서 실제로 돌려 본 버전"을 고정한다고 했고 R-10이 2.2.2로 돌렸다. Homebrew 안정판도 2.2.2다 | 최신 추적 | harness.md 1.3, R-10 |
| `lefthook.yml`: `assert_lefthook_installed: true`. 커밋 훅과 병합 커밋 훅(`pre-merge-commit`)은 `piped`로 `just fresh-build` → `just commit-guard "$GIT_INDEX_FILE"` → `just base=HEAD check`(병합이면 HEAD가 병합 전 커밋이다). 푸시 훅은 `piped`로 `just fresh-build` → `just push-check`(stdin 전달). 테스트가 파일 내용을 글자까지 고정한다 | 앞 단계가 실패하면 뒤를 돌리지 않는다. `merge --no-ff`는 `pre-commit`을 거치지 않는다(리뷰 재현). `commit-guard`는 xtask라 정리 뒤에 빌드한다 | `lefthook-local.yml`로 나누기 | harness.md 1.2, R-10 3.4, 리뷰 |
| 낡은 빌드: 훅은 검사 전에 `just fresh-build` = `fresh-cargo`(`cargo clean --locked --workspace`, `cargo llvm-cov clean --locked --workspace`)와 `fresh-node`(`pnpm install --frozen-lockfile --force`)를 돈다. `--force`는 잠금 파일대로 `node_modules`를 다시 맞춰 바꿔치기한 실행 파일·패키지 파일을 되돌린다(실측 6~7초, 20초 기준 안) | 작업 공간 crate와 `node_modules`만 다시 맞춘다. 훅 소유의 별도 target 폴더는 `CARGO_TARGET_DIR`를 지우는 환경 고정(ADR 0005)과 부딪히고 mtime을 그 폴더의 빌드 시각보다 앞으로 되돌리면 다시 속는다. `--frozen-lockfile`만으로는 바꿔치기한 `node_modules/.bin/eslint`가 그대로 남았다(실측) | 훅 소유 target 폴더, 전체 `cargo clean`, `node_modules` 지우고 설치 | ADR 0013, 리뷰 |
| `commit-guard`(`xtask commit-guard [인덱스]`, 저장소는 현재 폴더의 git 최상위): 커밋할 인덱스(훅의 `GIT_INDEX_FILE`)에서 (a) `git ls-files -v`에 assume-unchanged(소문자 태그)·skip-worktree(`S`) 항목이 있으면, (b) 모든 0단계 항목의 인덱스 blob이 작업 사본 파일의 `git hash-object --no-filters` 값(링크는 링크 대상 문자열의 해시)과 다르거나, 실행 권한이 모드와 다르거나, 파일이 없으면, (c) 충돌 단계 항목이나 서브모듈이 있으면, (d) 루트의 추적된 `.gitignore`만 무시 규칙으로 쓴(`ls-files --others --exclude-from`, `.git/info/exclude`·`core.excludesFile`·하위 `.gitignore`는 쓰지 않는다) 추적하지 않는 파일이 있으면 실패한다. `commit -a`는 통과한다 | 훅은 작업 사본을 검사하므로 커밋 내용과 작업 사본이 다른 길(부분 커밋, `update-index --cacheinfo`+표시, 숨긴 새 파일)을 닫는다. git의 "변경 없음" 판정과 표준 무시 규칙은 위 표시와 파일로 속일 수 있다(리뷰 재현) | lefthook의 숨기기, `git diff --quiet`와 `--exclude-standard`(첫 판) | 리뷰 |
| 루트 밖의 `.gitignore`(추적 여부와 무관)는 `source-check`의 `file:nested-gitignore`다(`node_modules`·`target`·`.git`·`.claude`는 원래 훑지 않는다). `test-diff`, `disable-report`(`--since` 포함), TS 끄기 수집도 위 (d)와 같은 기준으로 파일을 훑는다 | 하위 `.gitignore`에 `*`를 두면 자기 자신까지 숨긴다(리뷰 재현). 위반 예제는 `dot-gitignore.example`로 둔다 | 하위 `.gitignore` 허용 | 리뷰 |
| `justfile`이 `git_base::REDIRECTING_VARIABLES` 13개(`GIT_DIR`, `GIT_INDEX_FILE`, `GIT_CONFIG_PARAMETERS` 등)를 `unexport`한다. 커밋 인덱스는 `commit-guard`에만 인자로 준다 | 훅이 설정한 git 변수가 하네스 테스트의 임시 저장소 git 명령을 진짜 저장소로 돌린다(실측) | 테스트마다 변수 지우기(기존 테스트를 바꿔야 한다) | 실측 |
| 푸시 기준(`xtask push-base`, stdin): 지우기 줄은 건너뛴다. 푸시하는 로컬 SHA는 HEAD여야 한다. 원격 SHA가 있으면 그것(HEAD의 조상이어야 한다). 새 브랜치면 `git rev-list --boundary <로컬> --not --remotes`의 경계 커밋 하나(이미 원격에 있는 커밋이면 그 커밋, 원격과 공통 커밋이 없으면 첫 커밋 하나). 경계가 여럿이거나 줄마다 기준이 다르면 실패 | 훅은 작업 사본을 검사하므로 HEAD가 아닌 커밋은 검사할 수 없다. 원격 추적 참조가 오래되었으면 기준이 더 오래된 쪽이 되어 더 많이 본다 | 기본 브랜치 이름으로 `merge-base`(브랜치 이름은 받지 않는다, ADR 0011·0013), 새 브랜치는 `HEAD` | ADR 0013, 브리프 |
| `just check`의 마지막 단계는 `hooks-installed`다. `git rev-parse --git-path hooks`가 공통 git 폴더의 `hooks`가 아니면(`core.hooksPath`, 전역 설정 포함) 실패. 아니면 `target/git-hooks-expected`에 만든 임시 저장소에 같은 `lefthook.yml`로 `lefthook install`(전역·시스템 git 설정과 `LEFTHOOK*` 변수를 끈 채)을 돌려 만든 훅 파일과 바이트까지 같고 실행 권한이 있어야 한다. `lefthook.yml` 밖의 lefthook 설정 파일 29개 이름 중 하나라도 있으면 실패 | 설치 여부를 lefthook 자신에게 묻지 않고 같은 버전이 만드는 내용과 비교한다. 작업 사본에서도 공통 훅을 본다. 다른 설정 파일은 `lefthook.yml`을 덮어쓴다(실측) | `lefthook check-install`(내용 비교를 하지 않는다), `check` 머리줄의 의존 레시피(기존 테스트가 머리줄을 고정한다) | harness.md 1.2, R-10 |
| 하네스 자체 테스트: 예제와 기대값은 `harness/lint-file/`(`cases.toml`, `examples/`), `harness/disable-control/script-cases.toml`·`script-sources/`, 규칙 데이터는 `harness/lint-file/targets.toml`, `harness/disable-control/typescript.toml`, `harness/git-hooks/lefthook.toml`. 앞의 둘은 원소 하나를 뺀 변형마다 어떤 테스트가 실패해야 하고(`removing_any_*_element_breaks_some_test`), 셋째는 테스트의 독립 목록과 같아야 한다. 커밋 거부·푸시 거부·부분 커밋·작업 사본·`core.hooksPath`·mtime 되돌림은 `target/harness/` 아래 임시 저장소로 확인한다 | harness.md 6.8 | - | harness.md 6.8 |

## 결과

- 병합 뒤 저장소 루트에서 `lefthook install`을 한 번 실행해야 `just check`의 `hooks-installed`가 통과한다. 공통 `.git/hooks`에 설치되므로 모든 작업 사본의 커밋이 그때부터 훅을 거친다. `lefthook.yml`이 없는 옛 기준의 작업 사본에서는 lefthook이 설정을 찾지 못했다는 줄을 내고 커밋을 허용한다(실측).
- 커밋마다 약 2분이 든다(harness 1.2의 커밋 훅 = `just check`). 3분을 넘지 않아 커밋 훅에서 빼는 단계는 없다. 넘게 되면 1.2에 맞춰 nextest·커버리지를 커밋 훅에 두고 `vite build`·`tsc-violations`처럼 바뀐 파일과 무관하게 같은 결과를 내는 단계부터 푸시 훅·종료 훅으로 옮기는 것을 제안한다.
- 추적하지 않는 파일이 있으면 커밋할 수 없다. 메모 파일은 무시 목록에 넣거나 저장소 밖에 둔다.
- 기존 테스트 `network_sandbox_tests::unix_socket_fake_server_answers`는 저장소 경로가 길면(유닉스 소켓 경로 104바이트 제한) 실패해 깊은 작업 사본의 커밋을 막는다. 소켓을 시스템 임시 폴더에 만들도록 본문을 바꿨다(단언은 그대로, 메인 승인). `test-diff`가 이 테스트를 `modified`로 보고하므로 이 변경이 들어간 커밋을 기준 이전으로 두는 검사는 사용자 승인이 필요하다.
- `lint-file`은 파일 하나로 돌지 않는 검사(Rust: clippy·arch-check·deny·테스트·커버리지·test-diff, TS: tsc·knip·Vitest·source-tree 심볼릭 링크 훑기·vite build)를 돌리지 않는다고 출력에 적는다. `check-ts`의 ESLint 대상 밖 스크립트(`apps/desktop/vitest.config.mjs` 등)는 "check-ts의 ESLint 대상 밖"으로 표시한다.
- 원리상 훅이 못 막는 것:
  - `git commit --no-verify`, `-n`, `LEFTHOOK=0`, `LEFTHOOK_CONFIG`, `LEFTHOOK_EXCLUDE`, `git -c core.hooksPath=…`, `GIT_CONFIG_PARAMETERS`처럼 훅을 건너뛰게 하는 호출. `hooks-installed`는 설치 상태만 보고 한 번의 호출을 막지 못한다.
  - 훅이 돌지 않는 커밋: fast-forward 병합, `cherry-pick`, `revert`, `rebase`, `commit-tree`·`update-ref`로 만든 커밋.
  - `PATH` 앞에 둔 가짜 `lefthook`·`just`·`cargo`·`pnpm`·`node`(훅 파일이 `PATH`의 `lefthook`을 먼저 부른다), `CARGO_HOME`의 레지스트리 소스 바꿔치기, pnpm store 자체의 변조(`--force`는 store에서 다시 가져온다).
  - 의존 crate의 산출물(`target/` 아래 서드파티 `rlib`) 바꿔치기. `fresh-cargo`는 작업 공간 crate만 지운다.
  - 커밋 훅 도중 작업 사본을 바꾸는 다른 프로세스, 푸시 사이에 원격이 바뀌는 경우, 오래된 원격 추적 참조(기준이 더 오래되는 쪽이라 약해지지는 않는다).
  - `disable-report`: 매크로나 생성 코드가 만드는 끄기, 이름을 바꾼 파일의 끄기는 새 끄기로 보인다. TypeScript 파서가 다른 해석을 하는 문법(TS 7 전용 문법)은 TS 6 파서의 결과를 따른다.
  - `lint-file`: 타입 검사(`tsc`)와 작업 공간 단위 검사(`arch-check`, `clippy`, 테스트, knip, Vitest)는 파일 하나로 돌 수 없어 `just check`에 남는다(출력에 적는다). 다른 파일의 내용에 따라 바뀌는 판정은 디스크의 다른 파일을 읽는다.
- 뒤 작업:
  - H-15: 셸 훅이 `git update-index --assume-unchanged`·`--skip-worktree`·`--cacheinfo`, `.git/info/` 쓰기, `LEFTHOOK*` 환경 변수, `CARGO_HOME` 설정, `--no-verify`·`-n`, `core.hooksPath` 덮어쓰기를 거부한다. 편집 훅이 하위 `.gitignore` 만들기를 거부한다. 서브에이전트의 `git merge`·`cherry-pick`·`revert`·`rebase`를 통제한다. `PostToolUse` 편집 훅이 `just lint-file <경로>`를 부른다.
  - 메인 에이전트의 병합 절차(workflow): 병합은 `--no-ff`로 한다(병합 커밋 훅이 돈다). fast-forward가 된 경우에는 병합 직후 `just base=<병합 전 master> check`를 돈다.
  - H-35: `node_modules/`·`target/`의 서드파티 산출물, pnpm store, `PATH` 앞 폴더 쓰기 차단.
  - H-16: 깨끗한 checkout에서 훅 설치 확인을 다루는 방법(`lefthook install` 뒤 `just check`), 병합·fast-forward로 들어온 범위 검사.
  - H-20: `lefthook.yml`, 모든 폴더의 `.gitignore`, `.git/hooks`의 삭제·변경 거부.
- 바뀌면 다시 볼 조건: lefthook을 올릴 때(훅 내용·설정 파일 이름·숨기기 동작·stdin 전달), git이 훅에 주는 환경 변수가 바뀔 때, `check-ts`의 prettier·ESLint 줄이 바뀔 때(테스트가 알린다), 커밋 훅이 3분을 넘을 때.
