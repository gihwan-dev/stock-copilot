# ADR: 앱 상주·플랫폼·스케줄러 묶음 결정 표

- 상태: 확정 (ADR 0173 경량 절차, 2026-11-24)

| 작업 | 정한 것 | 다른 선택지 |
|---|---|---|
| J-01 | `ScheduledJobId`를 닫힌 열거형으로 정의하고 `BTreeMap` 레지스트리에 등록. 벽시계 비교 루프는 30초 기본 짧은 틱으로 `Clock::now()`와 비교하며 `CatchUpOnce`는 며칠을 건너뛰어도 1회만 따라잡고 `Skip`은 60초 유예 창 초과 시 건너뜀. AI-06은 `AiJobQueue` 포트로 분리 | 자유 문자열 작업 ID, `Instant` 기반 작업별 대기 타이머 |
| U-18 | 단일 실행을 외부 패키지 없이 `std::os::unix::net` 0700 폴더 안 0600 소켓(104바이트 제한)으로 구현. `main` 창 `CloseRequested`는 `Lifecycle::phase() == Running`이고 `QuitMode::KeepRunningOnClose`일 때만 숨김으로 막고, 명시적 종료(`ResidencyState::begin_quit`) 또는 `Stopping`/`Stopped`에서는 닫기를 허용. `MockRuntime::exit`이 `unimplemented!()`이므로 `tests/exit.rs`는 `begin_quit` 뒤 `main` 창 닫기로 실제 `ExitRequested`→정리 시퀀스를 유지. `tray-icon`이 잠금 파일에 없어 OS 메뉴 막대 아이콘 생성은 `TrayController` 포트·`UnavailableTrayController`로 고정하고 실제 아이콘 바인딩은 건너뜀 | `tauri-plugin-single-instance`·`tray-icon` 외부 패키지 추가(금지), `AppHandle::exit` 직접 호출(MockRuntime 미구현) |
| U-14 | `FilteredNotifier<D>`로 3종 알림(`NewSignal`·`UrgentDisclosure`·`BriefingFinished`)의 한국어 문구·종류별 켜기/끄기 필터와 `NotificationEventSubscriber`(`SignalRaised`·긴급 `DisclosureTriggerRecorded` 구독, `AsyncRuntimeNotificationDispatch` 비동기 송출)를 구현. `tauri-plugin-notification`이 잠금 파일에 없어 실제 macOS 알림 배너 송출은 `UnavailableNotificationDelivery`로 고정하고 건너뜀 | `tauri-plugin-notification` 외부 패키지 추가(금지), `EventSubscriber` 내부 `block_on` 동기 호출 |
