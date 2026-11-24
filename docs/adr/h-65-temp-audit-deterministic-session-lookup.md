# ADR h-65: 임시 폴더 감사 테스트의 가짜 셸 실행 지연 제거와 주입 가능한 세션 조회

- 상태: 초안 (2026-11-24)
- 관련 작업: H-65 (연관 H-53, ADR 0139)
- 반영한 파일: `harness/ci/temp-audit.mjs`, `harness/ci/tests/h53-temp.test.mjs`

## 배경

`harness/ci/tests/h53-temp.test.mjs`의 `"소유 세션 조회 결과가 불완전하면 감사는 실패한다"` 테스트가 다른 작업이 함께 도는 기기에서 내부 `spawnSync` 10초 제한에 걸려 간헐 실패했다(단독 실행 7.1초, D-58 병합 검사에서 10.09초로 1회 실패).

## 원인 실측

macOS 26.6.2(Build 25G83)에서 `temp-audit.mjs`와 `session-groups.py`, 그리고 문제 테스트의 각 구간을 분리 실측했다.

| 구간 | 유휴 실행 | 4코어 CPU 부하 하 | 관찰 |
|---|---:|---:|---|
| `session-groups.py` 단독 실행 | 40.3–76.2ms | — | Python 세션 조회 자체는 느리지 않음 |
| 실제 `python3`로 `temp-audit.mjs` 실행 | 110–158ms | 115–130ms | 실제 조회 경로는 10초 제한과 무관하게 빠름 |
| `python3` 부재 시 `temp-audit.mjs` 실행 | 51.9–60.8ms | — | 실패 경로 즉시 반환 |
| 런타임 작성 `#!/bin/sh` 가짜 `python3`의 첫 `execve` | 5,343–7,805ms (`/tmp` 7,224.1ms, `target/` 6,649.7ms) | >10,000ms 도달 가능 | 새로 쓴 파일의 `com.apple.provenance` 확장 속성 때문에 첫 실행이 `syspolicyd`/Gatekeeper/XProtect 동기 평가에 걸림 |
| 동일 inode의 두 번째 `execve` | 4.8–14.1ms | — | 평가 캐시 후 즉시 실행 |
| OS 서명 `/usr/bin/true` 심볼릭 링크 실행 | 3.9–9.7ms (직접) / 57.7–69.2ms (`temp-audit.mjs`) | 59.4–63.6ms (`temp-audit.mjs`) | 서명된 시스템 바이너리는 평가 지연 없음 |

참고로 macOS BSD `/bin/ps -A -o pid=,pgid=,sess=`는 커널 포인터 마스킹으로 `sess` 열에 `0`을 출력하므로, `os.getsid`와 `os.getpgid`를 쓰는 `session-groups.py` 자체는 macOS에서 제거할 수 없다.

## 결정

| 결정 | 근거 | 버린 대안 |
|---|---|---|
| 기존 테스트 `"소유 세션 조회 결과가 불완전하면 감사는 실패한다"`에서 런타임 `#!/bin/sh` 스크립트 작성 대신 OS 서명 바이너리(`/usr/bin/true`, Linux는 `/bin/true`)로 심볼릭 링크를 만들어 빈 stdout·종료 코드 0을 재현한다 | `spawnSync`의 `timeout: 10000`과 실패 단언 3건을 그대로 유지하면서, 실제 `temp-audit.mjs` 프로세스 기동과 `PATH` 경유 `python3` 조회 실패 경로를 그대로 검증한다. 실행 시간이 7.1초에서 62.8ms(부하 하 59.4–63.6ms)로 줄어든다 | `timeout` 상향(작업 요구사항 위반, 근본 원인 미해결); `/bin/ps`로 `session-groups.py` 교체(macOS에서 `sess=0`이라 세션 판별 불가); 기존 실제 경로 테스트 삭제(실제 `PATH`/서브프로세스 경계 검증 상실) |
| `harness/ci/temp-audit.mjs`에서 `parseSessionGroupsOutput`, `querySessionGroups`, `runTempAudit(command, args, options)`를 내보내 `inspectSessionGroups`·`signalGroup`·`wait`를 주입 가능하게 분리하고, 직접 CLI 실행 시 기존 동작을 그대로 유지한다 | OS 부하나 외부 `python3` 기동 없이 불완전 응답(빈 출력·비숫자·종료 실패·비0 상태)과 잔존 세션 그룹 종료(`SIGTERM` → `SIGKILL`) 경로를 밀리초 단위로 결정적으로 검증할 수 있다. 실제 `python3` 경로를 시험하는 기존 테스트들은 모두 유지된다 | 전역 환경 변수로 테스트 분기 삽입(제품 코드 오염 및 테스트 인식 규칙 위반) |

## 검증

- `node --test --test-reporter=spec harness/ci/tests/h53-temp.test.mjs`: 15/15 통과.
  - `"소유 세션 조회 결과가 불완전하면 감사는 실패한다"`: **43.63–62.8ms** (동시 부하 환경 포함; 기존 7.1초 / 부하 시 10.09초).
  - 신규 결정적 테스트 3건(`parseSessionGroupsOutput` 파싱 오류, 주입 `inspectSessionGroups` 불완전 결과 실패, 주입 `inspectSessionGroups`+`signalGroup` 잔존 그룹 종료): **0.58ms / 9.53ms / 5.71ms**.
- `node --test 'harness/ci/tests/*.test.mjs'`: 167/167 통과.
- Prettier 및 ESLint(`--max-warnings 0`) 통과.

### 격리 복사본 손 변형 (H-65 범위 5/5 검출)

| 번호 | 대상 파일 | 무력화한 조건 | 검출 테스트 | 결과 |
|---|---|---|---|---|
| M01 | `harness/ci/temp-audit.mjs` | `parseSessionGroupsOutput`: 비0 종료 상태 거부 제거 | `h53-temp.test.mjs` | 검출 |
| M02 | `harness/ci/temp-audit.mjs` | `parseSessionGroupsOutput`: 첫 줄 `"ok"` 표식 누락(빈 stdout) 거부 제거 | `h53-temp.test.mjs` | 검출 |
| M03 | `harness/ci/temp-audit.mjs` | `parseSessionGroupsOutput`: 비숫자/기호(`+123`, `0x10`, `1e2`, 공백) 출력 행 거부 제거 | `h53-temp.test.mjs` | 검출 |
| M04 | `harness/ci/temp-audit.mjs` | `runTempAudit`: 불완전 세션 조회(`undefined`) 실패 처리 제거 | `h53-temp.test.mjs` | 검출 |
| M05 | `harness/ci/temp-audit.mjs` | `runTempAudit`: 잔존 자손 그룹 `SIGKILL` 전송 제거 | `h53-temp.test.mjs` | 검출 |

## 남는 한계

- `symlinkSync` 테스트 경로는 호스트에 `/usr/bin/true` 또는 `/bin/true`가 존재한다고 가정한다(표준 macOS·Linux 환경 모두 충족).
