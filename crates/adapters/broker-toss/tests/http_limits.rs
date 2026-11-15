mod support;
#[cfg(test)]
mod tests {
    use application::SecretValue;
    use broker_toss::{ReadRequest, ReadRoute, TossEndpoint, TossHttpClient, TossHttpError};
    use testkit::LocalHttpServer;

    #[test]
    fn body_limit_includes_exactly_eight_mib_and_rejects_the_next_byte() {
        for size in [8_388_608, 8_388_609] {
            let mut wire =
                format!("HTTP/1.1 200 OK\r\nContent-Length: {size}\r\nConnection: close\r\n\r\n")
                    .into_bytes();
            wire.extend(std::iter::repeat_n(b'X', size));
            let server = LocalHttpServer::start(wire).expect("server");
            let time = crate::support::time();
            let client = TossHttpClient::new(
                &TossEndpoint::local(server.port()).expect("endpoint"),
                std::sync::Arc::<crate::support::FakeTime>::clone(&time),
                time,
            )
            .expect("client");
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("runtime");
            let result = runtime.block_on(client.get(
                &ReadRequest::new(ReadRoute::Accounts),
                &SecretValue::new(b"fake".to_vec()),
            ));
            if size == 8_388_608 {
                assert_eq!(
                    result
                        .expect("boundary allowed")
                        .body()
                        .as_exposed_secret()
                        .len(),
                    8_388_608
                );
            } else {
                assert_eq!(
                    result.expect_err("overflow"),
                    TossHttpError::ResponseTooLarge
                );
            }
            assert_ne!(server.finish().expect("server finished").len(), 0);
        }
    }
}
