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
    #[test]
    fn a_429_limit_reduction_controls_the_next_attempt_even_with_zero_retry_after() {
        let server = testkit::serve_sequence(vec![b"HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\nRetry-After: 0\r\nX-RateLimit-Limit: 1\r\nConnection: close\r\n\r\n".to_vec(), b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()]).expect("server");
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
        assert_eq!(
            runtime
                .block_on(client.get(
                    &ReadRequest::new(ReadRoute::Holdings),
                    &SecretValue::new(b"fake".to_vec())
                ))
                .expect("response")
                .status(),
            200
        );
        let now = time.now();
        assert_eq!(*now.utc_seconds().as_i64(), 1_800_000_001);
        assert_eq!(*now.nanosecond().as_u32(), 666_666_667);
        assert_eq!(server.finish().expect("requests").len(), 2);
    }
    #[test]
    fn zero_limit_does_not_prevent_retry_exhaustion_or_the_next_request() {
        let mut responses: Vec<_> = (0_u32..4_u32).map(|_| b"HTTP/1.1 429 Too Many Requests\r\nContent-Length: 4\r\nRetry-After: 0\r\nX-RateLimit-Limit: 0\r\nConnection: close\r\n\r\nlast".to_vec()).collect();
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
        let request = ReadRequest::new(ReadRoute::Holdings);
        let token = SecretValue::new(b"fake".to_vec());
        let last = runtime
            .block_on(client.get(&request, &token))
            .expect("last429");
        assert_eq!(last.status(), 429);
        assert_eq!(last.body().as_exposed_secret(), b"last");
        assert_eq!(last.headers().limit(), Some("0"));
        assert_eq!(
            runtime
                .block_on(client.get(&request, &token))
                .expect("next")
                .status(),
            200
        );
        assert_eq!(*time.now().utc_seconds().as_i64(), 1_800_000_011);
        assert_eq!(server.finish().expect("requests").len(), 5);
    }
    #[test]
    fn server_errors_are_returned_without_retry_or_wait() {
        for (status, reason) in [(500, "Internal Server Error"), (503, "Service Unavailable")] {
            let server = testkit::LocalHttpServer::start(format!("HTTP/1.1 {status} {reason}\r\nContent-Length: 0\r\nRetry-After: 17\r\nConnection: close\r\n\r\n").into_bytes()).expect("server");
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
            assert_eq!(
                runtime
                    .block_on(client.get(
                        &ReadRequest::new(ReadRoute::Holdings),
                        &SecretValue::new(b"fake".to_vec())
                    ))
                    .expect("response")
                    .status(),
                status
            );
            assert_eq!(*time.now().utc_seconds().as_i64(), 1_800_000_000);
            assert_eq!(*time.now().nanosecond().as_u32(), 0);
            assert!(
                server
                    .finish()
                    .expect("received bytes")
                    .starts_with(b"GET /api/v1/holdings ")
            );
        }
    }
    #[test]
    fn transport_failure_is_returned_without_retry_or_wait() {
        let server = testkit::LocalHttpServer::start(
            b"HTTP/1.1 200 OK\r\nContent-Length: 73\r\nConnection: close\r\n\r\nshort".to_vec(),
        )
        .expect("server");
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
        assert_eq!(
            runtime
                .block_on(client.get(
                    &ReadRequest::new(ReadRoute::Accounts),
                    &SecretValue::new(b"fake".to_vec())
                ))
                .expect_err("truncated"),
            broker_toss::TossHttpError::Transport
        );
        assert_eq!(*time.now().utc_seconds().as_i64(), 1_800_000_000);
        assert_eq!(*time.now().nanosecond().as_u32(), 0);
        assert!(
            server
                .finish()
                .expect("received bytes")
                .starts_with(b"GET /api/v1/accounts ")
        );
    }
    #[test]
    fn a_long_representable_retry_after_is_fully_honored() {
        let server = testkit::serve_sequence((0_u32..4_u32).map(|_| b"HTTP/1.1 429 Too Many Requests\r\nContent-Length: 3\r\nRetry-After: 86400\r\nConnection: close\r\n\r\nday".to_vec()).collect()).expect("server");
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
        let raw = runtime
            .block_on(client.get(
                &ReadRequest::new(ReadRoute::Holdings),
                &SecretValue::new(b"fake".to_vec()),
            ))
            .expect("raw429");
        assert_eq!(raw.status(), 429);
        assert_eq!(raw.body().as_exposed_secret(), b"day");
        assert_eq!(*time.now().utc_seconds().as_i64(), 1_800_259_207);
        assert_eq!(*time.now().nanosecond().as_u32(), 0);
        assert_eq!(server.finish().expect("requests").len(), 4);
    }
}
