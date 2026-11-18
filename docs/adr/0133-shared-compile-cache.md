# ADR 0133: H-51 작업 사본 사이의 컴파일 결과 공유 조사

- 상태: 확정 (2026-11-18)

- 날짜: 2026-11-18
- 관련 작업: H-51
- 구현 담당: Codex `gpt-6.1-terra` max. 2026-11-17 사용자가 구현 모델을 정했고, 조사·비판은 별도 Codex `gpt-6.1-sol`, 적대적 리뷰는 새 Claude Opus 5.5로 유지했다.

## 맥락

여러 작업 사본이 외부 의존성을 다시 컴파일해 시간과 디스크를 사용한다. 공유 캐시의 신뢰 경계와 메인 `target/`에서 APFS `clonefile`로 사적인 씨앗을 만드는 후보를 조사했다. 2026-11-18 사용자는 씨앗 없음과 있음을 번갈아 두 번 이상 실측해 씨앗 검사의 중앙값이 10% 이상 빠를 때만 채택하기로 했다.

## 결정

- 네 번의 검사 중앙값은 씨앗 없음 156.67초, 씨앗 있음 192.72초다. 씨앗 쪽이 23.0% 느려 개선 기준을 충족하지 못했다. 씨앗 레시피·스크립트·테스트는 제거하고 이 조사 기록을 남긴다. 이 측정은 속도 저하의 원인을 입증하지 않는다.
- `fresh-cargo`와 종료 훅은 ADR 0015대로 `--workspace` 정리를 유지한다. 외부 의존성까지 매번 정리하면 커밋·병합·푸시·종료에서 재컴파일 비용이 발생한다. 수동 `just fresh-all`은 외부 의존성까지 지우며 자동 훅에 넣지 않는다. 전체 정리 뒤 `just base=HEAD check`로 가속 없는 로컬 빌드를 확인할 수 있다.
- 기존 `NEXTEST_TEST_THREADS` 환경값 제거 방어는 유지한다. 이 작업에서 시험한 네 레시피의 영구 4스레드 고정과 관련 기존 테스트 기대값은 되돌렸다. 최신 master의 H-52는 nextest 기본 프로필에 4를 고정했으며 병합 시 유지한다.

## 버린 대안

여러 작업 사본이 쓰는 `sccache`는 사용자의 결정으로 버렸다. 여러 주체가 쓰는 공유 캐시를 검사 결과에 쓰면 새 신뢰 경계가 생기며, 이를 서명·검증하는 비용이 이득보다 크다. `CARGO_TARGET_DIR` 하나를 공유하는 방식도 빌드 잠금과 산출물 혼합 때문에 쓰지 않는다.

격리 fixture에서 `sccache 0.19.0`의 의존 함수는 실제로 `0`을 반환했고 테스트는 `1`을 기대했다. 무캐시 테스트는 종료 101과 `left: 0`, `right: 1`로 실패했다. `1`을 반환하는 별도 빌드의 유효한 캐시 아카이브를 원래 의존성 캐시 항목에 덮어쓴 뒤 원래 소스로 테스트하자 새 target에서 종료 0과 테스트 1건 통과가 나왔다. 서버 통계는 Rust 캐시 적중 2, 미스 0이었다. 그 target에서 `cargo clean --workspace --locked --offline` 뒤 래퍼 없이 다시 실행해도 통과했다. workspace만 다시 컴파일되고 외부 의존성 산출물은 남았다. 제품 코드와 실제 `check-fast`에 sccache를 적용한 실험은 아니다.

메인 체크아웃의 `target/`을 사적인 APFS 씨앗으로 복제하는 후보는 새 공유 쓰기 경계를 만들지 않지만 위 실측에서 채택 기준을 만족하지 못했다. 메인 씨앗을 조작하면 씨앗을 쓴 검사 결과가 달라질 수 있으며 이는 메인 체크아웃의 산출물을 신뢰하는 것과 같은 위협이다. 후보는 일반 복사로 넘어가지 않는 시스템 `clonefile`을 사용했다. 릴리스 workspace 산출물과 빌드 스크립트의 메인 절대 경로가 잘못 재사용되는 반례를 확인해 후보의 복제 범위를 제한했으나, 최종 트리에는 씨앗 구현이 없다.

## 알려진 빈틈

- `fresh-cargo`와 종료 훅의 `cargo clean --workspace` 뒤에도 외부 의존성 산출물이 남는다. 위 sccache 오염 fixture에서 정리 후 래퍼 없이 다시 실행해도 잘못된 의존성 결과가 유지됐다. 커밋마다 전체 정리하면 외부 의존성을 반복 컴파일해야 하므로 비용이 크다. 별도 `just fresh-all`로 로컬 전체 정리를 실행할 수 있고, GitHub 호스팅의 새 러너에서 컴파일 산출물을 복원하지 않는 원격 CI와 조건이 충족된 밤 전체 검사는 로컬 오염 산출물을 재사용하지 않는다. 이것이 외부 의존성 전체의 무결성을 증명하지는 않는다.

