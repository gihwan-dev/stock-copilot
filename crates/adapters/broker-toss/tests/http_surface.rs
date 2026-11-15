#[cfg(test)]
mod tests {
    use std::fs;

    fn cases() -> [(&'static str, &'static str, bool); 7] {
        [
            (
                "valid",
                "fn main() { std::mem::drop(broker_toss::TossEndpoint::local(1234)); std::mem::drop(broker_toss::ReadRequest::new(broker_toss::ReadRoute::Accounts)); }",
                true,
            ),
            (
                "forbidden_post",
                "fn main() { let client: broker_toss::TossHttpClient = todo!(); client.post(\"/api/v1/orders\"); }",
                false,
            ),
            (
                "path",
                "fn main() { broker_toss::ReadRequest::new(\"/api/v1/orders\"); }",
                false,
            ),
            (
                "origin",
                "fn main() { broker_toss::TossEndpoint::local(\"http://192.0.2.1\"); }",
                false,
            ),
            (
                "arbitrary_client",
                "fn main() { broker_toss::TossHttpClient::from_url(\"https://example.invalid\"); }",
                false,
            ),
            (
                "fields",
                "fn main() { let endpoint = broker_toss::TossEndpoint { origin: String::new(), mode: todo!() }; }",
                false,
            ),
            (
                "client",
                "fn main() { let client: broker_toss::TossHttpClient = todo!(); let raw = client.client; }",
                false,
            ),
        ]
    }
    #[test]
    fn external_consumers_cannot_construct_arbitrary_requests_or_origins() {
        let scratch = std::path::Path::new("../../../target/T-18")
            .join(format!("api-surface-{}", std::process::id()));
        fs::create_dir_all(&scratch).expect("scratch");
        let dependency = fs::canonicalize(".").expect("current crate");
        let manifest = scratch.join("Cargo.toml");
        fs::write(&manifest, format!("[package]\nname=\"t18-api-probe\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[workspace]\n[dependencies]\nbroker-toss={{path={}}}\n", serde_json::to_string(&dependency).expect("quoted path"))).expect("manifest");
        fs::copy("../../../Cargo.lock", scratch.join("Cargo.lock")).expect("pinned dependencies");
        fs::create_dir_all(scratch.join("src")).expect("source directory");
        for (name, source, allowed) in cases() {
            let input = scratch.join("src/main.rs");
            fs::write(&input, source).expect("probe");
            let output =
                testkit::check_consumer(&manifest, &scratch.join("target")).expect("compiler");
            assert_eq!(
                output.status.success(),
                allowed,
                "{name}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            if !allowed {
                assert!(
                    String::from_utf8_lossy(&output.stderr).contains("error[E"),
                    "{name}"
                );
            }
        }
        fs::remove_dir_all(scratch).expect("remove probes");
    }
}
