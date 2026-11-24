set shell := ["bash", "-euo", "pipefail", "-c"]
set positional-arguments

unexport CARGO_ENCODED_RUSTFLAGS
unexport CARGO_BUILD_RUSTFLAGS
unexport RUSTDOCFLAGS
unexport CARGO_BUILD_RUSTDOCFLAGS
unexport CLIPPY_CONF_DIR
unexport CARGO_TARGET_DIR
unexport CARGO_BUILD_TARGET_DIR
unexport RUSTC
unexport CARGO_BUILD_RUSTC
unexport CARGO_BUILD_RUSTC_WRAPPER
unexport CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER
unexport RUSTUP_TOOLCHAIN
unexport RUSTC_BOOTSTRAP
unexport TAURI_CONFIG
unexport NEXTEST_PROFILE
unexport NEXTEST_RETRIES
unexport NEXTEST_FLAKY_RESULT
unexport NEXTEST_NO_TESTS
unexport NEXTEST_USER_CONFIG_FILE
unexport NEXTEST_TEST_THREADS
unexport CARGO_TARGET_AARCH64_APPLE_DARWIN_RUNNER
unexport CARGO_BUILD_TARGET
unexport ESLINT_PLUGIN_BOUNDARIES_ROOT_PATH
unexport GIT_DIR
unexport GIT_WORK_TREE
unexport GIT_INDEX_FILE
unexport GIT_OBJECT_DIRECTORY
unexport GIT_ALTERNATE_OBJECT_DIRECTORIES
unexport GIT_COMMON_DIR
unexport GIT_SHALLOW_FILE
unexport GIT_CONFIG_GLOBAL
unexport GIT_CONFIG_SYSTEM
unexport GIT_CONFIG_COUNT
unexport GIT_CONFIG_PARAMETERS
unexport GIT_DIFF_OPTS
unexport GIT_ATTR_SOURCE
unexport NODE_OPTIONS
unexport NODE_PATH
export SANDBOX_RUNTIME := if env("CODEX_SANDBOX", "") == "seatbelt" { "1" } else { env("SANDBOX_RUNTIME", "") }
export RUSTFLAGS := "-D warnings"
export RUSTC_WRAPPER := ""
export RUSTC_WORKSPACE_WRAPPER := ""

base := ""

check: check-node
    [ "$(node -e 'process.stdout.write("audit-ready")')" = audit-ready ]
    node harness/ci/temp-audit.mjs "{{just_executable()}}" base={{quote(base)}} check-audited

[env("STOCK_COPILOT_PROPTEST_CASES", "4096")]
check-audited: check-node require-base check-ts
    cargo fmt --all --check
    "{{just_executable()}}" clippy Cargo.toml
    "{{just_executable()}}" deny
    "{{just_executable()}}" arch-check
    "{{just_executable()}}" source-check
    "{{just_executable()}}" sql-check
    node harness/sqlcipher/check.mjs
    "{{just_executable()}}" migration-check
    "{{just_executable()}}" capability-check
    node harness/generated-files/check.mjs
    cargo run --locked --quiet --package xtask -- gen-schema --check
    "{{just_executable()}}" raw-types
    "{{just_executable()}}" disable-check
    cargo nextest run --workspace --locked
    "{{just_executable()}}" coverage-check
    "{{just_executable()}}" test-diff {{quote(base)}}
    "{{just_executable()}}" disable-report
    "{{just_executable()}}" disable-report-since {{quote(base)}}
    "{{just_executable()}}" hooks-installed

check-full: advisory-db-fetch check
    "{{just_executable()}}" visual-check
    node harness/ds-overflow/examples.mjs
    "{{just_executable()}}" mutants-diff {{quote(base)}}

ds-check: check-pnpm
    node harness/ds-catalog/check.mjs
    node harness/ds-overflow/typecheck.mjs
    node --test apps/desktop/tests/ds22-overflow-verdict.test.mjs
    node harness/ds-overflow/launch.mjs