워크플로 정책은 CI·밤 검사 작업의 호스팅 러너와 단계, 복원 명령·액션 부재를 고정한다. 워크플로가 호출하는 스크립트의 향후 내용 변경이나 레지스트리의 변조까지 이 검사만으로 증명하지 않는다.

## 검증과 측정

실험은 2026-11-18에 각각 새 Git 작업 사본에서 씨앗 없음 → 있음 → 없음 → 있음 순서로 실행했다. 같은 커밋 `730ea4f8bb34824941ef80009922f7a6605d8b97`과 씨앗 스크립트 SHA-256 `907d644b222eb545bae3ad7270d4f3b911bcde75403d917fee0ac35d51d378ba`를 사용했다. 실제 메인 `target/`의 clonefile 스냅숏 하나를 두 씨앗 실험의 공통 원본으로 사용했다. 복제는 일반 복사로 대체하지 않았고 측정 사본과 스냅숏은 즉시 삭제했다. `CARGO_BUILD_JOBS=4`이며 측정한 커밋의 검사 레시피는 nextest를 4로 고정했다. 두 조건의 검사와 씨앗 적용은 모두 종료 0이었다.

| 순서 | 조건      | 검사 시작 시각 (KST) | 1분 load 시작 → 종료 | 검사 시간 | 씨앗 복제·정리 | 검사 직후 `du -sk` | 볼륨 가용 감소: 복제 / 검사 (바이트) |
| ---- | --------- | -------------------- | -------------------- | --------: | -------------: | -----------------: | -----------------------------------: |
| 1    | 씨앗 없음 | 05:16:27             | 41.06 → 45.22        |  203.25초 |           없음 |        907,240 KiB |                없음 / -5,269,790,720 |
| 2    | 씨앗 있음 | 05:20:31             | 39.84 → 101.48       |  244.59초 |        28.01초 |      9,165,728 KiB |            168,095,744 / 338,161,664 |
| 3    | 씨앗 없음 | 05:25:45             | 90.40 → 44.79        |  110.09초 |           없음 |        907,936 KiB |                 없음 / 2,166,489,088 |
| 4    | 씨앗 있음 | 05:28:22             | 72.42 → 53.77        |  140.85초 |        26.09초 |      9,168,004 KiB |         324,407,296 / -4,203,610,112 |

이번 씨앗 검사 중앙값은 192.72초로 무씨앗 156.67초보다 23.0% 느렸다. 복제·정리까지 반올림 전에 각각 더하면 씨앗 실행은 272.59초와 166.94초다. 각 쌍의 시작 load는 41.06 대 39.84, 90.40 대 72.42였지만 실행 중 부하가 크게 바뀌었다. 이 수치만으로 원인이나 다른 작업의 성능을 단정하지 않는다.

가용량 차이는 `statvfs`의 볼륨 가용 바이트를 복제·검사 전후로 뺀 값으로, `df`와 같이 같은 볼륨의 다른 작업을 포함한다. 음수는 그동안 전체 가용량이 늘었다는 뜻이다. 공통 스냅숏 생성 비용은 씨앗 복제 차이에 포함하지 않았다. `du`는 APFS 공유 블록을 중복 계산할 수 있으므로 사본의 실제 물리 증가량으로 읽을 수 없다. 이번 수치로 각 사본의 물리 디스크 증가분을 분리하지 못한다.

측정은 소스 변경이 없는 첫 `check-fast`라 정적 검사와 `xtask` 부트스트랩이 중심이며 제품 crate 변경 검사 시간을 대표하지 않는다. 앞선 단일 측정도 씨앗 없음 98.21초, 있음 233.25초와 복제 21.03초였으나 다른 스크립트 버전, 다른 부하 및 Cargo 잠금 대기가 있어 독립적인 성능 보증이 아니다.

작은 fixture에서 workspace 정리 뒤 외부 의존성 산출물이 남고 `fresh-all` 뒤 사라지는 것을 확인했다. 워크플로 정책 테스트는 CI·밤 검사에 컴파일 캐시 복원 액션, 씨앗 명령 또는 자체 호스팅 러너를 넣는 예제를 거부한다. 프로젝트 신뢰와 세션 훅 정의 신뢰는 각각 확인하지 못했다. 리뷰와 검사 결과는 리뷰 기록을 따른다.

## 인계

H-51: 메인은 병합 뒤 필요할 때 각 작업 사본에서 수동 `just fresh-all` 다음 `just base=HEAD check`로 전체 정리 빌드를 실행하도록 안내한다. `fresh-all`은 `target/`의 기록도 지우므로 필요한 기록은 먼저 밖에 보존한다.

## 손 변형

