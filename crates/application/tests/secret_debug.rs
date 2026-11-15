#[cfg(test)]
mod tests {
    use application::SecretValue;

    #[derive(Debug)]
    struct Envelope {
        secret: SecretValue,
    }

    fn check_formats(bytes: &[u8]) {
        let secret = SecretValue::new(bytes.to_vec().into());
        assert_eq!(format!("{secret:?}"), "SecretValue(<redacted>)");
        assert_eq!(format!("{secret:#?}"), "SecretValue(<redacted>)");
        assert_eq!(secret.as_exposed_secret(), bytes);
        let envelope = Envelope { secret };
        assert_eq!(
            format!("{envelope:?}"),
            "Envelope { secret: SecretValue(<redacted>) }"
        );
        assert_eq!(
            format!("{envelope:#?}"),
            "Envelope {\n    secret: SecretValue(<redacted>),\n}"
        );
        assert_eq!(envelope.secret.as_exposed_secret(), bytes);
    }

    #[test]
    fn normal_pretty_and_parent_debug_are_constant_across_bytes_and_lengths() {
        for byte in u8::MIN..=u8::MAX {
            for length in [0, 1, 2, 15, 32, 64, 255, 256, 1024] {
                check_formats(&vec![byte; length]);
            }
        }
        check_formats(b"A02_FAKE_SECRET_NEVER_IN_DEBUG");
        check_formats(b"\0\xff\x80\n\r\t");
    }
}