require-base:
    [ -n {{quote(base)}} ] || { printf 'base가 필요하다: just base=<HEAD|40자리 커밋 ID> check (커밋 훅은 HEAD, 종료 훅은 브리프의 기준 커밋, 푸시 훅은 푸시 전 원격 커밋)\n' >&2; exit 1; }
    cargo run --locked --quiet --manifest-path "{{justfile_directory()}}/Cargo.toml" --package xtask -- require-comparison-base {{quote(base)}}

check-node:
    actual="$(node --version)"; pinned="v$(cat .node-version)"; [ "$actual" = "$pinned" ] || { printf 'check-node: node %s 사용 중, 고정 버전은 %s (.node-version)\n' "$actual" "$pinned" >&2; exit 1; }

check-pnpm: check-node
    node harness/toolchain/pnpm.mjs

check-ts: check-node check-pnpm
    pnpm install --frozen-lockfile --ignore-scripts
    node harness/ts-deps/check.mjs
    node harness/ds-catalog/check.mjs
    node harness/visual/check.mjs
    node harness/ds-style/check.mjs
    node harness/ds-tokens/check.mjs
    cargo run --locked --quiet --package xtask -- gen-tokens --check
    node harness/ds-tokens/contrast.mjs
    pnpm exec prettier --check apps/desktop harness package.json pnpm-workspace.yaml .claude/hooks .claude/settings.json
    node harness/source-tree/check.mjs
    node harness/config-files/check.mjs
    node harness/claude-hooks/hashes.mjs
    pnpm -C apps/desktop exec tsc -p .
    node harness/ds-overflow/typecheck.mjs
    pnpm -C apps/desktop exec eslint --max-warnings 0 src
    node harness/eslint-violations/check.mjs
    apps/desktop/node_modules/.bin/eslint --max-warnings 0 --config apps/desktop/eslint.scripts.config.mjs apps/desktop/tests apps/desktop/eslint apps/desktop/eslint.config.mjs apps/desktop/eslint.scripts.config.mjs harness
    apps/desktop/node_modules/.bin/eslint --max-warnings 0 --config harness/claude-hooks/eslint.config.mjs .claude/hooks
    pnpm -C apps/desktop exec vite build
    node harness/tsc-violations/check.mjs
    node harness/knip-check/check.mjs
    node harness/vitest-check/check.mjs
    node --test 'apps/desktop/tests/*.test.mjs'
    node --test 'harness/claude-hooks/tests/*.test.mjs'
    node --test 'harness/codex-hooks/tests/*.test.mjs'
    node harness/codex-hooks/config.mjs
    node --test 'harness/antigravity-hooks/tests/*.test.mjs'
    node harness/antigravity-hooks/config.mjs
    node harness/ci/workflow-policy/check.mjs
    node harness/ci/review-records/check.mjs
    node harness/ci/linux/exclusions.mjs verify
    node --test 'harness/ci/tests/*.test.mjs'

clippy manifest *flags:
    cargo clippy --manifest-path "$1" --workspace --all-targets --locked "${@:2}" -- -D warnings

deny:
    test -d "${CARGO_HOME:-$HOME/.cargo}/advisory-dbs/advisory-db-3157b0e258782691" || { printf 'deny: 권고 DB가 없다. 네트워크가 되는 곳에서 just advisory-db-fetch를 한 번 돌린다\n' >&2; exit 1; }
    cargo deny --locked --offline check

arch-check:
    cargo run --locked --quiet --package xtask -- arch-check

source-check:
    cargo run --locked --quiet --package xtask -- source-check

sql-check:
    cargo run --locked --quiet --package xtask -- sql-check

migration-check:
    cargo run --locked --quiet --package xtask -- migration-check

capability-check:
    cargo run --locked --quiet --package xtask -- capability-check

raw-types:
    cargo run --locked --quiet --package xtask -- raw-types

disable-check:
    cargo run --locked --quiet --package xtask -- disable-check

