# H-51: 작업 사본의 빌드 씨앗

- 상태: 초안
- 날짜: 2026-11-18
- 관련 작업: H-51
- 구현 담당: Codex `gpt-6.1-terra` (max). 2026-11-17 사용자가 이 작업의 구현 모델로 정했다. 조사와 비판은 별도의 Codex `gpt-6.1-sol`, 적대적 리뷰는 새 Claude Opus 5.5라는 결정도 유지한다.

## 맥락

여러 작업 사본의 첫 빌드가 같은 의존성을 각각 컴파일해 시간과 디스크를 쓴다. 사용자 결정에 따라 메인 체크아웃의 `target/`을 각 작업 사본으로 APFS 복제한다. 메인 병합 검사가 쓰는 산출물을 신뢰하고, 각 복제본은 독립된 쓰기 경로를 가진다. 메인 체크아웃의 산출물이 조작되면 씨앗을 쓴 검사 결과도 바뀔 수 있다. 이는 메인 산출물에 대한 신뢰를 전제로 하며, 그 위험이 없어졌다는 뜻은 아니다.

## 결정

- 새 작업 사본의 `target/`이 없거나 비어 있을 때 `STOCK_COPILOT_SEED_TARGET`으로 메인 체크아웃의 `target/`을 지정하고 `just seed-target`을 실행한다. Git 공통 디렉터리로 확인한 메인 체크아웃의 정확한 경로만 받는다. 원본은 읽기만 하고, 다른 작업 사본과 산출물 쓰기를 공유하지 않는다.
- macOS 시스템 `clonefile`을 파일마다 호출한다. 실패할 때 일반 복사로 돌아가지 않고 임시 결과를 지운다. 시스템 `cp -c`는 복제 실패 시 일반 복사를 할 수 있다고 `man cp`가 명시하므로 쓰지 않는다. 이 환경에서 Node의 `COPYFILE_FICLONE_FORCE`는 `ENOSYS`였고, 저장소가 쓰는 시스템 Python의 표준 라이브러리로 `clonefile`을 직접 호출한다. 새 패키지나 설치 도구는 없다.
- 디렉터리는 새로 만들고 일반 파일을 복제한다. 원본의 심볼릭 링크나 특수 파일은 따라가거나 복제하지 않는다. 복제 중 실패하거나 원본이 없고 대상이 비어 있지 않으면 건너뛴다. 불완전한 결과는 게시하지 않는다.
- 복제한 workspace 산출물은 게시 전에 `cargo clean --locked --offline --workspace --target-dir <임시 복제본>`으로 제거한다. 실제로 다른 경로의 workspace 산출물이 새 코드에도 `Fresh`로 판정돼 이전 실행 결과가 나온 예제를 재현했기 때문이다. 외부 의존성 산출물은 남겨 Cargo가 자신의 fingerprint와 도구·입력에 따라 재사용하거나 다시 빌드한다. 정리 실패 시 전체 씨앗 적용을 건너뛴다.
- 정리 명령은 사용자 Cargo 설치의 `cargo`와 `rustc`를 고정 경로로 호출하고 컴파일러 래퍼를 비운다. 격리 fixture의 `CARGO_HOME/config.toml`에 가짜 `rustc`를 지정하면 원래 정리 명령이 이를 실행하는 것을 재현해, 해당 설정과 환경 변수로 대체되지 않는지 테스트한다. 홈 설정을 읽거나 수정하지 않았다.
- 작업 중 부하 제한을 실제 검사 자식 프로세스에도 적용한다. 기존 `justfile`은 외부 `NEXTEST_TEST_THREADS`를 지워 4가 전달되지 않았으므로, `check`, `check-ts`, `check-fast`, `coverage-check` 레시피가 신뢰된 고정값 4를 각각 설정한다. 호출자의 값은 계속 지운다. 이 고정값은 CI에도 적용된다.
- `fresh-cargo`는 `cargo clean --locked`와 `cargo llvm-cov clean --locked`를 실행한다. 종료 훅도 같은 범위를 오프라인으로 지운다. 이것은 이전에 `--workspace`가 남긴 외부 의존성 산출물까지 정리한다. `fresh-build`, 커밋·종료 훅, CI와 밤 실행은 씨앗을 호출하지 않는다.

## 버린 대안

여러 작업 사본이 쓰는 `sccache`는 사용자의 2026-11-18 결정으로 버렸다. 여러 주체가 쓰는 공유 캐시를 검사 결과에 쓰면 새 신뢰 경계가 생기며, 이를 서명·검증하는 비용이 이득보다 크다. `CARGO_TARGET_DIR` 하나를 공유하는 방식도 빌드 잠금과 산출물 혼합 때문에 쓰지 않는다. 기존 `target/`이 있는 작업 사본에 덮어쓰거나, 복제 실패를 일반 파일 복사로 대체하는 방식도 쓰지 않는다.

