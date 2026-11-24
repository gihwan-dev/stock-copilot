mod support;
#[cfg(test)]
mod tests {
    use super::support;
    use application::{Clock, SecretValue};
    use broker_toss::{ReadRequest, ReadRoute, TossEndpoint, TossHttpClient};

    fn ok() -> Vec<u8> {
        b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()
    }

    #[test]
    fn whole_market_listing_is_paced_below_one_request_per_second_without_blocking_sectors() {
        let server = testkit::serve_sequence(vec![ok(), ok(), ok(), ok()]).expect("server");
        let time = support::time();
        let client = TossHttpClient::new(
            &TossEndpoint::local(server.port()).expect("endpoint"),
            std::sync::Arc::<support::FakeTime>::clone(&time),
            std::sync::Arc::<support::FakeTime>::clone(&time),
        )
        .expect("client");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let token = SecretValue::new(b"fake".to_vec());
        let mut seen = Vec::new();
        for _ in 0_u8..3 {
            let response = runtime
                .block_on(client.get(&ReadRequest::new(ReadRoute::Instruments), &token))
                .expect("response");
            assert_eq!(response.status(), 200);
            seen.push(time.now());
        }
        let one_second =
            domain::TimeSpan::try_new(domain::SpanSeconds::new(1), domain::Nanosecond::new(0))
                .expect("1초");
        for (earlier, later) in seen.iter().zip(seen.iter().skip(1)) {
            let gap = earlier.elapsed_until(*later).expect("간격");
            assert!(gap > one_second, "{gap:?}");
        }
        let before_sector = time.now();
        assert_eq!(
            runtime
                .block_on(client.get(&ReadRequest::new(ReadRoute::Sectors), &token))
                .expect("섹터")
                .status(),
            200
        );
        assert_eq!(time.now(), before_sector);
        let requests = server.finish().expect("requests");
        assert_eq!(requests.len(), 4);
        for request in requests.iter().take(3) {
            assert!(String::from_utf8_lossy(request).starts_with("GET /api/v1/stocks/all"));
        }
    }
}
