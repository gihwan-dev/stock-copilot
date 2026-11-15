#[cfg(test)]
mod tests {
    use application::SecretValue;
    use broker_toss::{ReadRequest, ReadRoute, TossEndpoint, TossHttpClient, TossHttpError};
    use testkit::LocalHttpServer;

    #[test]
    fn local_transport_preserves_status_headers_and_bytes_without_debug_secrets() {
        let server = LocalHttpServer::start(b"HTTP/1.1 429 Too Many Requests\r\nContent-Length: 4\r\nRetry-After: 17\r\nX-RateLimit-Limit: 23\r\nX-RateLimit-Remaining: 11\r\nX-RateLimit-Reset: 19\r\nConnection: close\r\n\r\nraw!".to_vec()).expect("server");
        let port = server.port();
        let client =
            TossHttpClient::new(&TossEndpoint::local(port).expect("endpoint")).expect("client");
        let request = ReadRequest::new(ReadRoute::Accounts);
        let token = SecretValue::new(b"fake-token-731".to_vec());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let response = runtime
            .block_on(client.get(&request, &token))
            .expect("response");
        assert_eq!(response.status(), 429);
        assert_eq!(response.body(), b"raw!");
        assert_eq!(response.headers().retry_after(), Some("17"));
        assert_eq!(response.headers().limit(), Some("23"));
        assert_eq!(response.headers().remaining(), Some("11"));
        assert_eq!(response.headers().reset(), Some("19"));
        assert!(!format!("{client:?}{request:?}{response:?}{token:?}").contains("fake-token-731"));
        assert!(!format!("{response:?}").contains("raw!"));
        let bytes = server.finish().expect("server");
        let received = String::from_utf8_lossy(&bytes);
        assert!(received.starts_with("GET /api/v1/accounts HTTP/1.1"));
        assert!(received.contains("Bearer fake-token-731"));
    }

    #[test]
    fn zero_port_and_invalid_secret_are_closed_errors() {
        assert_eq!(
            TossEndpoint::local(0).err(),
            Some(TossHttpError::InvalidEndpoint)
        );
        let client =
            TossHttpClient::new(&TossEndpoint::local(49151).expect("endpoint")).expect("client");
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime");
        let error = runtime
            .block_on(client.get(
                &ReadRequest::new(ReadRoute::Accounts),
                &SecretValue::new(b"bad\r\nheader".to_vec()),
            ))
            .expect_err("reject");
        assert_eq!(error, TossHttpError::InvalidCredential);
        assert!(!format!("{error:?}").contains("header"));
    }

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
    }

    #[test]
    fn redirects_are_returned_without_following_the_location_or_leaking_it() {
        let server = LocalHttpServer::start(b"HTTP/1.1 302 Found\r\nLocation: http://192.0.2.1/secret-location\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()).expect("server");
        let client = TossHttpClient::new(&TossEndpoint::local(server.port()).expect("endpoint"))
            .expect("client");
        let response = runtime()
            .block_on(client.get(
                &ReadRequest::new(ReadRoute::Accounts),
                &SecretValue::new(b"fake".to_vec()),
            ))
            .expect("response");
        assert_eq!(response.status(), 302);
        assert!(!format!("{response:?}").contains("secret-location"));
        assert_ne!(server.finish().expect("received bytes").len(), 0);
    }

    #[test]
    fn token_issuance_uses_only_the_fixed_form_endpoint_and_preserves_raw_body() {
        let server = LocalHttpServer::start(
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nConnection: close\r\n\r\ntoken".to_vec(),
        )
        .expect("server");
        let client = TossHttpClient::new(&TossEndpoint::local(server.port()).expect("endpoint"))
            .expect("client");
        let response = runtime()
            .block_on(broker_toss::issue_token(
                &client,
                &SecretValue::new(b"id A+".to_vec()),
                &SecretValue::new(b"k&=\xFF".to_vec()),
            ))
            .expect("token");
        assert_eq!(response.body(), b"token");
        let request = String::from_utf8(server.finish().expect("received bytes")).expect("utf8");
        assert!(request.starts_with("POST /oauth2/token HTTP/1.1\r\n"));
        assert!(
            request
                .to_ascii_lowercase()
                .contains("content-type: application/x-www-form-urlencoded")
        );
        assert!(
            request.ends_with(
                "grant_type=client_credentials&client_id=id+A%2B&client_secret=k%26%3D%FF"
            )
        );
        assert!(!request.lines().next().expect("line").contains("id+A"));
        assert!(!format!("{response:?}").contains("token"));
    }

    #[test]
    fn query_and_account_values_are_encoded_and_sent_in_distinct_places() {
        let server = LocalHttpServer::start(
            b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n".to_vec(),
        )
        .expect("server");
        let client = TossHttpClient::new(&TossEndpoint::local(server.port()).expect("endpoint"))
            .expect("client");
        let request = ReadRequest::new(ReadRoute::Transactions)
            .with_query(broker_toss::QueryKey::Cursor, "next?&=#/ value".to_owned())
            .with_account(SecretValue::new(b"account-413".to_vec()));
        let response = runtime()
            .block_on(client.get(&request, &SecretValue::new(b"bearer-937".to_vec())))
            .expect("response");
        assert_eq!(response.status(), 204);
        assert_eq!(response.body(), b"");
        assert_eq!(response.headers().retry_after(), None);
        let received = String::from_utf8(server.finish().expect("received bytes")).expect("utf8");
        assert!(
            received
                .starts_with("GET /api/v1/transactions?cursor=next%3F%26%3D%23%2F+value HTTP/1.1")
        );
        assert!(
            received
                .to_ascii_lowercase()
                .contains("x-tossinvest-account: account-413")
        );
        assert!(received.contains("Bearer bearer-937"));
        assert!(!format!("{request:?}{response:?}{client:?}").contains("account-413"));
    }

    #[test]
    fn broken_transport_never_returns_the_request_or_response_body() {
        let server = LocalHttpServer::start(
            b"HTTP/1.1 200 OK\r\nContent-Length: 93\r\nConnection: close\r\n\r\nsecret-fragment"
                .to_vec(),
        )
        .expect("server");
        let client = TossHttpClient::new(&TossEndpoint::local(server.port()).expect("endpoint"))
            .expect("client");
        let error = runtime()
            .block_on(client.get(
                &ReadRequest::new(ReadRoute::Accounts),
                &SecretValue::new(b"secret-credential".to_vec()),
            ))
            .expect_err("truncated");
        assert_eq!(error, TossHttpError::Transport);
        assert_eq!(format!("{error:?}"), "Transport");
        assert_ne!(server.finish().expect("received bytes").len(), 0);
    }

    #[test]
    fn proxy_environment_is_ignored_in_a_fresh_process() {
        let status = testkit::proxy_probe(
            &testkit::test_executable().expect("test executable"),
            "tests::local_transport_preserves_status_headers_and_bytes_without_debug_secrets",
        )
        .expect("proxy probe");
        assert!(status.success());
    }
}
