#[must_use]
pub fn property_cases() -> u32 {
    cases_for(
        std::env::var("STOCK_COPILOT_PROPTEST_CASES")
            .as_deref()
            .ok(),
    )
}

#[must_use]
pub fn property_cases_at_least(minimum: u32) -> u32 {
    property_cases().max(minimum)
}

fn cases_for(ci: Option<&str>) -> u32 {
    match ci {
        None => 256,
        Some(_) => 4096,
    }
}

#[cfg(test)]
mod tests {
    use super::{cases_for, property_cases, property_cases_at_least};

    #[test]
    fn public_functions_follow_environment() {
        for value in [None, Some("4096"), Some("1"), Some(""), Some("wrong")] {
            let mut command =
                std::process::Command::new(std::env::current_exe().expect("test binary"));
            command.args(["--exact", "tests::property_cases_environment_probe"]);
            command.env_remove("STOCK_COPILOT_PROPTEST_CASES");
            if let Some(text) = value {
                command.env("STOCK_COPILOT_PROPTEST_CASES", text);
            }
            assert!(
                command.status().expect("environment probe").success(),
                "{value:?}"
            );
        }
    }

    #[test]
    fn property_cases_environment_probe() {
        let expected = if std::env::var("STOCK_COPILOT_PROPTEST_CASES").is_ok() {
            4096
        } else {
            256
        };
        assert_eq!(property_cases(), expected);
        assert!(property_cases_at_least(4096) >= 4096);
    }
    #[test]
    fn local_and_full_counts_are_fixed() {
        assert_eq!(cases_for(None), 256);
        assert_eq!(cases_for(Some("256")), 4096);
        assert_eq!(cases_for(Some("4096")), 4096);
    }
}

mod http_server;
pub use http_server::{
    ConnectionObservation, ConnectionProbe, LocalHttpServer, check_consumer, proxy_probe,
};

pub fn test_executable() -> std::io::Result<std::path::PathBuf> {
    std::env::current_exe()
}

pub fn canonical_temp_dir() -> std::io::Result<std::path::PathBuf> {
    std::fs::canonicalize(std::env::temp_dir())
}
pub use http_server::{ServeSequence, serve_sequence, serve_sequence_with_pending_first};

mod scripted_http_server;
pub use scripted_http_server::{GateObservation, HoldPosition, ResponseGate, ScriptedHttpServer};

pub mod fake_ai_cli;
mod fake_ai_cli_replay;
mod fake_mcp_client;
