mod support;
#[cfg(test)]
mod tests {
    use super::support;
    use application::{Clock, SecretValue};
    use broker_toss::{ReadRequest, ReadRoute, TossEndpoint, TossHttpClient};

    #[test]
    fn retries_return_last_unmodified_response_without_real_sleep() {
        let responses = (0_u32..4_u32).map(|_| b"HTTP/1.1 429 Too Many Requests\r\nContent-Length: 3\r\nRetry-After: 7\r\nX-RateLimit-Limit: 5\r\nConnection: close\r\n\r\nend".to_vec()).collect();
        let server = testkit::serve_sequence(responses).expect("server");
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
        let response = runtime
            .block_on(client.get(
                &ReadRequest::new(ReadRoute::Holdings),
                &SecretValue::new(b"fake".to_vec()),
            ))
            .expect("response");
        assert_eq!(*time.now().utc_seconds().as_i64(), 1_800_000_028);
        assert_eq!(response.status(), 429);
        assert_eq!(response.body().as_exposed_secret(), b"end");
        assert_eq!(server.finish().expect("requests").len(), 4);
    }

    #[test]
    fn limit_reduction_and_invalid_header_preserve_pacing_and_request_order() {
        let mut responses = Vec::new();
        for header in ["3", "broken", "1"] {
            responses.push(format!("HTTP/1.1 200 OK\r\nContent-Length: 0\r\nX-RateLimit-Limit: {header}\r\nConnection: close\r\n\r\n").into_bytes());
        }
        let server = testkit::serve_sequence(responses).expect("server");
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
        for symbol in ["alpha", "bravo", "charlie"] {
            let request = ReadRequest::new(ReadRoute::Quotes)
                .with_query(broker_toss::QueryKey::Symbol, symbol.to_owned());
            assert_eq!(
                runtime
                    .block_on(client.get(&request, &SecretValue::new(b"fake".to_vec())))
                    .expect("response")
                    .status(),
                200
            );
        }
        let now = time.now();
        assert_eq!(*now.utc_seconds().as_i64(), 1_800_000_001);
        assert_eq!(*now.nanosecond().as_u32(), 111_111_112);
        let requests = server.finish().expect("requests");
        assert_eq!(requests.len(), 3);
        for (request, symbol) in requests.iter().zip(["alpha", "bravo", "charlie"]) {
            assert!(
                String::from_utf8_lossy(request)
                    .starts_with(&format!("GET /api/v1/prices?symbol={symbol} HTTP/1.1"))
            );
        }
    }

    #[test]
    fn token_issuance_is_never_retried_on_429() {
        let server = testkit::LocalHttpServer::start(b"HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\nRetry-After: 53\r\nConnection: close\r\n\r\n".to_vec()).expect("server");
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
        let response = runtime
            .block_on(broker_toss::issue_token(
                &client,
                &SecretValue::new(b"fake-id".to_vec()),
                &SecretValue::new(b"fake-secret".to_vec()),
            ))
            .expect("response");
        assert_eq!(response.status(), 429);
        assert_eq!(*time.now().utc_seconds().as_i64(), 1_800_000_000);
        assert!(
            server
                .finish()
                .expect("received bytes")
                .starts_with(b"POST /oauth2/token HTTP/1.1")
        );
    }
    #[test]
    fn exhausted_retry_cooldown_is_kept_for_the_next_caller() {
        let mut responses: Vec<_> = (0_u32..4_u32).map(|_| b"HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\nRetry-After: 7\r\nConnection: close\r\n\r\n".to_vec()).collect();
        responses
            .push(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec());
        let server = testkit::serve_sequence(responses).expect("server");
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
        for expected in [429, 200] {
            let response = runtime
                .block_on(client.get(
                    &ReadRequest::new(ReadRoute::Holdings),
                    &SecretValue::new(b"fake".to_vec()),
                ))
                .expect("response");
            assert_eq!(response.status(), expected);
        }
        assert_eq!(*time.now().utc_seconds().as_i64(), 1_800_000_039);
        assert_eq!(server.finish().expect("requests").len(), 5);
    }
    #[test]
    fn rate_headers_survive_a_truncated_response_body() {
        let server = testkit::serve_sequence(vec![b"HTTP/1.1 429 Too Many Requests\r\nContent-Length: 73\r\nRetry-After: 7\r\nX-RateLimit-Limit: 1\r\nConnection: close\r\n\r\nshort".to_vec(), b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()]).expect("server");
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
        let request = ReadRequest::new(ReadRoute::Holdings);
        let token = SecretValue::new(b"fake".to_vec());
        assert_eq!(
            runtime
                .block_on(client.get(&request, &token))
                .expect_err("truncated"),
            broker_toss::TossHttpError::Transport
        );
        assert_eq!(
            runtime
                .block_on(client.get(&request, &token))
                .expect("response")
                .status(),
            200
        );
        assert_eq!(*time.now().utc_seconds().as_i64(), 1_800_000_008);
        assert_eq!(server.finish().expect("requests").len(), 2);
    }
}
