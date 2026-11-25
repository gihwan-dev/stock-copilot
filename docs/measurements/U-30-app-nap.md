# U-30 측정 기록: App Nap (S-10)

- 작업: [tasks.md](../tasks.md) U-30, 실측 [open-issues.md](../open-issues.md) S-10
- 상태: **측정 중, 결과 기록은 끝난 뒤**
- 도구: [app-probe.swift](app-probe.swift)의 `nap` 명령

## 방법

```sh
swiftc -O docs/measurements/app-probe.swift -o <임시>/app-probe
nohup <임시>/app-probe nap 10 60 > <임시>/u30-nap.csv &
```

1분마다 한 줄을 남긴다: 시각, 앱 생존, 화면에 올라온 앱 창 수, 그 1분 동안 앱 묶음(앱 + WebKit 자식)의 CPU 초, 유휴 깨움 수(`ri_pkg_idle_wkups`), 인터럽트 깨움 수, 측정 도구 자신의 `sleep` 초과 시간.

- 스케줄러는 30초마다 깬다(`scheduler::engine::DEFAULT_TICK_INTERVAL_SECONDS`). App Nap이 타이머를 미루면 창을 숨긴 구간에서 깨움 수가 눈에 띄게 줄고 CPU 초가 0에 붙는다.
- 앱에는 예약 작업의 예정·실제 시각을 남기는 로그가 없다. 실행 기록은 암호화된 DB(`job_execution_records`)에만 있고, 에이전트는 실제 계좌 자료가 든 DB를 열지 않는다. 그래서 이 도구는 간접 지표(깨움·CPU)만 잰다.
- 측정 도구 자신의 `sleep` 초과는 시스템 잠자기(덮개 닫힘 등)를 가려내는 데 쓴다. 초과가 크면 그 구간은 App Nap이 아니라 시스템 잠자기다.

## 이번 묶음에서 시작한 측정

- 시작: 2026-11-25, 10시간, 1분 간격. 출력은 측정 세션의 임시 폴더 `u30-nap.csv`(경로는 묶음 보고에 적음).
- 창 숨기기는 자동화하지 못했다. `NSRunningApplication.hide()`는 거부됐고(메뉴 막대 상주 앱), 창 닫기 버튼을 누르는 손쉬운 사용 권한과 System Events 자동화 권한이 측정 세션에 없다. **사용자가 주 창을 닫아(빨간 버튼, 메뉴 막대 상주로 숨김) 측정이 유효해진다.** 로그의 `onscreen_windows`가 0이 된 시각부터 8시간 이상이 유효 구간이다.

## 끝난 뒤 할 일

1. `onscreen_windows`가 0인 구간과 1인 구간의 분당 깨움 수·CPU 초를 비교한다.
2. 토스 키를 연결한 상태라면, 앱의 예약 작업 화면(또는 DB의 `job_execution_records`)에서 숨긴 구간의 작업마다 예정 시각과 실제 시각 차이를 본다. 차이가 5분을 넘는 작업이 하나라도 있으면 대응 작업을 추가한다(tasks.md U-30 완료 기준).
3. 결과 표를 아래에 채운다.

| 구간 | 분 수 | 분당 유휴 깨움 중앙값 | 분당 CPU 초 중앙값 | 작업별 최대 지연 |
|---|---|---|---|---|
| 창 보임 | | | | |
| 창 숨김 | | | | |