격리 fixture에서 `sccache 0.19.0`의 의존 함수는 실제로 `0`을 반환했고 테스트는 `1`을 기대했다. 무캐시 테스트는 종료 101과 `left: 0`, `right: 1`로 실패했다. `1`을 반환하는 별도 빌드의 유효한 캐시 아카이브를 원래 의존성 캐시 항목에 덮어쓴 뒤 원래 소스로 테스트하자 새 target에서 종료 0과 테스트 1건 통과가 나왔다. 서버 통계는 Rust 캐시 적중 2, 미스 0이었다. 그 target에서 `cargo clean --workspace --locked --offline` 뒤 래퍼 없이 다시 실행해도 통과했다. workspace만 다시 컴파일되고 외부 의존성 산출물은 남았다. 제품 코드와 실제 `check-fast`에 sccache를 적용한 실험은 아니다.

## 검증과 측정

두 측정 사본은 서로 다른 새 Git 작업 사본이며 같은 `732b58d1bab83bef0aa9a7affa51277316fbd651`에서 시작했다. 소스 변경이 없는 첫 `just base=<해당 SHA> check-fast`로 정적 검사와 `xtask` 부트스트랩을 측정했다. 작업 사본의 제품 crate 변경을 검사하는 시간은 이 표에 포함되지 않는다. 씨앗은 실측 전 메인 체크아웃 `target/`을 별도 측정용 메인 사본에 복제한 스냅숏이다. 측정 당시의 씨앗 스크립트는 이후 추가된 복제 결과 파일 종류 재확인과 Cargo·컴파일러 절대 경로 고정 전 버전이며, 최종 버전의 시간을 다시 측정한 값은 아니다. 둘 다 종료 0이었다.

| 항목                           |           씨앗 없음 |                              씨앗 있음 |
| ------------------------------ | ------------------: | -------------------------------------: |
| 시작 시각 (KST)                |            00:44:19 | 00:46:48 복제 시작, 00:47:53 검사 시작 |
| 시작 시 1분 load               |               83.22 |               복제 137.61, 검사 103.91 |
| `check-fast`                   |            141.05초 |                               213.79초 |
| 씨앗 복제와 workspace 정리     |                없음 |                                64.79초 |
| 검사 직후 `target/`의 `du -sk` |         909,860 KiB |                         12,590,140 KiB |
| 복제 전후 `df` 가용량 감소     |           해당 없음 |                    1,602,981,888바이트 |
| 검사 전후 `df` 가용량 감소     | 3,832,426,496바이트 |                    5,393,584,128바이트 |

씨앗을 쓴 검사 자체는 이번 한 번의 측정에서 51.6% 느렸고, 복제까지 포함하면 97.5% 느렸다. 두 실행의 load와 다른 스레드의 디스크 사용이 달랐다. `df` 차이는 같은 볼륨의 동시 작업과 삭제를 포함하므로 해당 작업만의 물리 사용량으로 단정할 수 없다. `du`는 APFS 공유 블록을 사본별로 계산할 수 있어 물리 증가량이 아니다. 이 한 번의 결과로 일반적인 속도 이득을 주장하지 않는다.

측정 뒤 `xtask`만 정리하고 같은 씨앗 사본에서 상세 Cargo 빌드를 다시 실행하자 의존성 45개는 `Fresh`, workspace의 `xtask`는 `Compiling`으로 나왔다. 측정 직후의 `.rlib` 비교에서는 workspace 패키지 이름에서 추정한 분류로 이름·크기·mtime이 씨앗과 같은 외부 의존성 파일 1,362개와 workspace 파일 0개가 남아 있었다. 이 파일 개수 전체가 실제 그 검사에서 소비됐다는 뜻은 아니다.

작은 fixture에서 씨앗 유무의 검사 결과, 비어 있지 않은 대상과 원본 보존, 복제 실패 시 일반 복사 금지, 다른 잠금 파일의 의존성 재빌드, 기존 `fresh-cargo`가 외부 의존성 산출물을 남기는 회귀와 수정 뒤 제거를 확인했다. Rust `1.95.0`으로 만든 씨앗을 Rust `1.99.1`에서 쓴 별도 실험에서는 외부 의존성을 다시 컴파일했고, 씨앗 유무의 테스트가 각각 종료 0이었다. 이는 두 버전과 작은 fixture에 대한 실측이다.

프로젝트 신뢰와 훅 정의 신뢰는 각각 확인하지 못했다. 세션 훅 확인용 Git 명령은 거부되지 않고 실행됐다. 최종 전체 검사와 리뷰 결과는 리뷰 기록을 따른다.

## 인계