`mktemp` 사본에서 새 워크플로·정리 회귀 테스트의 대조군을 실행한 뒤, 정식 JSON 문법과 justfile 구문을 확인한 각 변형을 실제 테스트로 실행했다. 변형 28개가 모두 검출됐고 사본은 삭제했다. 워크플로 변형 20개는 기존 정책의 기대값 정확 일치와 허용 목록을 포함한 첫 단언에서 검출됐으며, 이후 정규식만의 독립 검출력을 증명하지 않는다. 로그는 작업 사본의 `target/H-51/policy-mutations/`에 보존한다.

변형 23은 종료 훅의 상수 목록 변경을, 24는 공백 설명의 첫 문장 반전을 검출한다. 27·28은 존재하지 않는 작업 경로에서 sandbox 시작 전 기본 첫 단계 선택만 확인한다. 이 테스트가 실제 Claude·Codex 종료 훅의 성공 실행이나 모든 단계를 입증하지는 않는다. 문구 일부를 검사하는 테스트도 문서 전체의 의미적 무모순을 증명하지 않는다.

| 번호 | 바꾼 것                                                | 잡은 테스트                                                                |
| ---- | ------------------------------------------------------ | -------------------------------------------------------------------------- |
| 1    | .github/workflows/ci.yml linux: self-hosted            | remote checks run on hosted runners without restoring a compiled target    |
| 2    | .github/workflows/ci.yml linux: cache-action           | remote checks run on hosted runners without restoring a compiled target    |
| 3    | .github/workflows/ci.yml linux: seed-command           | remote checks run on hosted runners without restoring a compiled target    |
| 4    | .github/workflows/ci.yml linux: wrapper-env            | remote checks run on hosted runners without restoring a compiled target    |
| 5    | .github/workflows/ci.yml linux: rust-cache-action      | remote checks run on hosted runners without restoring a compiled target    |
| 6    | .github/workflows/ci.yml macos: self-hosted            | remote checks run on hosted runners without restoring a compiled target    |
| 7    | .github/workflows/ci.yml macos: cache-action           | remote checks run on hosted runners without restoring a compiled target    |
| 8    | .github/workflows/ci.yml macos: seed-command           | remote checks run on hosted runners without restoring a compiled target    |
| 9    | .github/workflows/ci.yml macos: wrapper-env            | remote checks run on hosted runners without restoring a compiled target    |
| 10   | .github/workflows/ci.yml macos: rust-cache-action      | remote checks run on hosted runners without restoring a compiled target    |
| 11   | .github/workflows/nightly.yml linux: self-hosted       | remote checks run on hosted runners without restoring a compiled target    |
| 12   | .github/workflows/nightly.yml linux: cache-action      | remote checks run on hosted runners without restoring a compiled target    |
| 13   | .github/workflows/nightly.yml linux: seed-command      | remote checks run on hosted runners without restoring a compiled target    |
| 14   | .github/workflows/nightly.yml linux: wrapper-env       | remote checks run on hosted runners without restoring a compiled target    |
| 15   | .github/workflows/nightly.yml linux: rust-cache-action | remote checks run on hosted runners without restoring a compiled target    |
| 16   | .github/workflows/nightly.yml macos: self-hosted       | remote checks run on hosted runners without restoring a compiled target    |
| 17   | .github/workflows/nightly.yml macos: cache-action      | remote checks run on hosted runners without restoring a compiled target    |
| 18   | .github/workflows/nightly.yml macos: seed-command      | remote checks run on hosted runners without restoring a compiled target    |
| 19   | .github/workflows/nightly.yml macos: wrapper-env       | remote checks run on hosted runners without restoring a compiled target    |
| 20   | .github/workflows/nightly.yml macos: rust-cache-action | remote checks run on hosted runners without restoring a compiled target    |
| 21   | workspace-only                                         | the known dependency-artifact gap is listed and full cleanup is manual     |
| 22   | missing-clean                                          | the known dependency-artifact gap is listed and full cleanup is manual     |
| 23   | `FRESH_STEPS`의 첫 정리 명령에서 `--workspace` 제거    | the known dependency-artifact gap is listed and full cleanup is manual     |
| 24   | 공백 첫 문장을 산출물이 지워진다는 뜻으로 반전         | the known dependency-artifact gap is listed and full cleanup is manual     |
| 25   | 공백 항목의 `fresh-all` 안내 제거                      | the known dependency-artifact gap is listed and full cleanup is manual     |
| 26   | 공백 항목의 무결성 한계를 증명한다는 뜻으로 반전       | the known dependency-artifact gap is listed and full cleanup is manual     |
| 27   | 종료 검사 기본 단계 목록을 빈 배열로 변경              | the default stop check selects workspace cleanup before starting a sandbox |
| 28   | 종료 검사 기본 단계 중 첫 단계를 제거                  | the default stop check selects workspace cleanup before starting a sandbox |
