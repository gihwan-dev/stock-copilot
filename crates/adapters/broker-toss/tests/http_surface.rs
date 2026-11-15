#[cfg(test)]
mod tests {
    use std::fs;

    fn cases() -> [(&'static str, &'static str, bool); 6] {
        [
            (
                "valid",
                "fn main() { let endpoint = broker_toss::TossEndpoint::local(1234); let request = broker_toss::ReadRequest::new(broker_toss::ReadRoute::Accounts); }",
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
        let executable = testkit::test_executable().expect("executable");
        let dependencies = executable.parent().expect("dependencies");
        let library = fs::read_dir(dependencies)
            .expect("dependencies")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("libbroker_toss-"))
                    && path
                        .extension()
                        .is_some_and(|extension| extension == "rlib")
            })
            .max_by_key(|path| {
                fs::metadata(path)
                    .expect("metadata")
                    .modified()
                    .expect("modified")
            })
            .expect("broker library");
        let scratch = dependencies
            .parent()
            .expect("profile")
            .join(format!("t18-api-{}", std::process::id()));
        fs::create_dir_all(&scratch).expect("scratch");
        for (name, source, allowed) in cases() {
            let input = scratch.join(format!("{name}.rs"));
            fs::write(&input, source).expect("probe");
            let output = testkit::compile_probe(
                &input,
                &library,
                dependencies,
                &scratch.join(format!("{name}.rmeta")),
            )
            .expect("compiler");
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
