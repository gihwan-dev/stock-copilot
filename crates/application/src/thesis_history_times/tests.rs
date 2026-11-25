use std::future::Future;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll, Waker};

use domain::{
    ChangeReason, Claim, ClaimContent, ClaimId, ClaimRole, Evidence, EvidenceRef, EvidenceUrl,
    InstrumentId, Nanosecond, NormalizedUrl, OffsetSeconds, ProposedChange, Thesis,
    ThesisOperation, ThesisPatch, ThesisVersion, Timestamp, UnixSeconds, UntrustedText,
};

use super::{ThesisEntryTime, ThesisHistoryTimes, TimedThesisRepository};
use crate::{Clock, SystemFailure, SystemPort, SystemPortError, ThesisRepository, WriteCompleted};

fn run<F: Future>(future: F) -> F::Output {
    let mut pinned = std::pin::pin!(future);
    match pinned
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("동기 테스트 퓨처"),
    }
}

fn at(seconds: i64) -> Timestamp {
    Timestamp::from_parts(
        UnixSeconds::new(seconds),
        Nanosecond::new(0),
        OffsetSeconds::new(0),
    )
    .expect("시각")
}

fn text(value: &str) -> UntrustedText {
    UntrustedText::try_new(value).expect("문장")
}

fn add_claim(thesis: &mut Thesis, id: &str) {
    let claim = Claim::new(
        ClaimId::try_new(id).expect("id"),
        ClaimRole::Core,
        ClaimContent::try_new(
            text("statement"),
            text("metric"),
            text("expected"),
            text("observe"),
            text("refutation"),
        )
        .expect("주장"),
    );
    let evidence = Evidence::try_new(vec![EvidenceRef::Web(
        EvidenceUrl::try_new(NormalizedUrl::try_new("https://example.com/a").expect("url"))
            .expect("근거"),
    )])
    .expect("근거");
    let reason = ChangeReason::try_new(text("reason"), text("model"), text("v1")).expect("이유");
    thesis
        .apply_batch(
            thesis.version(),
            ThesisPatch::try_new(vec![ProposedChange::new(
                ThesisOperation::AddClaim(claim),
                evidence,
                reason,
            )])
            .expect("변경"),
        )
        .expect("적용");
}

#[derive(Default)]
struct Stored {
    fail: RwLock<Option<SystemPortError>>,
}

impl ThesisRepository for Stored {
    fn store(
        &self,
        _instrument: &InstrumentId,
        _thesis: &Thesis,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let failure = *self.fail.read().expect("잠금");
        std::future::ready(failure.map_or(Ok(WriteCompleted), Err))
    }

    fn load(
        &self,
        _instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Option<Thesis>, SystemPortError>> + Send {
        std::future::ready(Ok(None))
    }
}

#[derive(Default)]
struct Times(RwLock<Vec<(InstrumentId, ThesisEntryTime)>>);

impl ThesisHistoryTimes for Times {
    fn record_times(
        &self,
        instrument: &InstrumentId,
        sequences: &[ThesisVersion],
        recorded_at: Timestamp,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        let mut rows = self.0.write().expect("잠금");
        for sequence in sequences {
            if !rows
                .iter()
                .any(|(owner, time)| owner == instrument && time.sequence == *sequence)
            {
                rows.push((
                    instrument.clone(),
                    ThesisEntryTime {
                        sequence: *sequence,
                        recorded_at,
                    },
                ));
            }
        }
        std::future::ready(Ok(WriteCompleted))
    }

    fn entry_times(
        &self,
        instrument: &InstrumentId,
    ) -> impl Future<Output = Result<Vec<ThesisEntryTime>, SystemPortError>> + Send {
        let rows = self
            .0
            .read()
            .expect("잠금")
            .iter()
            .filter(|(owner, _)| owner == instrument)
            .map(|(_, time)| *time)
            .collect();
        std::future::ready(Ok(rows))
    }
}

struct Ticking(RwLock<i64>);

impl Clock for Ticking {
    fn now(&self) -> Timestamp {
        at(*self.0.read().expect("잠금"))
    }
}

#[test]
fn storing_a_thesis_records_the_time_of_new_entries_and_keeps_older_times() {
    let repository = Arc::new(Stored::default());
    let times = Arc::new(Times::default());
    let clock = Arc::new(Ticking(RwLock::new(100)));
    let decorated = TimedThesisRepository::new(
        Arc::clone(&repository),
        Arc::clone(&times),
        Arc::clone(&clock),
    );
    let instrument = InstrumentId::try_new("inst-1").expect("종목");
    let mut thesis = Thesis::new();
    add_claim(&mut thesis, "claim-1");
    run(decorated.store(&instrument, &thesis)).expect("저장");
    *clock.0.write().expect("잠금") = 200;
    add_claim(&mut thesis, "claim-2");
    run(decorated.store(&instrument, &thesis)).expect("다시 저장");
    let recorded = run(times.entry_times(&instrument)).expect("시각");
    assert_eq!(
        recorded
            .iter()
            .map(|time| time.recorded_at)
            .collect::<Vec<_>>(),
        vec![at(100), at(200)]
    );
    assert_eq!(run(decorated.load(&instrument)).expect("읽기"), None);
}

#[test]
fn a_failed_store_records_no_time() {
    let repository = Arc::new(Stored::default());
    *repository.fail.write().expect("잠금") = Some(SystemPortError::new(
        SystemPort::Repository,
        SystemFailure::InvalidData,
    ));
    let times = Arc::new(Times::default());
    let decorated = TimedThesisRepository::new(
        Arc::clone(&repository),
        Arc::clone(&times),
        Arc::new(Ticking(RwLock::new(100))),
    );
    let instrument = InstrumentId::try_new("inst-1").expect("종목");
    let mut thesis = Thesis::new();
    add_claim(&mut thesis, "claim-1");
    let _failed = run(decorated.store(&instrument, &thesis)).expect_err("실패");
    assert_eq!(
        run(times.entry_times(&instrument)).expect("시각"),
        Vec::new()
    );
}