disable-report:
    cargo run --locked --quiet --package xtask -- disable-report

disable-report-since commit:
    cargo run --locked --quiet --package xtask -- disable-report --since "$1"

hooks-installed:
    cargo run --locked --quiet --package xtask -- hooks-installed

commit-guard index="":
    cargo run --locked --quiet --manifest-path "{{justfile_directory()}}/Cargo.toml" --package xtask -- commit-guard "$1"

fresh-build: fresh-cargo fresh-node

fresh-cargo:
    cargo clean --locked --workspace
    cargo llvm-cov clean --locked --workspace

fresh-all:
    cargo clean --locked
    cargo llvm-cov clean --locked

fresh-node:
    pnpm install --frozen-lockfile --force --ignore-scripts

commit-base:
    cargo run --locked --quiet --manifest-path "{{justfile_directory()}}/Cargo.toml" --package xtask -- commit-base

commit-check:
    if [ -f "$(git rev-parse --git-path stock-copilot-merge-gate.json)" ]; then node harness/ci/merge-gate.mjs --hook; else base="$("{{just_executable()}}" commit-base)"; STOCK_COPILOT_BASE_USE=commit "{{just_executable()}}" base="$base" "$("{{just_executable()}}" commit-recipe)"; fi

push-check:
    cargo run --locked --quiet --package xtask -- push-check "{{just_executable()}}"

remote-check sha:
    node harness/ci/remote-check.mjs "$1"

merge-gate branch receipt="":
    if [ -n "$2" ]; then node harness/ci/merge-gate.mjs "$1" "$2"; else node harness/ci/merge-gate.mjs "$1"; fi

[env("PROPTEST_RNG_SEED", "5")]
[env("STOCK_COPILOT_PROPTEST_CASES", "4096")]
coverage-check:
    cargo run --locked --quiet --package xtask -- coverage-check

test-diff commit:
    cargo run --locked --quiet --package xtask -- test-diff "$1"

test-diff-bundle-draft task base target mode="":
    if [ -n "$4" ]; then cargo run --locked --quiet --package xtask -- test-diff-bundle draft "$1" "$2" "$3" --mode "$4"; else cargo run --locked --quiet --package xtask -- test-diff-bundle draft "$1" "$2" "$3"; fi

test-diff-bundle-check bundle base target:
    cargo run --locked --quiet --package xtask -- test-diff-bundle check "$1" "$2" "$3"

test-diff-bundle-message bundle:
    cargo run --locked --quiet --package xtask -- test-diff-bundle message "$1"

mutants-diff commit:
    cargo run --locked --quiet --package xtask -- mutants --in-diff "$1"

mutants-diff-plan commit:
    cargo run --locked --quiet --package xtask -- mutants --in-diff-plan "$1"

mutants-diff-plan-platform commit platform:
    if [ "$2" = "macos" ]; then cargo run --locked --quiet --package xtask -- mutants --in-diff-plan "$1" --platform "$2"; else cargo run --locked --quiet --package xtask -- mutants --in-diff-plan "$1" --platform "$2" --linux-exclusions; fi

mutants-diff-shard commit shard:
    cargo run --locked --quiet --package xtask -- mutants --in-diff "$1" --shard "$2"

mutants-diff-platform-shard commit platform shard:
    if [ "$2" = "linux" ]; then cargo run --locked --quiet --package xtask -- mutants --in-diff "$1" --platform "$2" --shard "$3" --linux-exclusions; else cargo run --locked --quiet --package xtask -- mutants --in-diff "$1" --platform "$2" --shard "$3"; fi

mutants-daily commit shard:
    cargo run --locked --quiet --package xtask -- mutants --daily "$1" --shard "$2"

mutants-all:
    cargo run --locked --quiet --package xtask -- mutants

advisory-db-fetch:
    cargo deny --locked fetch db

gen-bindings:
    node harness/generated-files/check.mjs --write

lint-file path:
    cargo run --locked --quiet --package xtask -- lint-file "$1"

