use std::sync::Arc;

use tokio::sync::Mutex;

use super::{
    BuildKind, CheckStart, FoundUpdate, InstallOutcome, PUBKEY_PLACEHOLDER, ReleaseNotes,
    ScheduleStep, SourceFuture, UpdateCenter, UpdateFailure, UpdateMode, UpdatePhase, UpdateSource,
    UpdateVersion, mode_for, run_schedule,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Call {
    Check,
    Download,
    Install,
}

#[derive(Debug, Clone)]
enum Offer {
    Nothing,
    Version(&'static str),
    CheckFails,
    DownloadFails(&'static str),
    InstallFails(&'static str),
}

#[derive(Debug)]
struct FakeSource {
    offer: Offer,
    calls: Mutex<Vec<Call>>,
}

impl FakeSource {
    fn new(offer: Offer) -> Arc<Self> {
        Arc::new(Self {
            offer,
            calls: Mutex::new(Vec::new()),
        })
    }

    fn record(&self, call: Call) {
        if let Ok(mut calls) = self.calls.try_lock() {
            calls.push(call);
        }
    }

    fn calls(&self) -> Vec<Call> {
        match self.calls.try_lock() {
            Ok(calls) => calls.clone(),
            Err(_busy) => Vec::new(),
        }
    }
}

fn found(version: &str) -> FoundUpdate {
    FoundUpdate {
        version: UpdateVersion::new(version),
        notes: ReleaseNotes::new(format!("{version} 변경 사항")),
    }
}

impl UpdateSource for FakeSource {
    fn check(&self) -> SourceFuture<'_, Option<FoundUpdate>> {
        self.record(Call::Check);
        let result = match self.offer {
            Offer::Nothing => Ok(None),
            Offer::CheckFails => Err(UpdateFailure::Check),
            Offer::Version(version)
            | Offer::DownloadFails(version)
            | Offer::InstallFails(version) => Ok(Some(found(version))),
        };
        Box::pin(async move { result })
    }

    fn download(&self) -> SourceFuture<'_, ()> {
        self.record(Call::Download);
        let result = match self.offer {
            Offer::DownloadFails(_) => Err(UpdateFailure::Download),
            Offer::Nothing | Offer::CheckFails | Offer::Version(_) | Offer::InstallFails(_) => {
                Ok(())
            }
        };
        Box::pin(async move { result })
    }

    fn install(&self) -> Result<(), UpdateFailure> {
        self.record(Call::Install);
        match self.offer {
            Offer::InstallFails(_) => Err(UpdateFailure::Install),
            Offer::Nothing | Offer::CheckFails | Offer::Version(_) | Offer::DownloadFails(_) => {
                Ok(())
            }
        }
    }
}

fn center(mode: UpdateMode, source: &Arc<FakeSource>) -> UpdateCenter {
    let shared: Arc<dyn UpdateSource> = Arc::<FakeSource>::clone(source);
    UpdateCenter::new(mode, shared)
}

fn block<T>(future: impl Future<Output = T>) -> T {
    tauri::async_runtime::block_on(future)
}

#[test]
fn found_update_is_downloaded_in_background_and_becomes_ready() {
    let source = FakeSource::new(Offer::Version("0.2.0"));
    let center = center(UpdateMode::Enabled, &source);
    assert_eq!(block(center.phase()), UpdatePhase::Idle);
    assert_eq!(block(center.check()), UpdatePhase::Ready(found("0.2.0")));
    assert_eq!(source.calls(), vec![Call::Check, Call::Download]);
}

#[test]
fn no_update_reports_up_to_date_without_download() {
    let source = FakeSource::new(Offer::Nothing);
    let center = center(UpdateMode::Enabled, &source);
    assert_eq!(block(center.check()), UpdatePhase::UpToDate);
    assert_eq!(source.calls(), vec![Call::Check]);
}

#[test]
fn check_and_download_failures_are_reported_and_retried_next_time() {
    let failing_check = FakeSource::new(Offer::CheckFails);
    let first = center(UpdateMode::Enabled, &failing_check);
    assert_eq!(
        block(first.check()),
        UpdatePhase::Failed(UpdateFailure::Check)
    );
    assert_eq!(
        block(first.check()),
        UpdatePhase::Failed(UpdateFailure::Check)
    );
    assert_eq!(failing_check.calls(), vec![Call::Check, Call::Check]);

    let failing_download = FakeSource::new(Offer::DownloadFails("0.3.0"));
    let second = center(UpdateMode::Enabled, &failing_download);
    assert_eq!(
        block(second.check()),
        UpdatePhase::Failed(UpdateFailure::Download)
    );
}

#[test]
fn disabled_center_never_touches_the_source() {
    let source = FakeSource::new(Offer::Version("0.2.0"));
    let center = center(UpdateMode::Disabled, &source);
    assert_eq!(block(center.begin()), CheckStart::Skipped);
    assert_eq!(block(center.check()), UpdatePhase::Off);
    assert_eq!(
        block(center.install_ready()),
        InstallOutcome::NothingReady(UpdatePhase::Off)
    );
    assert_eq!(source.calls(), Vec::<Call>::new());
}

#[test]
fn second_check_is_skipped_while_one_is_running_or_ready() {
    let source = FakeSource::new(Offer::Version("0.2.0"));
    let center = center(UpdateMode::Enabled, &source);
    assert_eq!(block(center.begin()), CheckStart::Started);
    assert_eq!(block(center.phase()), UpdatePhase::Checking);
    assert_eq!(block(center.begin()), CheckStart::Skipped);
    assert_eq!(block(center.run()), UpdatePhase::Ready(found("0.2.0")));
    assert_eq!(block(center.check()), UpdatePhase::Ready(found("0.2.0")));
    assert_eq!(source.calls(), vec![Call::Check, Call::Download]);
}

#[test]
fn install_happens_only_for_a_ready_update() {
    let source = FakeSource::new(Offer::Version("0.2.0"));
    let center = center(UpdateMode::Enabled, &source);
    assert_eq!(
        block(center.install_ready()),
        InstallOutcome::NothingReady(UpdatePhase::Idle)
    );
    assert_eq!(source.calls(), Vec::<Call>::new());
    let _ready = block(center.check());
    assert_eq!(
        block(center.install_ready()),
        InstallOutcome::Installed(UpdateVersion::new("0.2.0"))
    );
    assert_eq!(
        source.calls(),
        vec![Call::Check, Call::Download, Call::Install]
    );
}

#[test]
fn failed_install_is_reported_and_leaves_ready_state() {
    let source = FakeSource::new(Offer::InstallFails("0.2.0"));
    let center = center(UpdateMode::Enabled, &source);
    let _ready = block(center.check());
    assert_eq!(
        block(center.install_ready()),
        InstallOutcome::Failed(UpdateFailure::Install)
    );
    assert_eq!(
        block(center.phase()),
        UpdatePhase::Failed(UpdateFailure::Install)
    );
}

#[test]
fn schedule_checks_once_at_start_and_again_after_each_wait() {
    let source = FakeSource::new(Offer::Nothing);
    let center = Arc::new(center(UpdateMode::Enabled, &source));
    let waits = Arc::new(Mutex::new(0_u8));
    let shared = Arc::clone(&waits);
    block(run_schedule(Arc::clone(&center), move || {
        let counter = Arc::clone(&shared);
        async move {
            let mut count = counter.lock().await;
            *count = count.saturating_add(1);
            if *count >= 3 {
                ScheduleStep::Stop
            } else {
                ScheduleStep::Continue
            }
        }
    }));
    assert_eq!(source.calls(), vec![Call::Check, Call::Check, Call::Check]);
}

#[test]
fn updates_are_off_in_debug_builds_and_without_a_real_public_key() {
    assert_eq!(
        mode_for(BuildKind::Debug, Some("dW50cnVzdGVkIGtleQ==")),
        UpdateMode::Disabled
    );
    assert_eq!(mode_for(BuildKind::Release, None), UpdateMode::Disabled);
    assert_eq!(
        mode_for(BuildKind::Release, Some("  ")),
        UpdateMode::Disabled
    );
    assert_eq!(
        mode_for(BuildKind::Release, Some(PUBKEY_PLACEHOLDER)),
        UpdateMode::Disabled
    );
    assert_eq!(
        mode_for(BuildKind::Release, Some("dW50cnVzdGVkIGtleQ==")),
        UpdateMode::Enabled
    );
}
