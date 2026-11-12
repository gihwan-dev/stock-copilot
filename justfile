set shell := ["bash", "-euo", "pipefail", "-c"]

check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo nextest run --workspace --locked

check-full: check

lint-file path:
    case "{{path}}" in *.rs) rustfmt --edition 2024 --check "{{path}}" ;; *) echo "lint-file: {{path}}에 적용할 검사가 아직 없다" ;; esac
