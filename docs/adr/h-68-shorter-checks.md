# ADR: H-68 검사 시간 줄이기 (초안 — 1차 실측 및 즉시 적용 안내)

- 상태: 초안 (구현 진행 중, 2026-11-24)
- 관련 작업: H-68 (사용자 결정 2026-11-24)
- 기준 커밋: `56b83b2b604a8923d79864c331b8e472d9adf02c`

> 이 커밋은 같은 기기에서 병렬로 실행 중인 다른 스레드가 즉시 참고할 수 있도록 H-51 공유 실측 결과와 당장 쓸 수 있는 절차를 먼저 공유하는 중간 문서다. 구현·후속 측정·변이 대상 표·손 변형 표는 이어지는 커밋에서 완성한다.

## 1. H-51 공유 실측 (전)

2026-11-24 `56b83b2b604a8923d79864c331b8e472d9adf02c` 기준, 이 기기(`h-68` 작업 사본)에서 확인한 결과:

1. **작업 사본 사이 빌드 결과 공유는 현재 꺼져 있다 (재사용률 0%).**
   - 근거: [ADR 0133](0133-shared-compile-cache.md)에서 H-51은 APFS 씨앗 복제와 `sccache`를 모두 채택하지 않고 `just fresh-cargo`/`just fresh-all`만 남겼다.
   - `justfile`은 `export RUSTC_WRAPPER := ""` 및 `unexport CARGO_TARGET_DIR`로 고정되어 있고, `.cargo/config.toml`도 존재하지 않는다.
   - 실측 1 (새 작업 사본에서 첫 `cargo build --locked -p domain`): **17.67초**, `Compiling: 10`, `Fresh: 0`, `target/` 크기 `380 MiB`. 외부 의존성(`rust_decimal`, `time`, `serde` 등)까지 매 사본마다 처음부터 다시 컴파일한다.
   - 실측 2 (속성 테스트 횟수별 `domain` 단위 실행, 1,315개 테스트, 워밍 빌드 완료 상태):
     - `env -u STOCK_COPILOT_PROPTEST_CASES cargo nextest run --locked -p domain` (256회): **116.68초** (nextest `Summary [ 114.367s]`, 1,315 passed)
     - `STOCK_COPILOT_PROPTEST_CASES=4096 cargo nextest run --locked -p domain` (4,096회): **612.79초** (nextest `Summary [ 610.505s]`, 1,315 passed, 10 slow)
     - 차이: **5.25배** (`domain` 한 crate에서만 **496.11초** 절약).
   - 실측 3 (깨끗한 `target/`에서 작은 Rust 변경 — `crates/domain/src/account_id.rs` 테스트 1건 추가 — 의 `just base=56b83b2b604a8923d79864c331b8e472d9adf02c check-fast`):
     - 총 경과 시간: **1,793.52초 (29분 53.5초)**, 종료 코드 0 (부하 평균 시작 56.95 → 종료 34.77, `CARGO_BUILD_JOBS=4 NEXTEST_TEST_THREADS=4`).
     - 이 가운데 `cargo nextest` 실행(2,480개 테스트)은 **171.62초**였고, 나머지 **약 1,622초(27분)**는 `xtask`·`clippy`·`nextest`용 외부 의존성 처음 컴파일과 `Blocking waiting for file lock on package cache` 대기에 쓰였다.
     - 완료 직후 사본별 `target/` 크기: **7.1 GiB** (`debug/deps` 4.2 GiB + `debug/incremental` 2.1 GiB + `debug/build` 331 MiB 등). 8개 스레드가 동시에 돌면 단순 합산 50 GiB 이상을 점유해 SSD 잔여 용량이 1 GiB 미만까지 떨어지는 현상이 실제로 관측됐다.

## 2. 병렬 실행 중인 다른 스레드가 당장 적용할 수 있는 절차

H-68 병합 전이라도, 현재 같은 기기에서 실행 중인 다른 작업 스레드는 아래 세 가지만 지켜도 실행 시간과 디스크 압박을 즉시 크게 줄일 수 있다.

1. **작업 중 범위 테스트는 `STOCK_COPILOT_PROPTEST_CASES`를 비운 상태(256회)로 돌린다.**
   ```sh
   env -u STOCK_COPILOT_PROPTEST_CASES cargo nextest run --locked -p <crate>
   ```
   - `STOCK_COPILOT_PROPTEST_CASES`가 설정되어 있으면 값이 `"256"`이어도 `testkit::property_cases()`가 **4,096회**를 반환하므로 반드시 `env -u`로 해제한다.
   - 실패 재현 시 씨앗 고정은 그대로 `PROPTEST_RNG_SEED=<값>`을 함께 넘긴다.
   - `just base=<40자리 SHA> check-fast`는 문서(`docs/**`)·하네스·`Cargo.toml`·`testkit`·`testkit-ports`·`xtask` 변경이 있으면 전체 `just check`(4,096회)로 승격된다. Rust 구현 중에는 문서 편집 전에 `check-fast`를 먼저 돌린다.
2. **작업이 끝난 스레드·리뷰 사본은 즉시 `target/`을 정리한다.**
   ```sh
   cargo clean --locked && cargo llvm-cov clean --locked
   ```
   - 사본 하나가 7 GiB 이상을 점유하므로, 작업·리뷰 완료 후 즉시 정리해 다른 스레드의 `ENOSPC`를 막는다.
3. **외부 도구 설치나 `~/.cargo/config.toml` 전역 수정은 하지 않는다.**
   - H-68 구현에서 저장소 안 `harness/build-cache/` 기반의 외부 의존성 전용 내용-주소 `rustc` 래퍼(작업 공간 crate 비공유, 무잠금 원자적 게시, APFS `clonefile` 복원, `cargo-mutants`·`cargo-llvm-cov` 격리, 자동 디스크 상한 정리)를 `justfile`에 연결한다.

## 3. 후속 구현 계획 (이 브랜치의 다음 커밋)

1. **속성 테스트 횟수 정책 단일화**: `harness/proptest/policy.toml`(작업 중 256, 전체/CI 4,096), `just test-fast`, `justfile` 전역 `unexport STOCK_COPILOT_PROPTEST_CASES`/`PROPTEST_CASES`, `fast_check.rs` 빠른 경로 환경 정리(승격 시 `just check`의 4,096 유지), 정책 검사 및 테스트.
2. **작업 사본 사이 외부 의존성 빌드 캐시**: 저장소 안 `harness/build-cache/`(`rustc-wrapper.sh`/`.mjs`, `prune.mjs`, `policy.json`, `check.mjs`), `just clean-worktree`/`cache-prune`/`cache-clear`/`cache-stats`, `coverage_check`·`mutants_check` 격리, 전·후 측정.
3. **변이 대상 한정**: 보호된 단일 목록 `harness/mutants/targets.toml` 및 `.cargo/mutants.toml` 1:1 동기화, 변경분·일간·주간 공통 소비, 최근 병합 전·후 변이 수 비교표, 문서 갱신, 손 변형 20건 이상 및 적대적 리뷰.
