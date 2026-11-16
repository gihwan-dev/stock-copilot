mod support;
#[cfg(test)]
mod tests {
    use application::SecretValue;
    use broker_toss::{ReadRequest, ReadRoute, RequestWait, TossEndpoint, TossHttpClient};
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    };
    use std::task::Poll;
    use std::time::Duration;
    #[derive(Default)]
    struct PendingWait(AtomicU64);
    impl RequestWait for PendingWait {
        fn wait(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
            self.0.store(
                u64::try_from(duration.as_nanos()).expect("duration"),
                Ordering::SeqCst,
            );
            Box::pin(std::future::pending())
        }
    }
    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
    }
    fn cancel_after<F: Future>(
        rt: &tokio::runtime::Runtime,
        future: F,
        progress: impl Fn() -> u64,
    ) -> u64 {
        let mut pending = Box::pin(future);
        rt.block_on(std::future::poll_fn(|cx| {
            assert!(pending.as_mut().poll(cx).is_pending());
            let observed = progress();
            if observed > 0 {
                Poll::Ready(observed)
            } else {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }))
    }
    #[test]
    fn cancelling_get_during_retry_keeps_cooldown_and_other_group_progress() {
        let server = testkit::serve_sequence(vec![b"HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\nRetry-After: 7\r\nConnection: close\r\n\r\n".to_vec(), b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()]).expect("server");
        let wait = Arc::new(PendingWait::default());
        let time: Arc<crate::support::FakeTime> = crate::support::time();
        let client = TossHttpClient::new(
            &TossEndpoint::local(server.port()).expect("endpoint"),
            time,
            Arc::<PendingWait>::clone(&wait),
        )
        .expect("client");
        let rt = runtime();
        let request = ReadRequest::new(ReadRoute::Holdings);
        let token = SecretValue::new(b"fake".to_vec());
        assert_eq!(
            cancel_after(&rt, client.get(&request, &token), || wait
                .0
                .load(Ordering::SeqCst)),
            8_000_000_000
        );
        assert_eq!(cancel_after(&rt, client.get(&request, &token), || 1), 1);
        assert_eq!(wait.0.load(Ordering::SeqCst), 8_000_000_000);
        assert_eq!(
            rt.block_on(client.get(&ReadRequest::new(ReadRoute::Quotes), &token))
                .expect("independent")
                .status(),
            200
        );
        assert_eq!(server.finish().expect("requests").len(), 2);
    }
    #[test]
    fn cancelling_get_during_send_keeps_interval_and_other_group_progress() {
        let server = testkit::serve_sequence_with_pending_first(vec![
            b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(),
        ])
        .expect("server");
        let wait = Arc::new(PendingWait::default());
        let time: Arc<crate::support::FakeTime> = crate::support::time();
        let client = TossHttpClient::new(
            &TossEndpoint::local(server.port()).expect("endpoint"),
            time,
            Arc::<PendingWait>::clone(&wait),
        )
        .expect("client");
        let rt = runtime();
        let request = ReadRequest::new(ReadRoute::Holdings);
        let token = SecretValue::new(b"fake".to_vec());
        assert_eq!(
            cancel_after(&rt, client.get(&request, &token), || u64::try_from(
                server.received_count()
            )
            .expect("count")),
            1
        );
        assert_eq!(cancel_after(&rt, client.get(&request, &token), || 1), 1);
        assert_eq!(wait.0.load(Ordering::SeqCst), 333_333_334);
        assert_eq!(
            rt.block_on(client.get(&ReadRequest::new(ReadRoute::Quotes), &token))
                .expect("independent")
                .status(),
            200
        );
        let requests = server.finish().expect("requests");
        assert_eq!(requests.len(), 2);
        assert!(
            requests
                .first()
                .expect("first")
                .starts_with(b"GET /api/v1/holdings ")
        );
        assert!(
            requests
                .last()
                .expect("last")
                .starts_with(b"GET /api/v1/prices ")
        );
    }
}
