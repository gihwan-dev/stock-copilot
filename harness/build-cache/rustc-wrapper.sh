#!/bin/sh
set -eu
if [ "${CI:-}" != "" ] || [ "${GITHUB_ACTIONS:-}" != "" ] || [ "${STOCK_COPILOT_BUILD_CACHE:-}" = "off" ] || [ "${CARGO_PRIMARY_PACKAGE:-}" != "" ] || [ "${LLVM_PROFILE_FILE:-}" != "" ]; then
  exec "$@"
fi
if [ "$#" -lt 2 ]; then
  exec "$@"
fi
case "${1##*/}" in
  rustc) ;;
  *) exec "$@" ;;
esac
has_registry=0
has_out_dir=0
for arg in "$@"; do
  case "$arg" in
    -vV|--version|-V|--print|--print=*|incremental=*|-Cincremental=*|instrument-coverage|-Cinstrument-coverage|instrument-coverage=*|-Cinstrument-coverage=*|@*)
      exec "$@"
      ;;
    */registry/src/*.rs)
      has_registry=1
      ;;
    --out-dir|--out-dir=*)
      has_out_dir=1
      ;;
  esac
done
if [ "$has_registry" -eq 0 ] || [ "$has_out_dir" -eq 0 ]; then
  exec "$@"
fi
script_dir="${0%/*}"
if [ "$script_dir" = "$0" ]; then
  script_dir="."
fi
exec node "$script_dir/rustc-wrapper.mjs" "$@"
