use std::net::IpAddr;
use std::str::FromStr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use domain::{Nanosecond, OffsetSeconds, SpanSeconds, TimeSpan, Timestamp, UnixSeconds};
use testkit_ports::SteppedClock;

use super::{
    CopyTextRequest, LookupFailed, PublicIpLookup, PublicIpRuntime, TextClipboard, UiCopyStatus,
    copy_reply, public_ip_reply,
};
use crate::ipc::error::IpcErrorKind;
use crate::ipc::state::BoxFuture;

#[derive(Default)]
struct CountingLookup {
    calls: AtomicUsize,
    replies: RwLock<Vec<Result<IpAddr, LookupFailed>>>,
}

impl PublicIpLookup for CountingLookup {
    fn lookup(&self) -> BoxFuture<'_, Result<IpAddr, LookupFailed>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let reply = self
            .replies
            .write()
            .map_or(Err(LookupFailed), |mut replies| {
                replies.pop().unwrap_or(Err(LookupFailed))
            });
        Box::pin(std::future::ready(reply))
    }
}

#[derive(Default)]
struct RecordingClipboard(RwLock<Vec<String>>);

impl TextClipboard for RecordingClipboard {
    fn copy(&self, text: String) -> BoxFuture<'_, Result<(), LookupFailed>> {
        let stored = self
            .0
            .write()
            .map(|mut copied| copied.push(text))
            .map_err(|_poison| LookupFailed);
        Box::pin(std::future::ready(stored))
    }
}

fn start() -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(1_795_564_800),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("time")
}

fn seconds(count: u64) -> TimeSpan {
    TimeSpan::try_new(SpanSeconds::new(count), Nanosecond::new(0)).expect("span")
}

fn ip(text: &str) -> IpAddr {
    IpAddr::from_str(text).expect("ip")
}

struct Fixture {
    lookup: Arc<CountingLookup>,
    clipboard: Arc<RecordingClipboard>,
    clock: Arc<SteppedClock>,
    runtime: PublicIpRuntime,
}

fn fixture(replies: Vec<Result<IpAddr, LookupFailed>>) -> Fixture {
    let lookup = Arc::new(CountingLookup {
        calls: AtomicUsize::new(0),
        replies: RwLock::new(replies.into_iter().rev().collect()),
    });
    let clipboard = Arc::new(RecordingClipboard::default());
    let clock = Arc::new(SteppedClock::new(start()));
    let runtime = PublicIpRuntime::new(
        Arc::<CountingLookup>::clone(&lookup),
        Arc::<RecordingClipboard>::clone(&clipboard),
        Arc::<SteppedClock>::clone(&clock),
    );
    Fixture {
        lookup,
        clipboard,
        clock,
        runtime,
    }
}

#[test]
fn public_ip_is_asked_at_most_once_a_minute() {
    let fx = fixture(vec![Ok(ip("203.0.114.7")), Ok(ip("203.0.114.8"))]);
    tauri::async_runtime::block_on(async {
        let first = public_ip_reply(Some(&fx.runtime)).await.expect("ip");
        assert_eq!(first.address, "203.0.114.7");
        fx.clock.advance_by(seconds(59)).expect("advance");
        let cached = public_ip_reply(Some(&fx.runtime)).await.expect("ip");
        assert_eq!(cached.address, "203.0.114.7");
        assert_eq!(fx.lookup.calls.load(Ordering::SeqCst), 1);
        fx.clock.advance_by(seconds(1)).expect("advance");
        let next = public_ip_reply(Some(&fx.runtime)).await.expect("ip");
        assert_eq!(next.address, "203.0.114.8");
        assert_eq!(fx.lookup.calls.load(Ordering::SeqCst), 2);
    });
}

#[test]
fn a_failed_lookup_is_retryable_and_is_not_retried_within_the_minute() {
    let fx = fixture(vec![Err(LookupFailed), Ok(ip("203.0.114.7"))]);
    tauri::async_runtime::block_on(async {
        for _ in 0_u8..3_u8 {
            let error = public_ip_reply(Some(&fx.runtime)).await.expect_err("fails");
            assert_eq!(error.kind, IpcErrorKind::Retryable);
        }
        assert_eq!(fx.lookup.calls.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn without_the_runtime_nothing_is_asked() {
    let error = tauri::async_runtime::block_on(public_ip_reply(None)).expect_err("not assembled");
    assert_eq!(error.kind, IpcErrorKind::NoData);
}

#[test]
fn copy_accepts_only_an_ip_address_and_puts_it_on_the_clipboard() {
    let fx = fixture(Vec::new());
    tauri::async_runtime::block_on(async {
        let copied = copy_reply(
            Some(&fx.runtime),
            CopyTextRequest {
                text: String::from(" 2606:4700:4700::1111 "),
            },
        )
        .await
        .expect("copied");
        assert_eq!(copied.status, UiCopyStatus::Copied);
        let rejected = copy_reply(
            Some(&fx.runtime),
            CopyTextRequest {
                text: String::from("rm -rf /"),
            },
        )
        .await
        .expect_err("rejected");
        assert_eq!(rejected.kind, IpcErrorKind::Bug);
    });
    assert_eq!(
        fx.clipboard.0.read().expect("copied").clone(),
        vec![String::from("2606:4700:4700::1111")]
    );
}
