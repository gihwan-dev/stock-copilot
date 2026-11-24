use application::{AuditEntry, AuditEventSink, AuditLog, SubscriberFailure, SystemPortError};
use std::collections::VecDeque;
use std::sync::{Arc, RwLock, RwLockWriteGuard};

#[derive(Debug, Default, Clone)]
pub struct AuditEventQueue(Arc<RwLock<VecDeque<AuditEntry>>>);

impl AuditEventQueue {
    fn entries(&self) -> RwLockWriteGuard<'_, VecDeque<AuditEntry>> {
        match self.0.write() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    pub async fn flush<Audit: AuditLog>(&self, audit: &Audit) -> Result<usize, SystemPortError> {
        let mut count = 0_usize;
        loop {
            let Some(entry) = self.entries().pop_front() else {
                return Ok(count);
            };
            if let Err(error) = audit.append(&entry).await {
                self.entries().push_front(entry);
                return Err(error);
            }
            count = count.saturating_add(1);
        }
    }
}

impl AuditEventSink for AuditEventQueue {
    fn enqueue(&self, entry: AuditEntry) -> Result<(), SubscriberFailure> {
        self.entries().push_back(entry);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::AuditEventQueue;
    use application::{
        AuditEntry, AuditEventSink, AuditKind, SystemFailure, SystemPort, SystemPortError,
    };
    use domain::{InstrumentId, Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
    use std::task::{Context, Poll, Waker};
    use testkit_ports::memory::MemoryReply;
    use testkit_ports::system_fakes::MemoryAuditLog;

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("즉시 완료되어야 한다"),
        }
    }

    fn entry(symbol: &str) -> AuditEntry {
        AuditEntry::new(
            Timestamp::from_parts(
                UnixSeconds::new(1_775_091_600),
                Nanosecond::new(0),
                OffsetSeconds::new(32_400),
            )
            .expect("시각"),
            AuditKind::SignalRaised(InstrumentId::try_new(symbol).expect("종목")),
        )
    }

    #[test]
    fn flush_appends_in_order_once_and_keeps_failed_entries() {
        let queue = AuditEventQueue::default();
        queue.enqueue(entry("kr-1")).expect("큐");
        queue.enqueue(entry("kr-2")).expect("큐");
        let failing = MemoryAuditLog::default();
        failing
            .script_append()
            .push(MemoryReply::Return(Err(SystemPortError::new(
                SystemPort::AuditLog,
                SystemFailure::InvalidData,
            ))));
        assert_eq!(run(queue.flush(&failing)).ok(), None);
        let log = MemoryAuditLog::default();
        for _ in 0_u8..4 {
            log.script_append().push(MemoryReply::Memory);
        }
        assert_eq!(run(queue.flush(&log)).expect("기록"), 2);
        assert_eq!(log.entries(), vec![entry("kr-1"), entry("kr-2")]);
        assert_eq!(run(queue.flush(&log)).expect("빈 큐"), 0);
    }
}
