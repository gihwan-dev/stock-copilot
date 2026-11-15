#[cfg(test)]
mod tests {
    use application::SecretValue;
    use proptest::{prelude::any, prop_assert, prop_assert_eq, proptest};

    #[derive(Debug)]
    struct Envelope {
        secret: SecretValue,
    }

    proptest! {
        #![proptest_config(proptest::test_runner::Config::with_cases(testkit::property_cases()))]
        #[test]
        fn arbitrary_bytes_and_lengths_have_identical_debug(
            bytes in proptest::collection::vec(any::<u8>(), 0..4097),
            different in proptest::collection::vec(any::<u8>(), 0..4097),
        ) {
            let secret = SecretValue::new(bytes.clone());
            let other = SecretValue::new(different);
            prop_assert_eq!(format!("{secret:?}"), "SecretValue(<redacted>)");
            prop_assert_eq!(format!("{secret:#?}"), "SecretValue(<redacted>)");
            prop_assert_eq!(format!("{secret:?}"), format!("{other:?}"));
            prop_assert_eq!(format!("{secret:#?}"), format!("{other:#?}"));
            prop_assert_eq!(secret.as_exposed_secret(), bytes.as_slice());
            let envelope = Envelope { secret };
            prop_assert_eq!(format!("{envelope:?}"), "Envelope { secret: SecretValue(<redacted>) }");
            prop_assert_eq!(format!("{envelope:#?}"), "Envelope {\n    secret: SecretValue(<redacted>),\n}");
            prop_assert_eq!(envelope.secret.as_exposed_secret(), bytes.as_slice());
        }

        #[test]
        fn marked_payload_is_absent_from_direct_and_parent_debug(
            payload in proptest::collection::vec(any::<u8>(), 0..4097),
        ) {
            let mut bytes = b"\0A02_FAKE_SECRET_".to_vec();
            bytes.extend(payload);
            let secret = SecretValue::new(bytes.clone());
            let normal = format!("{secret:?}");
            let pretty = format!("{secret:#?}");
            let envelope = Envelope { secret };
            let byte_representation = format!("{bytes:?}");
            for rendered in [normal, pretty, format!("{envelope:?}"), format!("{envelope:#?}")] {
                prop_assert!(!rendered.as_bytes().contains(&0));
                prop_assert!(!rendered.contains("A02_FAKE_SECRET_"));
                prop_assert!(!rendered.contains(&byte_representation));
            }
            prop_assert_eq!(envelope.secret.as_exposed_secret(), bytes.as_slice());
        }
    }
}
