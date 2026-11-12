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

check:
    cargo fmt --all --check
    "{{just_executable()}}" clippy Cargo.toml
    cargo nextest run --workspace --locked

check-full: check

clippy manifest *flags:
    cargo clippy --manifest-path "$1" --workspace --all-targets --locked "${@:2}" -- -D warnings

lint-file path:
    case "$1" in *.rs) rustfmt --edition 2024 --check -- "$1" ;; *) printf 'lint-file: %s에 적용할 검사가 아직 없다\n' "$1" ;; esac
