#[cfg(test)]
mod tests {
    use std::fmt;
    use std::io::{Error, ErrorKind};

    use application::TransportFailure;
    use sources::classify_transport;

    #[derive(Debug)]
    struct Layer {
        text: &'static str,
        cause: Option<Error>,
    }

    impl fmt::Display for Layer {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.text)
        }
    }

    impl std::error::Error for Layer {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            self.cause.as_ref().map(|cause| {
                let layer: &(dyn std::error::Error + 'static) = cause;
                layer
            })
        }
    }

    fn wrapped(kind: ErrorKind, text: &'static str) -> Layer {
        Layer {
            text: "error sending request",
            cause: Some(Error::new(kind, text)),
        }
    }

    #[test]
    fn the_deepest_io_cause_names_the_connect_failure() {
        assert_eq!(
            classify_transport(&wrapped(ErrorKind::ConnectionRefused, "refused")),
            Some(TransportFailure::Refused)
        );
        assert_eq!(
            classify_transport(&wrapped(
                ErrorKind::PermissionDenied,
                "Operation not permitted"
            )),
            Some(TransportFailure::Blocked),
            "macOS 네트워크 필터는 EPERM으로 끊는다"
        );
        assert_eq!(
            classify_transport(&wrapped(ErrorKind::ConnectionReset, "reset by peer")),
            Some(TransportFailure::Reset)
        );
    }

    #[test]
    fn dns_and_tls_failures_are_read_from_the_error_text() {
        assert_eq!(
            classify_transport(&wrapped(
                ErrorKind::Other,
                "dns error: failed to lookup address information"
            )),
            Some(TransportFailure::Dns)
        );
        assert_eq!(
            classify_transport(&wrapped(
                ErrorKind::InvalidData,
                "invalid peer certificate: UnknownIssuer"
            )),
            Some(TransportFailure::Tls)
        );
        assert_eq!(
            classify_transport(&wrapped(ErrorKind::Other, "something else")),
            None
        );
    }
}
