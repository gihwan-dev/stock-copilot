#[cfg(test)]
mod tests {
    use ai_claude_code::{
        DiscoveryConfig, DiscoveryOutcome, DiscoverySource, HostEnvironment, LoginCheckOutcome,
        StaticPathResolver, check_login_status, default_known_locations, find_claude_executable,
        run_claude_startup_check,
    };
    use application::{
        AiEnvironmentProblem, AiUsage, BugDiagnostic, CheckState, ClaudeInstallProblem,
        ClaudeLoginProblem, IsolationFailure, ResponseMismatch, SelfCheckAction, SelfCheckInput,
        evaluate_self_check,
    };
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(1_u64);

    struct TempDirGuard(PathBuf);

    impl TempDirGuard {
        fn new(label: &str) -> Self {
            let seq = COUNTER.fetch_add(1_u64, Ordering::Relaxed);
            let path =
                PathBuf::from("/tmp").join(format!("sc-ai13-{}-{seq}-{label}", std::process::id()));
            fs::create_dir_all(&path).expect("create dir");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            match fs::remove_dir_all(&self.0) {
                Ok(()) | Err(_) => {}
            }
        }
    }

    fn write_executable(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("mkdir");
        }
        fs::write(path, body).expect("write script");
        fs::set_permissions(path, fs::Permissions::from_mode(0o700_u32)).expect("chmod");
    }

    fn healthy_self_check_input() -> SelfCheckInput {
        SelfCheckInput {
            toss_key: CheckState::Healthy,
            allowed_ip: CheckState::Healthy,
            duplicate_key_use: CheckState::Healthy,
            claude_install: CheckState::Healthy,
            claude_login: CheckState::Healthy,
            ai_environment: CheckState::Healthy,
            ai_usage: AiUsage::Unknown,
            order_prerequisite: CheckState::Healthy,
        }
    }

    fn fake_host_env(home: &Path, path_env: &str) -> HostEnvironment {
        HostEnvironment::from_pairs([
            ("HOME", home.to_string_lossy().into_owned()),
            ("PATH", String::from(path_env)),
            ("USER", String::from("testuser")),
            ("LOGNAME", String::from("testuser")),
            ("TMPDIR", String::from("/tmp")),
            ("TOSS_CLIENT_SECRET", String::from("should-never-leak")),
            ("STOCK_MCP_TOKEN", String::from("mcp-never-in-auth-status")),
            ("CLAUDE_CONFIG_DIR", String::from("/tmp/should-not-pass")),
        ])
    }

    #[test]
    fn discovery_checks_known_locations_then_process_path_then_login_shell_path() {
        let dir = TempDirGuard::new("priority");
        let fake_home = dir.path().join("home");
        let known_bin = fake_home.join(".local").join("bin").join("claude");
        let proc_dir = dir.path().join("proc-bin");
        let shell_dir = dir.path().join("shell-bin");
        write_executable(&shell_dir.join("claude"), "#!/bin/sh\nexit 0\n");

        let env_empty_path = fake_host_env(&fake_home, "/nonexistent-dir");
        let cfg_shell =
            DiscoveryConfig::new(env_empty_path).with_known_locations(vec![known_bin.clone()]);
        let shell_resolver =
            StaticPathResolver::new(Some(shell_dir.to_string_lossy().into_owned()));
        assert_eq!(
            find_claude_executable(&cfg_shell, &shell_resolver),
            DiscoveryOutcome::Found {
                path: shell_dir.join("claude"),
                source: DiscoverySource::LoginShellPath,
            },
            "falls back to login shell PATH when not in known locations or process PATH"
        );

        write_executable(&proc_dir.join("claude"), "#!/bin/sh\nexit 0\n");
        let env_with_proc = fake_host_env(&fake_home, &proc_dir.to_string_lossy());
        let cfg_proc =
            DiscoveryConfig::new(env_with_proc).with_known_locations(vec![known_bin.clone()]);
        assert_eq!(
            find_claude_executable(&cfg_proc, &shell_resolver),
            DiscoveryOutcome::Found {
                path: proc_dir.join("claude"),
                source: DiscoverySource::ProcessPath,
            },
            "prefers process PATH over login shell PATH"
        );

        write_executable(&known_bin, "#!/bin/sh\nexit 0\n");
        assert_eq!(
            find_claude_executable(&cfg_proc, &shell_resolver),
            DiscoveryOutcome::Found {
                path: known_bin,
                source: DiscoverySource::KnownLocation,
            },
            "prefers known location over PATH"
        );
    }

    #[test]
    fn discovery_ignores_non_executable_files_and_respects_missing_explicit_override() {
        let dir = TempDirGuard::new("non-exec");
        let fake_home = dir.path().join("home");
        let non_exec = fake_home.join(".local").join("bin").join("claude");
        fs::create_dir_all(non_exec.parent().expect("p")).expect("mkdir");
        fs::write(&non_exec, "not executable").expect("write");
        fs::set_permissions(&non_exec, fs::Permissions::from_mode(0o600_u32)).expect("chmod 600");

        let cfg = DiscoveryConfig::new(fake_host_env(&fake_home, "relative/bin"))
            .with_known_locations(vec![non_exec]);
        let resolver = StaticPathResolver::new(Some(String::from("./relative-shell")));
        assert_eq!(
            find_claude_executable(&cfg, &resolver),
            DiscoveryOutcome::NotInstalled,
            "non-executable file and relative PATH entries are ignored"
        );

        let existing = dir.path().join("real-fake");
        write_executable(&existing, "#!/bin/sh\nexit 0\n");
        let override_missing = DiscoveryConfig::new(fake_host_env(&fake_home, "/usr/bin"))
            .with_known_locations(vec![existing])
            .with_explicit_override(dir.path().join("missing-override"));
        assert_eq!(
            find_claude_executable(&override_missing, &resolver),
            DiscoveryOutcome::NotInstalled,
            "missing explicit override must not silently fall back"
        );
        let defaults = default_known_locations(Some("/tmp/h"));
        assert!(
            defaults.contains(&PathBuf::from("/tmp/h/.local/bin/claude"))
                && defaults.contains(&PathBuf::from("/opt/homebrew/bin/claude")),
            "default known locations cover home local and homebrew"
        );
    }

    #[test]
    fn not_installed_reports_self_check_4_banner() {
        let dir = TempDirGuard::new("not-installed");
        let cfg = DiscoveryConfig::new(fake_host_env(dir.path(), "/nonexistent"))
            .with_known_locations(Vec::new());
        let report =
            run_claude_startup_check(&cfg, &StaticPathResolver::new(None), &dir.path().join("w"));
        assert_eq!(
            report.claude_install,
            CheckState::Problem(ClaudeInstallProblem::NotInstalled),
            "check #4 reports NotInstalled"
        );
        let mut input = healthy_self_check_input();
        report.apply_to_self_check(&mut input);
        let eval = evaluate_self_check(input, Vec::new());
        assert_eq!(
            eval.banner,
            Some(SelfCheckAction::InstallClaudeCode),
            "check #4 triggers InstallClaudeCode banner"
        );
    }

    #[test]
    fn auth_status_logged_in_verifies_sanitized_env_and_produces_no_banner() {
        let dir = TempDirGuard::new("logged-in");
        let fake_cli = dir.path().join("claude");
        let script = "#!/bin/sh\nif [ \"$1\" != \"auth\" ] || [ \"$2\" != \"status\" ]; then exit 9; fi\nif [ -n \"$TOSS_CLIENT_SECRET\" ] || [ -n \"$STOCK_MCP_TOKEN\" ] || [ -n \"$CLAUDE_CONFIG_DIR\" ]; then exit 8; fi\nif [ \"$CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC\" != \"1\" ] || [ -z \"$USER\" ]; then exit 7; fi\nprintf '{\"loggedIn\":true,\"apiKeySource\":\"none\"}\\n'\n";
        write_executable(&fake_cli, script);
        let cfg = DiscoveryConfig::new(fake_host_env(dir.path(), "/usr/bin:/bin"))
            .with_explicit_override(fake_cli);
        let report =
            run_claude_startup_check(&cfg, &StaticPathResolver::new(None), &dir.path().join("w"));
        assert_eq!(report.claude_install, CheckState::Healthy, "installed");
        assert_eq!(report.claude_login, CheckState::Healthy, "logged in");
        assert_eq!(report.ai_environment, CheckState::Healthy, "env clean");
        let mut input = healthy_self_check_input();
        report.apply_to_self_check(&mut input);
        assert_eq!(
            evaluate_self_check(input, Vec::new()).banner,
            None,
            "healthy startup produces zero banner"
        );
    }

    #[test]
    fn auth_status_logged_out_reports_self_check_5_banner() {
        let dir = TempDirGuard::new("logged-out");
        let fake_cli = dir.path().join("claude");
        write_executable(
            &fake_cli,
            "#!/bin/sh\nprintf '{\"loggedIn\":false}\\n'\nexit 1\n",
        );
        let cfg = DiscoveryConfig::new(fake_host_env(dir.path(), "/usr/bin:/bin"))
            .with_explicit_override(fake_cli);
        let report =
            run_claude_startup_check(&cfg, &StaticPathResolver::new(None), &dir.path().join("w"));
        assert_eq!(
            report.claude_login,
            CheckState::Problem(ClaudeLoginProblem::LoginRequired),
            "check #5 reports LoginRequired"
        );
        let mut input = healthy_self_check_input();
        report.apply_to_self_check(&mut input);
        assert_eq!(
            evaluate_self_check(input, Vec::new()).banner,
            Some(SelfCheckAction::LoginClaudeCode),
            "check #5 triggers LoginClaudeCode banner"
        );
    }

    #[test]
    fn api_key_in_env_refuses_spawn_and_maps_to_check_6_without_banner() {
        let dir = TempDirGuard::new("api-key-env");
        let marker = dir.path().join("spawned.marker");
        let fake_cli = dir.path().join("claude");
        write_executable(
            &fake_cli,
            &format!(
                "#!/bin/sh\ntouch '{}'\nprintf '{{\"loggedIn\":true}}\\n'\n",
                marker.display()
            ),
        );
        let env_with_key = HostEnvironment::from_pairs([
            ("HOME", "/tmp/h"),
            ("PATH", "/usr/bin"),
            ("USER", "u"),
            ("TMPDIR", "/tmp"),
            ("ANTHROPIC_API_KEY", "sk-ant-forbidden"),
        ]);
        let cfg = DiscoveryConfig::new(env_with_key).with_explicit_override(fake_cli);
        let report =
            run_claude_startup_check(&cfg, &StaticPathResolver::new(None), &dir.path().join("w"));
        assert!(
            !marker.exists(),
            "must never spawn binary when API key present"
        );
        assert_eq!(
            report.ai_environment,
            CheckState::Problem(AiEnvironmentProblem::ApiKeySource),
            "maps to check #6 ApiKeySource"
        );
        let mut input = healthy_self_check_input();
        report.apply_to_self_check(&mut input);
        assert_eq!(
            evaluate_self_check(input, Vec::new()).banner,
            None,
            "check #6 never triggers main screen banner"
        );
    }

    #[test]
    fn malformed_and_api_key_source_auth_status_fail_closed() {
        let dir = TempDirGuard::new("bad-auth");
        let bad_json = dir.path().join("claude-bad");
        write_executable(&bad_json, "#!/bin/sh\nprintf 'not-json'\n");
        let env = fake_host_env(dir.path(), "/usr/bin:/bin");
        assert_eq!(
            check_login_status(&bad_json, &dir.path().join("w1"), &env),
            LoginCheckOutcome::CheckUnavailable,
            "malformed stdout fails closed"
        );
        let cfg_bad = DiscoveryConfig::new(env.clone()).with_explicit_override(bad_json);
        let rep_bad = run_claude_startup_check(
            &cfg_bad,
            &StaticPathResolver::new(None),
            &dir.path().join("w2"),
        );
        assert_eq!(
            rep_bad.claude_login,
            CheckState::Bug(BugDiagnostic::Response(ResponseMismatch::Validation)),
            "malformed auth status recorded as bug diagnostic, not healthy"
        );

        let api_src = dir.path().join("claude-api");
        write_executable(
            &api_src,
            "#!/bin/sh\nprintf '{\"loggedIn\":true,\"apiKeySource\":\"env\"}\\n'\n",
        );
        assert_eq!(
            check_login_status(&api_src, &dir.path().join("w3"), &env),
            LoginCheckOutcome::EnvironmentRejected(IsolationFailure::ApiKeySource),
            "non-none apiKeySource rejected"
        );
    }
}