H-51: 메인은 병합 뒤 새 작업 사본의 첫 빌드 전에만 선택적으로 씨앗 레시피를 쓰도록 각 스레드에 알린다. 기존 `target/`은 덮어쓰지 않으며, `fresh-build`는 그 복제본을 포함해 모두 지운다.

## 손 변형

`mktemp -d` 복사본에 고정 `rust-toolchain.toml`을 포함했다. 변경하지 않은 대조군 17개가 통과한 뒤 Python 구문 검사를 통과한 변형을 각각 실제 테스트로 실행했다. 그때 `justfile`은 스레드를 셸에 고정한 이전 버전이었으며 씨앗 Python 코드는 현재와 같다. 네 레시피 속성 변형은 변경 뒤 별도 복사본의 정상 대조군 통과 뒤 실행했다. 각 행은 테스트가 잡은 변형이다. 고정 툴체인이 빠진 첫 실행은 결과에서 제외했다.

| 번호 | 바꾼 것                            | 잡은 테스트                              |
| ---- | ---------------------------------- | ---------------------------------------- |
| 1    | clone nofollow 플래그 변경         | `a cloned symlink is rejected`           |
| 2    | clone 성공 판정 반전               | `a clone is private`                     |
| 3    | 복제 결과 심볼릭 링크 검사 제거    | `a cloned symlink is rejected`           |
| 4    | 원본 심볼릭 링크에서 실패          | `a clone is private`                     |
| 5    | 디렉터리 판정 변경                 | `a clone is private`                     |
| 6    | 대상 디렉터리 생성 제거            | `a clone is private`                     |
| 7    | 하위 디렉터리 재귀 제거            | `a clone is private`                     |
| 8    | 일반 파일 판정 변경                | `a clone is private`                     |
| 9    | 일반 파일 복제 제거                | `a clone is private`                     |
| 10   | 대상 파일을 빈 디렉터리로 판정     | `a target file or a target inside`       |
| 11   | 빈 디렉터리 판정 반전              | `a clone is private`                     |
| 12   | 운영체제 판정 반전                 | `a clone is private`                     |
| 13   | 없는 원본 검사 제거                | `a symlink seed root and a missing seed` |
| 14   | 원본과 대상 중첩 판정 반전         | `a target file or a target inside`       |
| 15   | 초기 빈 대상 판정 반전             | `a clone is private`                     |
| 16   | 트리 복제 제거                     | `a clone is private`                     |
| 17   | 게시 전 workspace 정리 제거        | `seeded and unseeded workspaces`         |
| 18   | 빈 대상 제거 조건 반전             | `seeded and unseeded workspaces`         |
| 19   | 복제본 대신 원본을 이동            | `a clone is private`                     |
| 20   | 복제 성공을 실패로 반환            | `a clone is private`                     |
| 21   | 복제 실패에서 예외 전파            | `failed cloning leaves`                  |
| 22   | 실패한 임시 복제본 정리 제거       | `failed cloning leaves`                  |
| 23   | 고정 Cargo 대신 PATH의 Cargo 사용  | `a fake cargo at the front`              |
| 24   | Cargo locked 인자 변경             | `workspace cleanup is locked`            |
| 25   | Cargo offline 인자 변경            | `workspace cleanup is locked`            |
| 26   | Cargo workspace 인자 변경          | `workspace cleanup is locked`            |
| 27   | Cargo target-dir 인자 변경         | `workspace cleanup is locked`            |
| 28   | Cargo 작업 디렉터리 변경           | `workspace cleanup is locked`            |
| 29   | Cargo 성공 강제 제거               | `workspace cleanup is locked`            |
| 30   | 공통 Git 디렉터리 판정 반전        | `a clone is private`                     |
| 31   | 메인 씨앗 경로 변경                | `a clone is private`                     |
| 32   | 신뢰한 원본 경로 판정 반전         | `a clone is private`                     |
| 33   | 사본의 대상 경로 변경              | `a clone is private`                     |
| 34   | 씨앗 루트 심볼릭 링크 거부 제거    | `a symlink seed root and a missing seed` |
| 35   | 정리 컴파일러 고정 제거            | `a cargo-home config cannot`             |
| 36   | 정리 컴파일러 래퍼 비우기 제거     | `a cargo-home config cannot`             |
| 37   | workspace 래퍼 비우기 제거         | `a cargo-home config cannot`             |
| 38   | `check` 스레드 상한 4 → 5          | `recipes fix nextest threads`            |
| 39   | `check-ts` 스레드 상한 4 → 5       | `recipes fix nextest threads`            |
| 40   | `coverage-check` 스레드 상한 4 → 5 | `recipes fix nextest threads`            |
| 41   | `check-fast` 스레드 상한 4 → 5     | `recipes fix nextest threads`            |
