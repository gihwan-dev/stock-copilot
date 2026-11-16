use src_tauri::build_app;

fn main() -> Result<(), tauri::Error> {
    #[expect(
        clippy::disallowed_macros,
        reason = "tauri::generate_context! expands to include_bytes! and env!"
    )]
    #[expect(
        clippy::disallowed_methods,
        reason = "tauri::generate_context! expands to std::process::exit"
    )]
    #[expect(
        clippy::disallowed_types,
        reason = "tauri::generate_context! expands to std::collections::HashMap"
    )]
    let context = tauri::generate_context!();
    let app = build_app(tauri::Builder::default(), context)?;
    app.run(|_, _| {});
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn composition_root_constructs_the_fixed_product_endpoint_without_sending_requests() {
        let endpoint = broker_toss::TossEndpoint::production();
        assert!(format!("{endpoint:?}").contains("Production"));
        let spec: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string("../../../docs/toss-openapi-1.2.28.json").expect("spec"),
        )
        .expect("JSON");
        let expected_origin = spec["servers"][0]["url"].as_str().expect("origin");
        assert_eq!(
            format!("{endpoint:?}"),
            format!("TossEndpoint {{ origin: {expected_origin:?}, mode: Production }}")
        );
        let client = broker_toss::TossHttpClient::new(
            &endpoint,
            std::sync::Arc::new(testkit::fixed_clock().expect("clock")),
            std::sync::Arc::new(broker_toss::TokioRequestWait),
        )
        .expect("product configuration");
        assert_eq!(format!("{client:?}"), "TossHttpClient(<redacted>)");
    }
}
