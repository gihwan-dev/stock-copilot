# G-09 측정 기록: 성능 (PERF-1~5)

- 작업: [tasks.md](../tasks.md) G-09, 기준 [design.md](../design.md) 7.4
- 기록일: 2026-11-25 (fin-measure 묶음)
- 대상: 설치 앱 `/Applications/Stock Copilot.app` v0.2.0 (Developer ID 서명), Apple Silicon, macOS 27.2
- 도구: [app-probe.swift](app-probe.swift)

```sh
swiftc -O docs/measurements/app-probe.swift -o <임시>/app-probe
<임시>/app-probe startup 10
<임시>/app-probe idle 600 10
```

## 요약

| 항목 | 기준 | 결과 | 판정 |
|---|---|---|---|
| PERF-1 시작 → 주 화면 표시 | 2초 이하 | 10회, 중앙값 0.467초, 최대 0.584초 | 통과 (토스 키 없는 상태) |
| PERF-2 채팅 첫 글자 | 5초 이하 | 미측정 | 사용자 키 연결 후 |
| PERF-3 본인 확인 통과 → 토스 전송 | 1초 이하 | 미측정 | 사용자 키 연결 후 |
| PERF-4 수집·예약 작업이 UI를 멈추지 않음 | U-31 테스트 | 이 기록의 범위 밖 | U-31 |
| PERF-5 대기 CPU·메모리 | CPU 2% 미만, 메모리 300MB 미만 | 10분 평균 CPU 0.037%, 최대 119.3MB | 통과 (토스 키 없는 상태) |

## PERF-1: 시작 → 주 화면 표시

방법: 앱을 SIGTERM으로 끝내고 3초 쉰 뒤 `open -a`를 부른 시각에서, 앱 프로세스가 생긴 시각과 그 프로세스의 창이 화면에 올라온 시각(`CGWindowListCopyWindowInfo`, 레이어 0, 5ms 간격 확인)까지를 잰다. 주 창은 `tauri.conf.json`에서 `visible: false`로 만들어지고, 코어가 저장된 데이터로 `MainScreenData`를 만든 뒤에야 `show_prepared_main`이 창을 띄운다(`apps/desktop/src-tauri/src/lib.rs`). 그래서 창이 화면에 올라온 시각을 "저장된 데이터로 주 화면을 그린 시각"의 근사로 쓴다. 웹뷰의 첫 그리기는 창 표시와 거의 같거나 조금 뒤다.

| 회 | 프로세스 생성(초) | 창 표시(초) |
|---|---|---|
| 1 | 0.069 | 0.584 |
| 2 | 0.068 | 0.472 |
| 3 | 0.068 | 0.447 |
| 4 | 0.066 | 0.453 |
| 5 | 0.066 | 0.465 |
| 6 | 0.067 | 0.453 |
| 7 | 0.065 | 0.467 |
| 8 | 0.068 | 0.425 |
| 9 | 0.071 | 0.548 |
| 10 | 0.067 | 0.472 |

최소 0.425, 중앙값 0.467, 평균 0.479, 최대 0.584초. 첫 회만 조금 느리다(디스크 캐시).

한계: 토스 키를 연결하지 않아 저장된 보유·시세가 없는 상태다. 키 연결 뒤 저장 데이터가 쌓이면 다시 잰다.

## PERF-5: 대기 10분

방법: PERF-1 직후 창을 띄운 채로 손대지 않고 600초 동안 10초마다, 앱 프로세스와 그 앱이 "책임 프로세스"인 자식(WebKit WebContent·GPU·Networking 등, `responsibility_get_pid_responsible_for_pid`) 전체의 CPU 시간(`proc_pid_rusage`의 user+system)과 물리 메모리 사용량(`ri_phys_footprint`)을 합해 잰다. 평균 CPU는 600초 동안 늘어난 CPU 시간 ÷ 600초다.

| 값 | 결과 |
|---|---|
| 프로세스 수 | 6 (앱, WebContent, GPU, Networking, SetStoreUpdateService, audio SandboxHelper) |
| 평균 CPU | 0.037% |
| 10초 구간 최대 CPU | 0.444% |
| 최대 메모리 | 119.3MB (끝 무렵 108~110MB) |

한계: 창이 보이는 상태이고 토스 키·AI 실행이 없는 대기다. 키 연결 뒤 예약 동기화가 도는 상태로 다시 잰다.

## 사용자 키 연결 후 할 것

- PERF-2: 채팅 창에서 질문을 보낸 시각부터 첫 글자가 보이는 시각까지 10회. 화면 녹화(초당 60프레임)로 보낸 프레임과 첫 글자 프레임 차이를 센다.
- PERF-3: 주문 승인에서 본인 확인(Touch ID)을 통과한 시각부터 토스 전송 요청까지 10회. 소액 실주문 검증(O-15)과 함께 하고 감사 로그의 두 시각 차이로 잰다.
- PERF-1·5 재측정: 저장 데이터가 쌓이고 예약 동기화가 도는 상태에서 위 도구로 다시 잰다.