known-gaps:
    node harness/known-gaps/list.mjs

claude-regression:
    node harness/claude-regression/run.mjs

linux-check: check-node
    node harness/ci/linux/exclusions.mjs verify
    cargo fmt --all --check
    "{{just_executable()}}" clippy Cargo.toml $(node harness/ci/linux/exclusions.mjs cargo)
    "{{just_executable()}}" deny
    "{{just_executable()}}" arch-check
    "{{just_executable()}}" source-check
    "{{just_executable()}}" sql-check
    node harness/sqlcipher/check.mjs
    "{{just_executable()}}" migration-check
    "{{just_executable()}}" capability-check
    "{{just_executable()}}" raw-types
    "{{just_executable()}}" disable-check
    node harness/ci/workflow-policy/check.mjs
    node harness/ci/review-records/check.mjs

[env("STOCK_COPILOT_PROPTEST_CASES", "4096")]
linux-test:
    cargo nextest run --workspace --locked $(node harness/ci/linux/exclusions.mjs cargo) -E "$(node harness/ci/linux/exclusions.mjs filter)"

codex-regression:
    node harness/codex-regression/run.mjs

codex-setup *flags:
    node harness/codex-hooks/config.mjs --setup "$@"

gen-tokens:
    cargo run --locked --quiet --package xtask -- gen-tokens

tokens-contrast:
    node harness/ds-tokens/contrast.mjs

mutants-shard shard:
    cargo run --locked --quiet --package xtask -- mutants --shard "$1"

check-fast: check-node require-base
    cargo run --locked --quiet --package xtask -- check-fast {{quote(base)}} "{{just_executable()}}"

check-fast-static:
    "{{just_executable()}}" deny
    cargo run --locked --quiet --package xtask -- gen-schema --check
    cargo fmt --all --check
    "{{just_executable()}}" arch-check
    "{{just_executable()}}" source-check
    "{{just_executable()}}" sql-check
    node harness/sqlcipher/check.mjs
    "{{just_executable()}}" migration-check
    "{{just_executable()}}" capability-check
    "{{just_executable()}}" raw-types
    "{{just_executable()}}" disable-check
    node harness/source-tree/check.mjs
    node harness/config-files/check.mjs
    node harness/claude-hooks/hashes.mjs
    node harness/codex-hooks/config.mjs
    node harness/antigravity-hooks/config.mjs
    node harness/ci/workflow-policy/check.mjs
    node harness/ci/review-records/check.mjs

test-diff-fast *packages:
    cargo run --locked --quiet --package xtask -- test-diff-selected {{quote(base)}} "$@"

disable-report-fast:
    "{{just_executable()}}" disable-report-since {{quote(base)}}

commit-recipe:
    cargo run --locked --quiet --manifest-path "{{justfile_directory()}}/Cargo.toml" --package xtask -- commit-recipe

frontend-build: check-pnpm
    pnpm -C apps/desktop exec vite build

check-fast-generated: frontend-build
    node harness/generated-files/check.mjs

visual-update:
    node harness/visual/launch.mjs update

visual-measure:
    node harness/visual/launch.mjs measure

visual-check:
    node harness/visual/launch.mjs compare

check-lite: check-node check-pnpm
    pnpm install --frozen-lockfile --ignore-scripts
    pnpm -C apps/desktop exec tsc -p .
    pnpm -C apps/desktop exec eslint --max-warnings 0 src
    pnpm -C apps/desktop exec vite build
    node harness/vitest-check/check.mjs
    node --test 'apps/desktop/tests/*.test.mjs'
    cargo fmt --all --check
    "{{just_executable()}}" clippy Cargo.toml
    "{{just_executable()}}" arch-check
    "{{just_executable()}}" source-check
    "{{just_executable()}}" sql-check
    "{{just_executable()}}" migration-check
    "{{just_executable()}}" capability-check
    node harness/generated-files/check.mjs
    env -u STOCK_COPILOT_PROPTEST_CASES cargo nextest run --workspace --locked
