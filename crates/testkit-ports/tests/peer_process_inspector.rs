#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::task::{Context, Poll, Waker};

    use application::{
        ExecutablePath, PeerProcessInfo, PeerProcessInspector, PeerProcessLookupError, ProcessId,
    };
    use testkit_ports::system_fakes::ScriptedPeerProcessInspector;

    fn run<F: Future>(future: F) -> F::Output {
        let mut pinned = std::pin::pin!(future);
        match pinned
            .as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
        {
            Poll::Ready(value) => value,
            Poll::Pending => panic!("대본 가짜는 즉시 완료되어야 한다"),
        }
    }

    #[test]
    fn scripted_peer_process_inspector_replays_fifo_results_and_records_calls() {
        let parent = ProcessId::try_new(4100_u32).expect("부모 PID");
        let exe =
            ExecutablePath::try_new("/Applications/StockCopilot.app/mcp-bridge").expect("경로");
        let info = PeerProcessInfo::new(parent, exe);
        let inspector = ScriptedPeerProcessInspector::new([
            Ok(info.clone()),
            Err(PeerProcessLookupError::NotFound),
            Err(PeerProcessLookupError::PermissionDenied),
        ]);
        inspector
            .script()
            .push(Err(PeerProcessLookupError::Unavailable));

        let pid_one = ProcessId::try_new(5001_u32).expect("PID 1");
        let pid_two = ProcessId::try_new(5002_u32).expect("PID 2");
        let pid_three = ProcessId::try_new(5003_u32).expect("PID 3");
        let pid_four = ProcessId::try_new(5004_u32).expect("PID 4");

        assert_eq!(
            run(inspector.inspect(pid_one)),
            Ok(info),
            "첫 호출은 대본 Ok 반환"
        );
        assert_eq!(
            run(inspector.inspect(pid_two)),
            Err(PeerProcessLookupError::NotFound),
            "둘째 호출은 NotFound 반환"
        );
        assert_eq!(
            run(inspector.inspect(pid_three)),
            Err(PeerProcessLookupError::PermissionDenied),
            "셋째 호출은 PermissionDenied 반환"
        );
        assert_eq!(
            run(inspector.inspect(pid_four)),
            Err(PeerProcessLookupError::Unavailable),
            "넷째 호출은 Unavailable 반환"
        );
        assert_eq!(
            inspector.calls(),
            vec![pid_one, pid_two, pid_three, pid_four],
            "호출한 상대 PID를 순서대로 기록한다"
        );
    }
}
