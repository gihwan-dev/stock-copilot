set shell := ["bash", "-euo", "pipefail", "-c"]
set positional-arguments

check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo nextest run --workspace --locked

check-full: check

lint-file path:
    case "$1" in *.rs) rustfmt --edition 2024 --check -- "$1" ;; *) printf 'lint-file: %s에 적용할 검사가 아직 없다\n' "$1" ;; esac
