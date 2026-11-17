use src_tauri::{build_app, run_app};
use src_tauri::{
    database::DatabaseFactory, ports::Ports, product_ports::ProductPorts, unavailable::Unavailable,
};
use std::sync::Arc;

fn product_context() -> tauri::Context<tauri::Wry> {
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
    context
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = product_context();
    let clock = platform_macos::SystemClock::new()
        .map_err(|error| std::io::Error::other(format!("{error:?}")))?;
    let shared_clock = Arc::new(clock);
    let ports = Ports::<ProductPorts> {
        broker_read: Arc::new(Unavailable),
        market_info: Arc::new(Unavailable),
        macro_series: Arc::new(Unavailable),
        fundamentals: Arc::new(Unavailable),
        ai_provider: Arc::new(Unavailable),
        portfolios: Arc::new(Unavailable),
        transactions: Arc::new(Unavailable),
        market_data: Arc::new(Unavailable),
        news: Arc::new(Unavailable),
        theses: Arc::new(Unavailable),
        estimates: Arc::new(Unavailable),
        signals: Arc::new(Unavailable),
        proposals: Arc::new(Unavailable),
        valuations: Arc::new(Unavailable),
        settings: Arc::new(Unavailable),
        audit: Arc::new(Unavailable),
        secrets: Arc::new(Unavailable),
        notifier: Arc::new(Unavailable),
        news_feeds: Vec::new(),
        clock: Arc::clone(&shared_clock),
        presence: Arc::new(platform_macos::UnavailablePresence),
        events: application::EventBusBuilder::default().build(),
        saved_accounts: Vec::new(),
        database: Arc::new(database_factory(shared_clock)),
    };
    run_app(
        build_app(tauri::Builder::default(), ports, context)?,
        |handle, code| handle.exit(code.as_i32()),
    );
    Ok(())
}

fn database_factory<C: application::Clock + 'static>(clock: Arc<C>) -> DatabaseFactory {
    DatabaseFactory(Box::new(move |path, key| {
        src_tauri::database::prepare_directory(&path)?;
        store_sqlite::Database::open(&path, &key, clock.as_ref())
    }))
}

#[cfg(test)]
mod tests {
    #[test]
    fn product_acl_allows_only_main_window_event_subscription() {
        let mut context = super::product_context();
        let authority = context.runtime_authority_mut();
        let local = tauri::ipc::Origin::Local;
        let remote = tauri::ipc::Origin::Remote {
            url: "https://example.com".parse().expect("원격 출처"),
        };
        for command in ["plugin:event|listen", "plugin:event|unlisten"] {
            assert!(
                authority
                    .resolve_access(command, "main", "main", &local)
                    .is_some(),
                "{command}"
            );
            assert!(
                authority
                    .resolve_access(command, "other", "other", &local)
                    .is_none(),
                "{command}"
            );
            assert!(
                authority
                    .resolve_access(command, "main", "main", &remote)
                    .is_none(),
                "{command}"
            );
        }
        for command in ["plugin:event|emit", "plugin:event|emit_to"] {
            assert!(
                authority
                    .resolve_access(command, "main", "main", &local)
                    .is_none(),
                "{command}"
            );
        }
        let without_permissions = tauri::runtime_authority!(
            std::collections::BTreeMap::new(),
            tauri::utils::acl::resolved::Resolved::default()
        );
        for command in ["plugin:event|listen", "plugin:event|unlisten"] {
            assert!(
                without_permissions
                    .resolve_access(command, "main", "main", &local)
                    .is_none(),
                "{command}"
            );
        }
    }

    #[test]
    fn product_event_permissions_are_individually_required() {
        use std::collections::BTreeMap;
        use tauri::utils::acl::{capability::Capability, manifest::Manifest, resolved::Resolved};

        let manifests: BTreeMap<String, Manifest> = serde_json::from_str(
            &std::fs::read_to_string("gen/schemas/acl-manifests.json").expect("생성된 ACL"),
        )
        .expect("ACL 매니페스트");
        let capability: Capability = serde_json::from_str(
            &std::fs::read_to_string("capabilities/main.json").expect("제품 capability"),
        )
        .expect("제품 capability 형식");
        for (omitted, denied, retained) in [
            (
                "core:event:allow-listen",
                "plugin:event|listen",
                "plugin:event|unlisten",
            ),
            (
                "core:event:allow-unlisten",
                "plugin:event|unlisten",
                "plugin:event|listen",
            ),
        ] {
            let mut edited = capability.clone();
            edited
                .permissions
                .retain(|permission| permission.identifier().get() != omitted);
            assert_eq!(edited.permissions.len() + 1, capability.permissions.len());
            let capabilities = BTreeMap::from([(edited.identifier.clone(), edited)]);
            let resolved = Resolved::resolve(
                &manifests,
                capabilities,
                tauri::utils::platform::Target::MacOS,
            )
            .expect("제품 ACL 해석");
            let authority = tauri::runtime_authority!(BTreeMap::new(), resolved);
            assert!(
                authority
                    .resolve_access(denied, "main", "main", &tauri::ipc::Origin::Local)
                    .is_none(),
                "{omitted}: {denied}"
            );
            assert!(
                authority
                    .resolve_access(retained, "main", "main", &tauri::ipc::Origin::Local)
                    .is_some(),
                "{omitted}: {retained}"
            );
        }
    }

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
            std::sync::Arc::new(testkit_ports::SteppedClock::new(
                broker_toss::TossTimestamp::try_from("2026-11-16T00:00:00Z")
                    .expect("clock")
                    .into_domain(),
            )),
            std::sync::Arc::new(broker_toss::TokioRequestWait),
        )
        .expect("product configuration");
        assert_eq!(format!("{client:?}"), "TossHttpClient(<redacted>)");
    }

    #[test]
    fn configured_database_is_opened_and_explicitly_closed_after_refresh_cleanup() {
        use src_tauri::database::{DatabaseStatus, open_database};
        use src_tauri::lifecycle::Lifecycle;
        use std::sync::Arc;
        use testkit_ports::{memory::MemoryReply, system_fakes::MemorySecretStore};
        let directory = std::path::PathBuf::from("../../../target/U-01")
            .join(format!("database-lifecycle-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("테스트 폴더");
        let path = store_sqlite::DatabasePath::new(directory.join("nested").join("store.db"));
        let secrets = MemorySecretStore::default();
        secrets.script_load().push(MemoryReply::Return(Ok(Some(
            application::SecretValue::new(vec![83; 32]),
        ))));
        let clock = Arc::new(testkit_ports::SteppedClock::new(
            broker_toss::TossTimestamp::try_from("2026-11-16T00:00:00Z")
                .expect("clock")
                .into_domain(),
        ));
        let factory = Arc::new(super::database_factory(clock));
        tauri::async_runtime::block_on(async {
            let state = open_database(&secrets, factory, || Ok(path)).await;
            assert_eq!(state.status(), DatabaseStatus::Ready);
            let lifecycle =
                Lifecycle::new(tauri::async_runtime::spawn(std::future::pending()), state);
            lifecycle.shutdown().await;
            let guard = lifecycle.outcome().lock().await;
            match &guard.as_ref().expect("종료 결과").database {
                Ok(src_tauri::lifecycle::DatabaseClosure::Closed(_receipt)) => {}
                Ok(src_tauri::lifecycle::DatabaseClosure::NotOpened) => {
                    panic!("열린 DB의 명시적 종료 증거 필요")
                }
                Err(error) => panic!("종료 실패: {error:?}"),
            }
        });
        std::fs::remove_dir_all(directory).expect("테스트 정리");
    }
}
