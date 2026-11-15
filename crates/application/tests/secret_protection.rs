#[cfg(test)]
mod tests {
    use application::SecretValue;

    const _: fn() = || {
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<Value> AmbiguousIfImplemented<()> for &Value {}
        struct Implemented;
        impl<Value: Clone> AmbiguousIfImplemented<Implemented> for &Value {}
        let _ = <&SecretValue as AmbiguousIfImplemented<_>>::probe;
    };

    const _: fn() = || {
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<Value> AmbiguousIfImplemented<()> for &Value {}
        struct Implemented;
        impl<Value: Copy> AmbiguousIfImplemented<Implemented> for &Value {}
        let _ = <&SecretValue as AmbiguousIfImplemented<_>>::probe;
    };

    const _: fn() = || {
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<Value> AmbiguousIfImplemented<()> for &Value {}
        struct Implemented;
        impl<Value: std::fmt::Display> AmbiguousIfImplemented<Implemented> for &Value {}
        let _ = <&SecretValue as AmbiguousIfImplemented<_>>::probe;
    };

    const _: fn() = || {
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<Value> AmbiguousIfImplemented<()> for &Value {}
        struct Implemented;
        impl<Value: serde_core::Serialize> AmbiguousIfImplemented<Implemented> for &Value {}
        let _ = <&SecretValue as AmbiguousIfImplemented<_>>::probe;
    };

    const _: fn() = || {
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<Value> AmbiguousIfImplemented<()> for &Value {}
        struct Implemented;
        impl<Value: serde_core::Deserialize<'static>> AmbiguousIfImplemented<Implemented> for &Value {}
        let _ = <&SecretValue as AmbiguousIfImplemented<_>>::probe;
    };

    const _: fn() = || {
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<Value> AmbiguousIfImplemented<()> for &Value {}
        struct Implemented;
        impl<Value: serde_core::de::DeserializeOwned> AmbiguousIfImplemented<Implemented> for &Value {}
        let _ = <&SecretValue as AmbiguousIfImplemented<_>>::probe;
    };

    const _: fn() = || {
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<Value> AmbiguousIfImplemented<()> for &Value {}
        struct Implemented;
        impl<Value: AsRef<[u8]>> AmbiguousIfImplemented<Implemented> for &Value {}
        let _ = <&SecretValue as AmbiguousIfImplemented<_>>::probe;
    };

    const _: fn() = || {
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<Value> AmbiguousIfImplemented<()> for &Value {}
        struct Implemented;
        impl<Value: AsMut<[u8]>> AmbiguousIfImplemented<Implemented> for &Value {}
        let _ = <&SecretValue as AmbiguousIfImplemented<_>>::probe;
    };

    const _: fn() = || {
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<Value> AmbiguousIfImplemented<()> for &Value {}
        struct Implemented;
        impl<Value: std::ops::Deref> AmbiguousIfImplemented<Implemented> for &Value {}
        let _ = <&SecretValue as AmbiguousIfImplemented<_>>::probe;
    };

    const _: fn() = || {
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<Value> AmbiguousIfImplemented<()> for &Value {}
        struct Implemented;
        impl<Value: std::ops::DerefMut> AmbiguousIfImplemented<Implemented> for &Value {}
        let _ = <&SecretValue as AmbiguousIfImplemented<_>>::probe;
    };

    const _: fn() = || {
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<Value> AmbiguousIfImplemented<()> for &Value {}
        struct Implemented;
        impl<Value: PartialEq> AmbiguousIfImplemented<Implemented> for &Value {}
        let _ = <&SecretValue as AmbiguousIfImplemented<_>>::probe;
    };

    const _: fn() = || {
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<Value> AmbiguousIfImplemented<()> for &Value {}
        struct Implemented;
        impl<Value: Eq> AmbiguousIfImplemented<Implemented> for &Value {}
        let _ = <&SecretValue as AmbiguousIfImplemented<_>>::probe;
    };

    const _: fn() = || {
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<Value> AmbiguousIfImplemented<()> for &Value {}
        struct Implemented;
        impl<Value: std::hash::Hash> AmbiguousIfImplemented<Implemented> for &Value {}
        let _ = <&SecretValue as AmbiguousIfImplemented<_>>::probe;
    };

    const _: fn() = || {
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<Value> AmbiguousIfImplemented<()> for &Value {}
        struct Implemented;
        impl<Value: PartialOrd> AmbiguousIfImplemented<Implemented> for &Value {}
        let _ = <&SecretValue as AmbiguousIfImplemented<_>>::probe;
    };

    const _: fn() = || {
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<Value> AmbiguousIfImplemented<()> for &Value {}
        struct Implemented;
        impl<Value: Ord> AmbiguousIfImplemented<Implemented> for &Value {}
        let _ = <&SecretValue as AmbiguousIfImplemented<_>>::probe;
    };

    const _: fn() = || {
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<Value> AmbiguousIfImplemented<()> for &Value {}
        struct Implemented;
        impl<Value: Default> AmbiguousIfImplemented<Implemented> for &Value {}
        let _ = <&SecretValue as AmbiguousIfImplemented<_>>::probe;
    };

    const _: fn() = || {
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<Value> AmbiguousIfImplemented<()> for &Value {}
        struct Implemented;
        impl<Value: From<Vec<u8>>> AmbiguousIfImplemented<Implemented> for &Value {}
        let _ = <&SecretValue as AmbiguousIfImplemented<_>>::probe;
    };

    const _: fn() = || {
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<Value> AmbiguousIfImplemented<()> for &Value {}
        struct Implemented;
        impl<Value: Into<Vec<u8>>> AmbiguousIfImplemented<Implemented> for &Value {}
        let _ = <&SecretValue as AmbiguousIfImplemented<_>>::probe;
    };

    const _: fn() = || {
        trait AmbiguousIfImplemented<Marker> {
            fn probe() {}
        }
        impl<Value> AmbiguousIfImplemented<()> for &Value {}
        struct Implemented;
        impl<Value: std::borrow::Borrow<[u8]>> AmbiguousIfImplemented<Implemented> for &Value {}
        let _ = <&SecretValue as AmbiguousIfImplemented<_>>::probe;
    };

    #[test]
    fn secret_source_has_only_private_storage_debug_and_explicit_access() {
        let source = std::fs::read_to_string("src/secret_store.rs").expect("비밀 타입 소스");
        let compact: String = source
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        assert!(!compact.contains("derive"));
        assert!(compact.contains("pubstructSecretValue(SecretSlice<u8>);"));
        assert_eq!(compact.matches("impl").count(), 2);
        assert!(compact.contains("implstd::fmt::DebugforSecretValue{"));
        assert!(compact.contains("implSecretValue{"));
        assert!(compact.contains("pubfnnew(value:Vec<u8>)->Self"));
        assert_eq!(compact.matches("pubfn").count(), 2);
        assert!(compact.contains("pubfnas_exposed_secret(&self)->&[u8]"));
    }
    #[test]
    fn secrecy_manifest_keeps_default_only_and_serialization_probe_is_dev_only() {
        let manifest = std::fs::read_to_string("Cargo.toml").expect("application 매니페스트");
        let compact: String = manifest
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        let (normal, dev) = compact
            .split_once("[dev-dependencies]")
            .expect("개발 의존성");
        assert!(normal.contains("secrecy=\"=0.10.3\""));
        assert!(!normal.contains("serde"));
        assert!(dev.contains("serde_core={version=\"=1.0.229\",default-features=false}"));
        assert!(!compact.contains("secrecy/serde"));
        assert!(!compact.contains("zeroize_derive"));
    }
}
