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
export RUSTFLAGS := "-D warnings"
export RUSTC_WRAPPER := ""
export RUSTC_WORKSPACE_WRAPPER := ""

check: check-ts
    cargo fmt --all --check
    "{{just_executable()}}" clippy Cargo.toml
    "{{just_executable()}}" deny
    "{{just_executable()}}" arch-check
    "{{just_executable()}}" source-check
    cargo nextest run --workspace --locked

check-full: advisory-db-fetch check

check-node:
    actual="$(node --version)"; pinned="v$(cat .node-version)"; [ "$actual" = "$pinned" ] || { printf 'check-node: node %s 사용 중, 고정 버전은 %s (.node-version)\n' "$actual" "$pinned" >&2; exit 1; }

check-ts: check-node
    pnpm install --frozen-lockfile
    node harness/ts-deps/check.mjs
    pnpm exec prettier --check apps/desktop harness package.json pnpm-workspace.yaml
    pnpm -C apps/desktop exec tsc -p .
    pnpm -C apps/desktop exec eslint --max-warnings 0 src
    node harness/eslint-violations/check.mjs
    pnpm -C apps/desktop exec vite build
    node harness/tsc-violations/check.mjs
    node --test 'apps/desktop/tests/*.test.mjs'

clippy manifest *flags:
    cargo clippy --manifest-path "$1" --workspace --all-targets --locked "${@:2}" -- -D warnings

deny:
    test -d "${CARGO_HOME:-$HOME/.cargo}/advisory-dbs/advisory-db-3157b0e258782691" || { printf 'deny: 권고 DB가 없다. 네트워크가 되는 곳에서 just advisory-db-fetch를 한 번 돌린다\n' >&2; exit 1; }
    cargo deny --locked --offline check

arch-check:
    cargo run --locked --quiet --package xtask -- arch-check

source-check:
    cargo run --locked --quiet --package xtask -- source-check

advisory-db-fetch:
    cargo deny --locked fetch db

lint-file path:
    case "$1" in *.rs) rustfmt --edition 2024 --check -- "$1" ;; *) printf 'lint-file: %s에 적용할 검사가 아직 없다\n' "$1" ;; esac
